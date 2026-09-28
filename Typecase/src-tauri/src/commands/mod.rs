/* Intent-level commands (CONTEXT.md §29): React requests Typecase operations,
   never arbitrary system operations. State is a single Mutex-protected
   catalog + library bundle. */

use crate::catalog::Catalog;
use crate::downloads::{self, FontMeta};
use crate::fontmanager::InstallRecord;
use crate::library::model::{FaceStatus, FlatState, LibraryState, Scope};
use crate::library::store::{Store, States};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use tauri::{Manager, State};

pub struct TypecaseState {
    /// The active catalog. RwLock (not Mutex) because commands only read it;
    /// the single writer is the M8 apply path. Swapped atomically.
    pub catalog: RwLock<Catalog>,
    pub data_dir: PathBuf,
    /// Built once at setup (outside any async runtime): reqwest's blocking
    /// client owns an internal tokio runtime, which must never be created or
    /// dropped from within an async context. Commands only clone the handle.
    pub http: reqwest::blocking::Client,
    /// M8: a fetched-but-not-applied refresh (§15: nothing auto-applies).
    pub pending_catalog: Mutex<Option<PendingRefresh>>,
    pub install_store: crate::fontmanager::store::InstallStore,
    /// What Typecase wrote to Windows, keyed by face id (M6 §2 contract).
    pub installs: Mutex<HashMap<String, InstallRecord>>,
    pub store: Store,
    pub states: Mutex<States>,
}

/// M8: a refresh result the user has not acted on yet.
pub struct PendingRefresh {
    /// The §16 apply-merge of the active catalog with the fresh candidate.
    pub merged: Vec<crate::library::model::FontRecord>,
    pub diff: crate::catalog::CatalogDiff,
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
    let catalog = state.catalog.read().expect("catalog rwlock poisoned");
    let out = all_statuses(&catalog.records, &states);
    #[cfg(debug_assertions)]
    eprintln!("[typecase] get_catalog: {} faces", out.len());
    out
}

#[tauri::command]
pub fn get_library(state: State<TypecaseState>) -> Vec<FaceStatus> {
    let states = state.states.lock().expect("library mutex poisoned");
    let catalog = state.catalog.read().expect("catalog rwlock poisoned");
    library_statuses(&catalog.records, &states)
}

#[tauri::command]
pub fn get_font_details(state: State<TypecaseState>, id: String) -> Option<FaceStatus> {
    let states = state.states.lock().expect("library mutex poisoned");
    let catalog = state.catalog.read().expect("catalog rwlock poisoned");
    catalog
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
    let rec = {
        let catalog = state.catalog.read().expect("catalog rwlock poisoned");
        catalog
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("unknown face id: {id}"))?
    };
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
    let rec = {
        let catalog = state.catalog.read().expect("catalog rwlock poisoned");
        catalog
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("unknown face id: {id}"))?
    };
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

/// M7: every font registered in Windows (both scopes), classified by
/// ownership against Typecase's install records (§19–20).
#[tauri::command]
pub fn get_installed_fonts(
    state: State<TypecaseState>,
) -> Result<Vec<crate::externalfonts::ExternalFont>, String> {
    let records = state.installs.lock().expect("install mutex poisoned");
    Ok(crate::externalfonts::discover(&records))
}

/// M7: remove an EXTERNAL font (no Typecase record) after the UI's explicit
/// §25 confirmation. System scope elevates via the M6 one-shot helper.
#[tauri::command]
pub fn remove_external_font(
    state: State<TypecaseState>,
    value_name: String,
    file_path: String,
    scope: String,
) -> Result<(), String> {
    #[cfg(debug_assertions)]
    eprintln!("[typecase] remove_external_font: {value_name} scope={scope}");
    let scope = match scope.as_str() {
        "user" => Scope::User,
        "system" => Scope::System,
        other => return Err(format!("unknown scope: {other}")),
    };
    // Refuse to remove anything Typecase manages through this path — managed
    // fonts have their own record-based uninstall (§20).
    let records = state.installs.lock().expect("install mutex poisoned");
    if records.values().any(|r| {
        r.entries
            .iter()
            .any(|e| e.value_name == value_name)
    }) {
        return Err(
            "this font was installed by Typecase — use the managed uninstall instead".into(),
        );
    }
    drop(records);

    if scope == Scope::System {
        crate::externalfonts::remove_external_elevated(&value_name, &file_path)
    } else {
        crate::externalfonts::remove_external(
            Scope::User,
            &value_name,
            &file_path,
        )
    }
}

/* ---- M8: catalog refresh (§15) + export/backup (§26) ---- */

/// M8: fetch the live metadata endpoint, run the same merge as the M3
/// generator (Rust port), and diff it against the active catalog. The result
/// is PARKED as a pending refresh — nothing is applied until the user
/// explicitly confirms (§15: manual, user-controlled, non-destructive).
/// The network runs on a blocking thread; no lock is held across it.
#[tauri::command]
pub async fn refresh_catalog(state: State<'_, TypecaseState>) -> Result<crate::catalog::CatalogDiff, String> {
    refresh_catalog_inner(&state).await
}

/// The command body, callable directly from the dev-only M8 check.
pub async fn refresh_catalog_inner(state: &TypecaseState) -> Result<crate::catalog::CatalogDiff, String> {
    #[cfg(debug_assertions)]
    eprintln!("[typecase] refresh_catalog: fetching {}", crate::catalog::refresh::METADATA_URL);
    let client = state.http.clone();
    let payload = tauri::async_runtime::spawn_blocking(move || -> Result<crate::catalog::refresh::EndpointPayload, String> {
        let res = client
            .get(crate::catalog::refresh::METADATA_URL)
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("metadata fetch failed: {e}"))?;
        let raw = res.text().map_err(|e| format!("metadata read failed: {e}"))?;
        crate::catalog::refresh::parse_payload(&raw)
    })
    .await
    .map_err(|e| format!("refresh task failed: {e}"))??;

    let candidate = crate::catalog::refresh::build_candidate(&payload);
    let diff_raw = {
        let catalog = state.catalog.read().expect("catalog rwlock poisoned");
        crate::catalog::refresh::diff_catalog(&catalog.records, &candidate)
    };
    #[cfg(debug_assertions)]
    eprintln!(
        "[typecase] refresh_catalog: +{} ~{} -{} ({} total)",
        diff_raw.added, diff_raw.changed, diff_raw.removed, diff_raw.total
    );
    let diff = crate::catalog::CatalogDiff {
        total: diff_raw.total,
        added: diff_raw.added,
        changed: diff_raw.changed,
        removed: diff_raw.removed,
        added_names: diff_raw.added_names.clone(),
        removed_names: diff_raw.removed_names.clone(),
        unchanged: diff_raw.unchanged,
    };
    if diff_raw.unchanged {
        // Nothing to apply — do not park a no-op candidate.
        return Ok(diff);
    }
    let merged = {
        let catalog = state.catalog.read().expect("catalog rwlock poisoned");
        crate::catalog::refresh::apply_merge(&catalog.records, &candidate)
    };
    *state
        .pending_catalog
        .lock()
        .expect("pending catalog mutex poisoned") = Some(PendingRefresh { merged, diff: diff.clone() });
    Ok(diff)
}

/// M8: apply a parked refresh — the explicit user-confirmed step. Writes the
/// merged catalog to catalog/catalog.json and swaps it into the active state.
/// The embedded snapshot remains the offline floor for future loads.
#[tauri::command]
pub fn apply_catalog_refresh(state: State<TypecaseState>) -> Result<crate::catalog::ApplyOutcome, String> {
    apply_catalog_refresh_inner(&state)
}

/// The command body, callable directly from the dev-only M8 check.
pub fn apply_catalog_refresh_inner(state: &TypecaseState) -> Result<crate::catalog::ApplyOutcome, String> {
    let pending = state
        .pending_catalog
        .lock()
        .expect("pending catalog mutex poisoned")
        .take()
        .ok_or_else(|| "no catalog refresh is pending".to_string())?;
    crate::catalog::save_to_disk(&state.data_dir, &pending.merged)?;
    let removed_total = pending.merged.iter().filter(|r| r.removed_from_source).count();
    {
        let mut catalog = state.catalog.write().expect("catalog rwlock poisoned");
        *catalog = Catalog { records: pending.merged };
    }
    #[cfg(debug_assertions)]
    eprintln!(
        "[typecase] apply_catalog_refresh: {} records ({} marked removed-from-source)",
        pending.diff.total, removed_total
    );
    Ok(crate::catalog::ApplyOutcome { total: pending.diff.total, removed_total })
}

/// M8 export outcome payload.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOutcome {
    pub id: String,
    pub family: String,
    pub file: String,
    pub file_count: usize,
    pub source: String,
}

/// M8 (§11/§26): export a family as a ZIP of its TTF/OTF files plus a README
/// manifest. Cached files are the primary source; installed files are the
/// fallback. Works offline (§26) — no network is involved. The destination
/// directory is chosen by the backend (§31); the frontend never supplies one.
#[tauri::command]
pub fn export_font(state: State<TypecaseState>, id: String) -> Result<ExportOutcome, String> {
    export_font_inner(&state, &id)
}

/// The command body, callable directly from the dev-only M8 check.
pub fn export_font_inner(state: &TypecaseState, id: &str) -> Result<ExportOutcome, String> {
    #[cfg(debug_assertions)]
    eprintln!("[typecase] export_font: {id}");
    let rec = {
        let catalog = state.catalog.read().expect("catalog rwlock poisoned");
        catalog
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("unknown face id: {id}"))?
    };
    let installs = state.installs.lock().expect("install mutex poisoned");
    let source = crate::exports::export_source(&state.data_dir, &rec, &installs)
        .ok_or_else(|| format!("{id} has no exportable files — cache or install it first"))?;
    drop(installs);
    let (files, source_name) = match &source {
        crate::exports::ExportSource::Cached { files } => (files.clone(), "cached"),
        crate::exports::ExportSource::Installed { files } => (files.clone(), "installed"),
    };
    let archive = crate::exports::build_family_zip(&rec, &files)?;
    let path = crate::exports::write_export(&exports_dir(&state.data_dir), &id, &archive)?;
    Ok(ExportOutcome {
        id: id.to_string(),
        family: rec.family,
        file: path.to_string_lossy().into_owned(),
        file_count: files.len(),
        source: source_name.into(),
    })
}

/// Where exports land: exports/ under the app data dir (§32 layout), kept
/// out of the fonts cache so a cache deletion can never eat a backup.
pub fn exports_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("exports")
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
        catalog: RwLock::new(Catalog::for_dir(&dir)),
        data_dir: dir,
        http,
        pending_catalog: Mutex::new(None),
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
            removed_from_source: false,
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
