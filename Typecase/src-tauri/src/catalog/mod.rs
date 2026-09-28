/* Catalog loading (CONTEXT.md §14–15): the generated catalog.json is embedded
   at compile time as the bundled snapshot (§15: ships with the app). M8 adds
   the disk layer: catalog/catalog.json in the app data dir, produced only by
   an explicit user-confirmed refresh apply. Load order: disk if present, else
   embedded — the snapshot remains the offline floor. */

use crate::library::model::FontRecord;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct CatalogFile {
    #[allow(dead_code)]
    generated: String,
    #[allow(dead_code)]
    source: String,
    records: Vec<FontRecord>,
}

pub struct Catalog {
    pub records: Vec<FontRecord>,
}

/// Where the refreshed catalog lives under the app data dir (§32 layout).
pub const DISK_CATALOG_DIR: &str = "catalog";
pub const DISK_CATALOG_FILE: &str = "catalog.json";

impl Catalog {
    /// The compile-time bundled snapshot (no network, no API key).
    pub fn embedded() -> Self {
        const RAW: &str = include_str!("../../resources/catalog.json");
        let parsed: CatalogFile =
            serde_json::from_str(RAW).expect("embedded catalog.json must parse");
        Self {
            records: parsed.records,
        }
    }

    /// Disk catalog if present and valid; otherwise the embedded snapshot.
    /// Corrupt or empty disk files fall back — never a broken catalog.
    pub fn for_dir(data_dir: &Path) -> Self {
        let path = data_dir.join(DISK_CATALOG_DIR).join(DISK_CATALOG_FILE);
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<CatalogFile>(&text) {
                Ok(parsed) if !parsed.records.is_empty() => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[typecase] catalog: disk snapshot from {} ({} families)",
                        path.display(),
                        parsed.records.len()
                    );
                    Self {
                        records: parsed.records,
                    }
                }
                Ok(_) => {
                    eprintln!("[typecase] catalog: disk catalog at {} is empty; using embedded", path.display());
                    Self::embedded()
                }
                Err(e) => {
                    eprintln!("[typecase] catalog: corrupt disk catalog at {}: {e}; using embedded", path.display());
                    Self::embedded()
                }
            },
            Err(_) => Self::embedded(),
        }
    }

    pub fn get(&self, id: &str) -> Option<&FontRecord> {
        self.records.iter().find(|r| r.id == id)
    }
}

/// Atomically persist a catalog to disk (§32 layout). Temp file + fsync +
/// rename, mirroring the library/installs stores. The file is parseable by
/// `Catalog::for_dir` and by the M3 generator's consumers.
pub fn save_to_disk(data_dir: &Path, records: &[FontRecord]) -> Result<(), String> {
    let dir = data_dir.join(DISK_CATALOG_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let path = dir.join(DISK_CATALOG_FILE);
    let tmp = dir.join("catalog.json.tmp");
    let file = CatalogFileSer {
        generated: crate::downloads::now_rfc3339_pub(),
        source: "google-fonts",
        count: records.len(),
        records,
    };
    let text = serde_json::to_string(&file).map_err(|e| format!("serialize catalog: {e}"))?;
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
        f.write_all(text.as_bytes())
            .and_then(|_| f.sync_all())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
    }
    fs::rename(&tmp, &path).map_err(|e| format!("rename into place: {e}"))
}

/// Serialize side of CatalogFile (with count, matching the M3 shape).
#[derive(Serialize)]
struct CatalogFileSer<'a> {
    generated: String,
    source: &'static str,
    count: usize,
    records: &'a [FontRecord],
}

/* ---- M8 refresh wire payloads ---- */

/// What a refresh discovered (data for the non-intrusive §15 notice).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDiff {
    pub total: usize,
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
    pub added_names: Vec<String>,
    pub removed_names: Vec<String>,
    pub unchanged: bool,
}

/// Result of applying a refresh: how many records the active catalog now
/// carries and how many of them are marked removed-from-source (§16).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    pub total: usize,
    pub removed_total: usize,
}

pub mod refresh;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_loads() {
        let cat = Catalog::embedded();
        assert!(cat.records.len() > 1000, "bundled snapshot should be the full generated catalog");
    }

    #[test]
    fn ids_are_unique_and_sorted() {
        let cat = Catalog::embedded();
        let mut ids: Vec<_> = cat.records.iter().map(|r| r.id.clone()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "record ids must be unique");
        let fams: Vec<_> = cat.records.iter().map(|r| r.family.as_str()).collect();
        let mut sorted = fams.clone();
        sorted.sort();
        assert_eq!(fams, sorted, "records must stay family-sorted (binary search / UI assumptions)");
    }

    #[test]
    fn lookup_by_id() {
        let cat = Catalog::embedded();
        let inter = cat.get("inter").expect("inter must exist in the generated catalog");
        assert_eq!(inter.family, "Inter");
        assert!(!inter.weights.is_empty());
    }

    /* ---- M8: disk catalog ---- */

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("typecase-cat-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_disk_catalog_falls_back_to_embedded() {
        let dir = tmpdir("missing");
        let cat = Catalog::for_dir(&dir);
        assert_eq!(cat.records.len(), Catalog::embedded().records.len());
    }

    #[test]
    fn disk_catalog_roundtrips_and_takes_precedence() {
        let dir = tmpdir("disk");
        let mut slim = Catalog::embedded().records;
        slim.truncate(50);
        save_to_disk(&dir, &slim).unwrap();

        let cat = Catalog::for_dir(&dir);
        assert_eq!(cat.records.len(), 50, "a present disk catalog wins over embedded");
        // Every field survives the roundtrip, including the M8 flag.
        let emb = Catalog::embedded();
        for (a, b) in cat.records.iter().zip(emb.records.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.removed_from_source, b.removed_from_source);
        }
    }

    #[test]
    fn corrupt_disk_catalog_falls_back_to_embedded() {
        let dir = tmpdir("corrupt");
        fs::create_dir_all(dir.join(DISK_CATALOG_DIR)).unwrap();
        fs::write(dir.join(DISK_CATALOG_DIR).join(DISK_CATALOG_FILE), "{ junk").unwrap();
        let cat = Catalog::for_dir(&dir);
        assert_eq!(cat.records.len(), Catalog::embedded().records.len());
    }

    #[test]
    fn save_marks_nothing_removed_by_itself() {
        // The apply merge sets the flag; persistence just records what it got.
        let dir = tmpdir("save");
        let mut recs = vec![Catalog::embedded().records[0].clone()];
        recs[0].removed_from_source = true;
        save_to_disk(&dir, &recs).unwrap();
        let cat = Catalog::for_dir(&dir);
        assert!(cat.records[0].removed_from_source, "the §16 flag persists");
    }
}
