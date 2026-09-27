/* Catalog loading (CONTEXT.md §14): the generated catalog.json is embedded at
   compile time as the bundled snapshot. Disk-level catalog refresh arrives with
   M8; the on-disk location is reserved but not read yet. */

use crate::library::model::FontRecord;
use serde::Deserialize;

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

    pub fn get(&self, id: &str) -> Option<&FontRecord> {
        self.records.iter().find(|r| r.id == id)
    }
}

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
}
