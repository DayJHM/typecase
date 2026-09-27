/* Library persistence (CONTEXT.md §32): a single JSON file in the app data
   directory — the simplest reliable mechanism. Atomic writes; corruption
   falls back to defaults rather than fabricating state. */

use crate::library::model::LibraryState;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub type States = HashMap<String, LibraryState>;

pub struct Store {
    path: PathBuf,
}

impl Store {
    /// Build a store for `dir` (the app data dir). The file itself is created
    /// lazily on first save.
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("state").join("library.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load persisted state. Missing or corrupt files yield an empty map —
    /// never fabricated cached/installed entries (STAGE2_PLAN §4).
    pub fn load(&self) -> Result<States, String> {
        match fs::read_to_string(&self.path) {
            Ok(text) => {
                if text.trim().is_empty() {
                    return Ok(States::new());
                }
                serde_json::from_str(&text).map_err(|e| {
                    format!(
                        "corrupt library state at {}: {e}; starting from defaults",
                        self.path.display()
                    )
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(States::new()),
            Err(e) => Err(format!("cannot read {}: {e}", self.path.display())),
        }
    }

    /// Atomic save: write to a temp file in the same directory, fsync, rename.
    pub fn save(&self, states: &States) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let tmp = self.path.with_extension("json.tmp");
        {
            let text = serde_json::to_string_pretty(states)
                .map_err(|e| format!("serialize state: {e}"))?;
            let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
            f.write_all(text.as_bytes())
                .and_then(|_| f.sync_all())
                .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        }
        fs::rename(&tmp, &self.path).map_err(|e| format!("rename into place: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("typecase-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn fresh_dir_loads_empty() {
        let store = Store::new(&tmpdir("fresh"));
        let states = store.load().unwrap();
        assert!(states.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tmpdir("roundtrip");
        let store = Store::new(&dir);
        let mut states = States::new();
        states.insert(
            "inter".into(),
            LibraryState {
                cached: true,
                installed: true,
                managed_by_typecase: Some(true),
                install_scope: Some(crate::library::model::Scope::User),
            },
        );
        store.save(&states).unwrap();

        let reloaded = Store::new(&dir).load().unwrap();
        assert_eq!(reloaded.len(), 1);
        assert!(reloaded["inter"].cached && reloaded["inter"].installed);
        assert_eq!(reloaded["inter"].install_scope, Some(crate::library::model::Scope::User));
    }

    #[test]
    fn corrupt_file_falls_back_to_empty_with_error() {
        let dir = tmpdir("corrupt");
        let store = Store::new(&dir);
        std::fs::create_dir_all(dir.join("state")).unwrap();
        std::fs::write(store.path(), "{ not json !!!").unwrap();

        let result = store.load();
        assert!(result.is_err(), "corruption must be reported, not silently accepted");
        assert!(store.load().unwrap_err().contains("corrupt"));
    }

    #[test]
    fn empty_file_is_empty_state() {
        let dir = tmpdir("emptyfile");
        let store = Store::new(&dir);
        std::fs::create_dir_all(dir.join("state")).unwrap();
        std::fs::write(store.path(), "").unwrap();
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn save_is_atomic_no_tmp_leftover() {
        let dir = tmpdir("atomic");
        let store = Store::new(&dir);
        store.save(&States::new()).unwrap();
        assert!(store.path().exists());
        assert!(!store.path().with_extension("json.tmp").exists());
    }
}
