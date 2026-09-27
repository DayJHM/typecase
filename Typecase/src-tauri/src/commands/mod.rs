/* Intent-level commands (CONTEXT.md §29): React requests Typecase operations,
   never arbitrary system operations. State is a single Mutex-protected
   catalog + library bundle. */

use crate::catalog::Catalog;
use crate::downloads::{self, FontMeta};
use crate::fontmanager::InstallRecord;
use crate::library::model::{FaceStatus, FlatState, LibraryState, Scope};
use crate::library::store::{Store, States};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

pub struct TypecaseState {
    pub catalog: Catalog,
    pub data_dir: PathBuf,
    /// Built once at setup (outside any async runtime): reqwest's blocking
    /// client owns an internal tokio runtime, which must never be created or
    /// dropped from within an async context. Commands only clone the handle.
    pub http: reqwest::blocking::Client,
    pub install_store: crate::fontmanager::store::InstallStore,
    /// What Typecase wrote to Windows, keyed by face id (M6 §2 contract).
    pub installs: Mutex<HashMap<String, InstallRecord>>,
    pub store: Store,
    pub states: Mutex<States>,
}

fn status(record: &crate::library::model::FontRecord, state: &LibraryState) -> FaceStatus {
    FaceStatus {
        record: record.clone(),
        state: FlatState::from(state),
    }
}

/* Pure helpers so the filtering/ordering logic is unit-testable without a
   Tauri State (the commands themselves are thin adapters over these). */

fn all_statuses(records: &[crate::library::model::FontRecord], states: &States) -> Vec<FaceStatus> {
    records
        .iter()
        .map(|r| {
            let st = states.get(&r.id).cloned().unwrap_or_default();
            status(r, &st)
        })
        .collect()
}

fn library_statuses(records: &[crate::library::model::FontRecord], states: &States) -> Vec<FaceStatus> {
    records
        .iter()
        .filter(|r| states.get(&r.id).is_some_and(|s| s.cached || s.installed))
        .map(|r| {
            let st = states.get(&r.id).cloned().unwrap_or_default();
            status(r, &st)
        })
        .collect()
}

#[tauri::command]
pub fn get_catalog(state: State<TypecaseState>) -> Vec<FaceStatus> {
    let states = state.states.lock().expect("library mutex poisoned");
    let out = all_statuses(&state.catalog.records, &states);
    #[cfg(debug_assertions)]
    eprintln!("[typecase] get_catalog: {} faces", out.len());
    out
}

#[tauri::command]
pub fn get_library(state: State<TypecaseState>) -> Vec<FaceStatus> {
    let states = state.states.lock().expect("library mutex poisoned");
    library_statuses(&state.catalog.records, &states)
}

#[tauri::command]
pub fn get_font_details(state: State<TypecaseState>, id: String) -> Option<FaceStatus> {
    let states = state.states.lock().expect("library mutex poisoned");
    state
        .catalog
        .get(&id)
        .map(|r| {
            let st = states.get(&r.id).cloned().unwrap_or_default();
            status(r, &st)
        })
}

/// M5: servable sources for every cached family — full `font://` URLs built
/// by the backend from the manifests, so the frontend never touches paths.
#[tauri::command]
pub fn get_font_sources(state: State<TypecaseState>) -> Vec<crate::downloads::FontSource> {
    crate::downloads::font_sources(&state.data_dir)
}

/// M4: download a family from the configured source, validate every file
/// (sfnt magic, size bounds, streamed sha256), commit it into
/// fonts/<family-id>/, and mark it cached. The network pipeline runs on a
/// blocking thread so the UI stays responsive; the library-state lock is
/// never held across network I/O.
#[tauri::command]
pub async fn download_font(
    state: State<'_, TypecaseState>,
    id: String,
) -> Result<FontMeta, String> {
    #[cfg(debug_assertions)]
    eprintln!("[typecase] download_font: invoked for {id}");
    let rec = state
        .catalog
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("unknown face id: {id}"))?;
    let data_dir = state.data_dir.clone();
    let client = state.http.clone();
    let rec_for_task = rec.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        downloads::download_all(&client, &data_dir, &rec_for_task)
    })
    .await
    .map_err(|e| format!("download task failed: {e}"))??;
    #[cfg(debug_assertions)]
    eprintln!(
        "[typecase] download_font: validated {} file(s), committing",
        outcome.files.len()
    );

    let meta = downloads::commit(&state.data_dir, &rec, &outcome)?;
    let mut states = state.states.lock().expect("library mutex poisoned");
    downloads::update_library_state(&mut states, &rec.id);
    let snapshot = states.clone();
    drop(states);
    state.store.save(&snapshot)?;
    Ok(meta)
}

/// §24 result payload for the frontend confirmation flow.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOutcome {
    pub id: String,
    pub freed_bytes: u64,
}

/// §24: the separate, explicit cache-deletion operation (uninstalling never
/// deletes the cache; only this does). Removes fonts/<family-id>/ and clears
/// the cached flag; installed/ownership state is preserved —
/// installed-but-missing-locally is a valid §17 state. The UI asks for
/// explicit confirmation before invoking this.
#[tauri::command]
pub fn delete_cached_family(
    state: State<TypecaseState>,
    id: String,
) -> Result<DeleteOutcome, String> {
    #[cfg(debug_assertions)]
    eprintln!("[typecase] delete_cached_family: {id}");
    // Validate before touching anything: the operation targets an existing cache.
    {
        let states = state.states.lock().expect("library mutex poisoned");
        if !states.get(&id).is_some_and(|s| s.cached) {
            return Err(format!("{id} is not cached"));
        }
    }
    let freed = crate::downloads::delete_cached_dir(&state.data_dir, &id)?;
    let snapshot = {
        let mut states = state.states.lock().expect("library mutex poisoned");
        crate::downloads::clear_cached_state(&mut states, &id)?;
        states.clone()
    };
    state.store.save(&snapshot)?;
    Ok(DeleteOutcome { id, freed_bytes: freed })
}

/// UI: make the native window chrome (title bar on Windows via UxTheme,
/// GTK hint on Linux) follow the in-app theme preference. The frontend
/// invokes this at startup and on every dark-mode toggle.
#[tauri::command]
pub fn apply_window_theme(window: tauri::Window, dark: bool) -> Result<(), String> {
    #[cfg(debug_assertions)]
    eprintln!("[typecase] apply_window_theme: dark={dark}");
    let theme = if dark { tauri::Theme::Dark } else { tauri::Theme::Light };
    window
        .set_theme(Some(theme))
        .map_err(|e| format!("cannot set window theme: {e}"))
}

/* ---- M6: install / uninstall (§29 intent level; §21 abstraction) ---- */

/// M6: install every cached file of a family for `scope`. Builds the plan
/// from the cache manifest, executes it through the platform FontManager,
/// then records exactly what was written (uninstall reverses the record).
#[tauri::command]
pub fn install_font(
    state: State<TypecaseState>,
    id: String,
    scope: String,
) -> Result<InstallOutcome, String> {
    #[cfg(debug_assertions)]
    eprintln!("[typecase] install_font: {id} scope={scope}");
    let scope = match scope.as_str() {
        "user" => Scope::User,
        "system" => Scope::System,
        other => return Err(format!("unknown scope: {other}")),
    };
    let rec = state
        .catalog
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("unknown face id: {id}"))?;
    let meta = crate::downloads::read_manifest(&state.data_dir, &id)
        .ok_or_else(|| format!("{id} is not cached — download it first"))?;

    // (weight, style, cached absolute path) for every manifest file.
    let files: Vec<(u16, String, String)> = meta
        .files
        .iter()
        .map(|f| {
            (
                f.weight,
                f.style.clone(),
                state
                    .data_dir
                    .join(crate::downloads::FONTS_DIR)
                    .join(&id)
                    .join(&f.file)
                    .to_string_lossy()
                    .into_owned(),
            )
        })
        .collect();
    let fonts_root = crate::fontmanager::fonts_root(scope);
    let record = crate::fontmanager::plan_install(
        &id,
        &rec.family,
        scope,
        &crate::downloads::now_rfc3339_pub(),
        &files,
        |name| fonts_root.join(name).to_string_lossy().into_owned(),
    );

    crate::fontmanager::install_scoped(&record)?;

    // Record what was written, then flip library state.
    let mut installs = state
        .installs
        .lock()
        .expect("install mutex poisoned");
    installs.insert(id.clone(), record);
    state.install_store.save(&installs)?;
    drop(installs);

    let snapshot = {
        let mut states = state.states.lock().expect("library mutex poisoned");
        let entry = states.entry(id.clone()).or_default();
        entry.installed = true;
        entry.managed_by_typecase = Some(true);
        entry.install_scope = Some(scope);
        states.clone()
    };
    state.store.save(&snapshot)?;
    Ok(InstallOutcome {
        id,
        family: rec.family,
        scope: scope_str(scope).into(),
        files: meta.files.len(),
    })
}

/// M6: uninstall a family Typecase installed, reversing exactly its recorded
/// entries. External/unknown fonts are refused (M7 adds the discovery +
/// warning flow; §20/§25).
#[tauri::command]
pub fn uninstall_font(state: State<TypecaseState>, id: String) -> Result<InstallOutcome, String> {
    #[cfg(debug_assertions)]
    eprintln!("[typecase] uninstall_font: {id}");
    let mut installs = state.installs.lock().expect("install mutex poisoned");
    let record = installs
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("{id} was not installed by Typecase"))?;
    crate::fontmanager::uninstall_scoped(&record)?;
    installs.remove(&id);
    state.install_store.save(&installs)?;
    drop(installs);

    let snapshot = {
        let mut states = state.states.lock().expect("library mutex poisoned");
        let entry = states.entry(id.clone()).or_default();
        entry.installed = false;
        // The cache survives uninstall (§24); managed flag drops with it.
        entry.managed_by_typecase = None;
        entry.install_scope = None;
        states.clone()
    };
    state.store.save(&snapshot)?;
    Ok(InstallOutcome {
        id,
        family: record.family,
        scope: scope_str(record.scope).into(),
        files: record.entries.len(),
    })
}

fn scope_str(scope: Scope) -> &'static str {
    match scope {
        Scope::User => "user",
        Scope::System => "system",
    }
}

/// M6 result payload for the UI.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    pub id: String,
    pub family: String,
    pub scope: String,
    pub files: usize,
}

pub fn init_state(app: &tauri::App) -> TypecaseState {
    let dir = app
        .path()
        .app_data_dir()
        .expect("cannot resolve app data dir");
    let store = Store::new(&dir);
    let (states, warning) = match store.load() {
        Ok(s) => (s, None),
        Err(e) => (States::new(), Some(e)),
    };
    if let Some(w) = warning {
        eprintln!("[typecase] {w}");
    }
    eprintln!("[typecase] library state: {}", store.path().display());
    let install_store = crate::fontmanager::store::InstallStore::new(&dir);
    let installs = match install_store.load() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[typecase] {e}");
            HashMap::new()
        }
    };
    eprintln!("[typecase] install records: {}", installs.len());
    let http = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());
    TypecaseState {
        catalog: Catalog::embedded(),
        data_dir: dir,
        http,
        install_store,
        installs: Mutex::new(installs),
        store,
        states: Mutex::new(states),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::model::FontRecord;

    fn rec(id: &str) -> FontRecord {
        FontRecord {
            id: id.into(),
            family: format!("Family {id}"),
            category: "Sans".into(),
            designer: String::new(),
            year: 2020,
            styles: 4,
            weights: vec![400],
            italic: false,
            note: String::new(),
            pairs_with: String::new(),
            popularity: 0,
        }
    }

    #[test]
    fn status_flattens_state_into_payload() {
        let r = rec("inter");
        let st = LibraryState {
            cached: true,
            ..Default::default()
        };
        let v = serde_json::to_value(status(&r, &st)).unwrap();
        assert_eq!(v["phase"], "library");
        assert_eq!(v["cached"], true);
        assert_eq!(v["family"], "Family inter");
    }

    #[test]
    fn all_statuses_defaults_missing_states() {
        let records = vec![rec("a"), rec("b")];
        let mut states = States::new();
        states.insert("b".into(), LibraryState {
            cached: true,
            ..Default::default()
        });
        let out = all_statuses(&records, &states);
        assert_eq!(out.len(), 2);
        let a = out.iter().find(|f| f.record.id == "a").unwrap();
        let b = out.iter().find(|f| f.record.id == "b").unwrap();
        assert_eq!(serde_json::to_value(a).unwrap()["phase"], "online");
        assert_eq!(serde_json::to_value(b).unwrap()["phase"], "library");
    }

    #[test]
    fn library_statuses_filters_to_cached_or_installed() {
        let records = vec![rec("online-face"), rec("cached"), rec("installed")];
        let mut states = States::new();
        states.insert("cached".into(), LibraryState {
            cached: true,
            installed: false,
            ..Default::default()
        });
        states.insert("installed".into(), LibraryState {
            cached: true,
            installed: true,
            managed_by_typecase: Some(true),
            install_scope: Some(crate::library::model::Scope::User),
        });
        let out = library_statuses(&records, &states);
        let mut ids: Vec<_> = out.iter().map(|f| f.record.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["cached", "installed"]);
        // catalog order is preserved within the filter
        assert_eq!(out[0].record.id, "cached");
    }

    #[test]
    fn library_statuses_empty_state_yields_empty() {
        let records = vec![rec("a"), rec("b")];
        let out = library_statuses(&records, &States::new());
        assert!(out.is_empty());
    }
}
