/* Install records (M6_FONTMANAGER_DESIGN §2): what Typecase actually wrote
   to Windows, per family. Uninstall reverses exactly these entries — absent
   record means "not installed by Typecase", never a registry guess (§20). */

use super::InstallRecord;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct InstallStore {
    path: PathBuf,
}

impl InstallStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("state").join("installed.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Missing file → empty map; corrupt file is an error (the caller logs
    /// and starts empty, same policy as library.json).
    pub fn load(&self) -> Result<HashMap<String, InstallRecord>, String> {
        match fs::read_to_string(&self.path) {
            Ok(text) if text.trim().is_empty() => Ok(HashMap::new()),
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| format!("corrupt install records at {}: {e}", self.path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(e) => Err(format!("cannot read {}: {e}", self.path.display())),
        }
    }

    /// Atomic write: temp file in the same directory, fsync, rename.
    pub fn save(&self, records: &HashMap<String, InstallRecord>) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(records)
            .map_err(|e| format!("serialize install records: {e}"))?;
        {
            let mut f = fs::File::create(&tmp)
                .map_err(|e| format!("create {}: {e}", tmp.display()))?;
            f.write_all(text.as_bytes())
                .and_then(|_| f.sync_all())
                .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        }
        fs::rename(&tmp, &self.path).map_err(|e| format!("rename into place: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fontmanager::plan_install;
    use crate::library::model::Scope;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("typecase-istore-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample(id: &str) -> InstallRecord {
        plan_install(
            id,
            "Inter",
            Scope::User,
            "2026-09-27T00:00:00Z",
            &[(400, "normal".into(), "C:\\cache\\a.ttf".into())],
            |n| format!("C:\\Users\\t\\Fonts\\{n}"),
        )
    }

    #[test]
    fn fresh_loads_empty_and_missing() {
        let s = InstallStore::new(&tmpdir("fresh"));
        assert!(s.load().unwrap().is_empty());
    }

    #[test]
    fn roundtrip_preserves_entries() {
        let dir = tmpdir("roundtrip");
        let s = InstallStore::new(&dir);
        let mut records = HashMap::new();
        records.insert("inter".into(), sample("inter"));
        s.save(&records).unwrap();

        let back = InstallStore::new(&dir).load().unwrap();
        assert_eq!(back["inter"], sample("inter"));
    }

    #[test]
    fn corrupt_reports_error() {
        let dir = tmpdir("corrupt");
        let s = InstallStore::new(&dir);
        fs::create_dir_all(dir.join("state")).unwrap();
        fs::write(s.path(), "{ nope").unwrap();
        assert!(s.load().is_err());
    }
}
