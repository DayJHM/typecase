/* M8 catalog refresh (CONTEXT.md §14–16, M3_CATALOG_DESIGN §11).

   The M3 build-time generator (tools/generate-catalog.mjs) is ported here so
   the runtime refresh performs the SAME merge: the Google Fonts metadata
   endpoint + the curated editorial layer → candidate FontRecords. The
   candidate is only diffed and notified about (§15); it replaces the active
   catalog solely through the explicit user-confirmed apply path.

   §16 policy lives in diff/apply: a family the source no longer lists is
   marked removedFromSource and NEVER deleted — cache, install state and
   every operation on them survive a refresh. */

use crate::library::model::FontRecord;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

/// Metadata endpoint from M3 (keyless, no auth — M3_CATALOG_DESIGN §2).
pub const METADATA_URL: &str = "https://fonts.google.com/metadata/fonts";

pub const CURATED_NOTES: &str = include_str!("../../../tools/curated-notes.json");

/* ---- id slug (§4 of the M3 design; byte-identical to the JS generator) ---- */

/// family → NFKD → strip combining marks → [^a-z0-9]+ → "-" → trim "-".
pub fn slug(family: &str) -> String {
    let nfkd: String = family.nfkd().collect();
    let mut out = String::with_capacity(nfkd.len());
    let mut last_dash = true; // suppresses a leading dash
    for ch in nfkd.chars() {
        if ch.is_ascii_alphanumeric() {
            let lower = ch.to_ascii_lowercase();
            out.push(lower);
            last_dash = false;
        } else if matches!(ch, '\u{0300}'..='\u{036F}') {
            // Combining marks are stripped exactly like the generator's
            // regex — they must not become dashes (ñ → n, not n-andu).
            continue;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/* ---- the endpoint payload (only the fields the merge reads) ---- */

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointFamily {
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub designers: Vec<String>,
    #[serde(default)]
    pub date_added: String,
    #[serde(default)]
    pub fonts: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub popularity: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointPayload {
    #[serde(default)]
    pub family_metadata_list: Vec<EndpointFamily>,
}

/// Parse the endpoint's JSON, tolerating the junk `)]}'` prefix line the
/// service puts before the payload (same behaviour as the M3 generator).
pub fn parse_payload(raw: &str) -> Result<EndpointPayload, String> {
    let body = if raw.trim_start().starts_with(")]}") {
        raw.split_once('\n')
            .map(|(_, rest)| rest)
            .unwrap_or(raw)
    } else {
        raw
    };
    serde_json::from_str(body).map_err(|e| format!("metadata payload failed to parse: {e}"))
}

/* ---- merge rules (ported from the generator) ---- */

/// Google's five categories map onto Typecase's taxonomy (M3 §5). Unknown
/// categories fall back to Sans — the endpoint only ever sends the five.
pub fn map_category(g: &str) -> &'static str {
    match g {
        "Sans Serif" => "Sans",
        "Serif" => "Serif",
        "Display" => "Display",
        "Monospace" => "Mono",
        "Handwriting" => "Script",
        _ => "Sans",
    }
}

/// Weight derivation from the named-instance map (M3 §6). Keys look like
/// "400", "700i" — the integer prefix is the weight; a trailing i is italic.
pub fn derive_weights(fonts: &HashMap<String, serde_json::Value>) -> (Vec<u16>, bool) {
    let mut weights: Vec<u16> = Vec::new();
    let mut italic = false;
    for key in fonts.keys() {
        let (digits, has_i) = match key.as_bytes().strip_suffix(b"i") {
            Some(prefix) => (prefix, true),
            None => (key.as_bytes(), false),
        };
        italic |= has_i;
        if let Ok(w) = std::str::from_utf8(digits).unwrap_or("").parse::<u16>() {
            weights.push(w);
        }
    }
    weights.sort_unstable();
    weights.dedup();
    if weights.is_empty() {
        weights.push(400);
    }
    (weights, italic)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CuratedRow {
    pub family: String,
    #[serde(default)]
    pub designer: String,
    #[serde(default)]
    pub year: u16,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub pairs_with: String,
}

fn curated_map() -> HashMap<String, CuratedRow> {
    let rows: Vec<CuratedRow> = serde_json::from_str(CURATED_NOTES).unwrap_or_default();
    rows.into_iter().map(|r| (r.family.clone(), r)).collect()
}

/// One metadata row → a FontRecord, with the curated overlay applied
/// (curated values win per field; generated values fill the rest).
pub fn build_record(f: &EndpointFamily, curated: Option<&CuratedRow>) -> FontRecord {
    let (weights, italic) = derive_weights(&f.fonts);
    let year_from_source = f
        .date_added
        .get(..4)
        .and_then(|y| y.parse::<u16>().ok())
        .unwrap_or(0);
    FontRecord {
        id: slug(&f.family),
        family: f.family.clone(),
        category: map_category(&f.category).to_string(),
        designer: curated
            .map(|c| c.designer.clone())
            .unwrap_or_else(|| f.designers.join(", ")),
        year: curated.map(|c| c.year).unwrap_or(year_from_source),
        styles: f.fonts.len().min(u16::MAX as usize) as u16,
        weights,
        italic,
        note: curated.map(|c| c.note.clone()).unwrap_or_default(),
        pairs_with: curated.map(|c| c.pairs_with.clone()).unwrap_or_default(),
        popularity: f.popularity.as_u64().unwrap_or(0).min(u32::MAX as u64) as u32,
        removed_from_source: false,
    }
}

/// The full merge: endpoint rows + curated overlay → candidate records in
/// the canonical order (family, code-point sort — the ordering contract of
/// M3_CATALOG_DESIGN §8; ids stay unsorted, `Catalog::get` is a linear scan).
pub fn build_candidate(payload: &EndpointPayload) -> Vec<FontRecord> {
    let curated = curated_map();
    let mut records: Vec<FontRecord> = payload
        .family_metadata_list
        .iter()
        .map(|f| build_record(f, curated.get(&f.family)))
        .collect();
    records.sort_by(|a, b| a.family.cmp(&b.family));
    records
}

/* ---- diff (§15 notification data; §16 classification) ---- */

/// What a refresh discovered, relative to the active catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshDiff {
    pub total: usize,
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
    /// Sampled family names for the notice (capped to keep the payload small).
    pub added_names: Vec<String>,
    pub removed_names: Vec<String>,
    /// True when the candidate is byte-equivalent to the active catalog
    /// (no added/changed/removed families) — nothing to apply.
    pub unchanged: bool,
}

/// Compare candidate records with the currently active ones. Metadata
/// equality is whole-record: any field change (new weights, renamed designer,
/// new popularity) counts as `changed`.
pub fn diff_catalog(active: &[FontRecord], candidate: &[FontRecord]) -> RefreshDiff {
    let active_by_id: HashMap<&str, &FontRecord> =
        active.iter().map(|r| (r.id.as_str(), r)).collect();
    let candidate_ids: std::collections::HashSet<&str> =
        candidate.iter().map(|r| r.id.as_str()).collect();

    let mut added_names: Vec<String> = Vec::new();
    let mut changed = 0usize;
    for rec in candidate {
        match active_by_id.get(rec.id.as_str()) {
            None => {
                if added_names.len() < 12 {
                    added_names.push(rec.family.clone());
                }
            }
            Some(old) => {
                let same = old.category == rec.category
                    && old.designer == rec.designer
                    && old.year == rec.year
                    && old.styles == rec.styles
                    && old.weights == rec.weights
                    && old.italic == rec.italic
                    && old.popularity == rec.popularity;
                if !same {
                    changed += 1;
                }
            }
        }
    }

    let mut removed_names: Vec<String> = Vec::new();
    for old in active {
        if !candidate_ids.contains(old.id.as_str()) {
            if removed_names.len() < 12 {
                removed_names.push(old.family.clone());
            }
        }
    }

    // Recount precisely (the capped name lists are for display only).
    let added_total = candidate
        .iter()
        .filter(|r| !active_by_id.contains_key(r.id.as_str()))
        .count();
    let removed_total = active
        .iter()
        .filter(|r| !candidate_ids.contains(r.id.as_str()))
        .count();

    RefreshDiff {
        total: candidate.len(),
        added: added_total,
        changed,
        removed: removed_total,
        added_names,
        removed_names,
        unchanged: added_total == 0 && changed == 0 && removed_total == 0,
    }
}

/* ---- apply (§16): the only write path, user-confirmed ---- */

/// Merge the candidate into the active set: updated/new records replace
/// theirs; families absent from the candidate KEEP their current record with
/// `removedFromSource: true` — never deleted, state untouched (§16).
/// Existing removedFromSource flags on families the source lists again are
/// cleared (it is back). Records already marked removed stay marked (the
/// candidate does not carry the flag) — an apply is idempotent.
pub fn apply_merge(active: &[FontRecord], candidate: &[FontRecord]) -> Vec<FontRecord> {
    let candidate_by_id: HashMap<&str, &FontRecord> =
        candidate.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut out: Vec<FontRecord> = Vec::with_capacity(candidate.len());
    for rec in candidate {
        out.push(rec.clone());
    }
    for old in active {
        match candidate_by_id.get(old.id.as_str()) {
            Some(_new) => {} // the candidate's record wins (it was just pushed)
            None => {
                let mut removed = old.clone();
                removed.removed_from_source = true;
                out.push(removed);
            }
        }
    }
    out.sort_by(|a, b| a.family.cmp(&b.family));
    out
}

/* ---- tests ---- */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::model::FontRecord;

    fn row(family: &str, weights: &[&str], category: &str) -> EndpointFamily {
        let mut fonts = HashMap::new();
        for w in weights {
            fonts.insert(w.to_string(), serde_json::json!({}));
        }
        EndpointFamily {
            family: family.into(),
            category: category.into(),
            designers: vec!["A Designer".into()],
            date_added: "2020-05-06".into(),
            fonts,
            popularity: serde_json::json!(42),
        }
    }

    #[test]
    fn slug_matches_the_m3_generator_on_the_whole_catalog() {
        // Parity contract with tools/generate-catalog.mjs §4: every embedded
        // record's id must be reproducible from its family name.
        let cat = crate::catalog::Catalog::embedded();
        assert!(cat.records.len() > 1000);
        let mismatches: Vec<&str> = cat
            .records
            .iter()
            .filter(|r| slug(&r.family) != r.id)
            .map(|r| r.family.as_str())
            .collect();
        assert!(
            mismatches.is_empty(),
            "slug drift on {}/{} families, e.g. {:?}",
            mismatches.len(),
            cat.records.len(),
            &mismatches[..mismatches.len().min(5)]
        );
    }

    #[test]
    fn slug_handles_the_documented_shapes() {
        assert_eq!(slug("Inter"), "inter");
        assert_eq!(slug("Bodoni Moda"), "bodoni-moda");
        assert_eq!(slug("ABeeZee"), "abeezee");
        // diacritics collapse through NFKD + combining-mark strip
        assert_eq!(slug("Passero One"), "passero-one");
        assert_eq!(slug("Ñandú Extra"), "nandu-extra");
        // æ does not NFKD-decompose, so it is not an a–z char for either the
        // JS generator's regex or this port — it becomes the same dash.
        assert_eq!(slug("Àæ—x"), "a-x");
        // runs of punctuation collapse to one dash; no leading/trailing dash
        assert_eq!(slug("  --Weird__Name!!--  "), "weird-name");
    }

    #[test]
    fn payload_parses_with_and_without_junk_prefix() {
        let json = r#"{"familyMetadataList":[{"family":"Inter","category":"Sans Serif","designers":["X"],"dateAdded":"2016-01-01","fonts":{"400":{},"400i":{}},"popularity":5}]}"#;
        let plain = parse_payload(json).unwrap();
        assert_eq!(plain.family_metadata_list.len(), 1);
        // The junk prefix itself ends in a `}` — concatenation, not format!.
        let prefixed = ")]}'\n".to_string() + json;
        let prefixed = parse_payload(&prefixed).unwrap();
        assert_eq!(prefixed.family_metadata_list.len(), 1);
        assert!(parse_payload("{ not json").is_err());
    }

    #[test]
    fn weights_derive_from_instance_keys() {
        let (w, italic) = derive_weights(&row("X", &["100", "400", "400i", "700i", "900"], "Sans").fonts);
        assert_eq!(w, vec![100, 400, 700, 900]);
        assert!(italic);
        let (w0, italic0) = derive_weights(&HashMap::new());
        assert_eq!(w0, vec![400], "empty instance map falls back to 400");
        assert!(!italic0);
    }

    #[test]
    fn categories_map_onto_the_taxonomy() {
        assert_eq!(map_category("Sans Serif"), "Sans");
        assert_eq!(map_category("Handwriting"), "Script");
        assert_eq!(map_category("Something New"), "Sans", "unknown falls back, never panics");
    }

    #[test]
    fn build_record_applies_the_curated_overlay() {
        let payload = EndpointPayload {
            family_metadata_list: vec![row("Inter", &["400", "700"], "Sans Serif")],
        };
        let candidate = build_candidate(&payload);
        assert_eq!(candidate.len(), 1);
        let inter = &candidate[0];
        assert_eq!(inter.id, "inter");
        assert_eq!(inter.category, "Sans");
        // Inter IS in the curated layer — its curated designer wins.
        assert_eq!(inter.designer, "Rasmus Andersson");
        assert!(!inter.note.is_empty(), "curated note travels into the record");
    }

    #[test]
    fn candidate_preserves_the_family_code_point_order() {
        let payload = EndpointPayload {
            family_metadata_list: vec![
                row("beta", &["400"], "Serif"),
                row("Alpha", &["400"], "Serif"),
                row("aB", &["400"], "Serif"),
            ],
        };
        let candidate = build_candidate(&payload);
        let fams: Vec<&str> = candidate.iter().map(|r| r.family.as_str()).collect();
        assert_eq!(fams, vec!["Alpha", "aB", "beta"], "byte order: uppercase sorts before lowercase (localeCompare would disagree)");
    }

    fn rec(id: &str, family: &str, weight: u16) -> FontRecord {
        FontRecord {
            id: id.into(),
            family: family.into(),
            category: "Sans".into(),
            designer: String::new(),
            year: 2020,
            styles: 2,
            weights: vec![weight],
            italic: false,
            note: String::new(),
            pairs_with: String::new(),
            popularity: 0,
            removed_from_source: false,
        }
    }

    #[test]
    fn diff_counts_added_changed_removed() {
        let active = vec![rec("keep", "Keep", 400), rec("gone", "Gone", 400)];
        let mut candidate = vec![rec("keep", "Keep", 400), rec("new", "New", 400)];
        candidate[0].popularity = 7; // changed metadata

        let d = diff_catalog(&active, &candidate);
        assert_eq!(d.added, 1);
        assert_eq!(d.changed, 1);
        assert_eq!(d.removed, 1);
        assert_eq!(d.added_names, vec!["New"]);
        assert_eq!(d.removed_names, vec!["Gone"]);
        assert!(!d.unchanged);
    }

    #[test]
    fn diff_reports_unchanged_when_metadata_is_identical() {
        let active = vec![rec("a", "Aa", 400)];
        let candidate = vec![rec("a", "Aa", 400)];
        assert!(diff_catalog(&active, &candidate).unchanged);
    }

    #[test]
    fn apply_marks_removed_and_keeps_everything_else() {
        let active = vec![rec("keep", "Keep", 400), rec("gone", "Gone", 400)];
        let candidate = vec![rec("keep", "Keep", 400), rec("new", "New", 400)];
        let merged = apply_merge(&active, &candidate);

        let gone = merged.iter().find(|r| r.id == "gone").expect("removed family is NEVER deleted (§16)");
        assert!(gone.removed_from_source);
        assert_eq!(gone.family, "Gone");
        assert!(merged.iter().any(|r| r.id == "new"));
        assert!(merged.iter().any(|r| r.id == "keep" && !r.removed_from_source));

        // Order contract survives the merge.
        let fams: Vec<&str> = merged.iter().map(|r| r.family.as_str()).collect();
        let mut sorted = fams.clone();
        sorted.sort();
        assert_eq!(fams, sorted);
    }

    #[test]
    fn apply_clears_the_flag_when_the_source_lists_it_again() {
        let mut active = rec("back", "Back", 400);
        active.removed_from_source = true;
        let candidate = vec![rec("back", "Back", 400)];
        let merged = apply_merge(&[active], &candidate);
        assert!(!merged[0].removed_from_source, "a returned family is available again");
    }

    #[test]
    fn apply_is_idempotent_for_already_removed_families() {
        let mut active = rec("gone", "Gone", 400);
        active.removed_from_source = true;
        let merged = apply_merge(&[active.clone()], &[]);
        assert_eq!(merged.len(), 1);
        assert!(merged[0].removed_from_source);
    }

    #[test]
    fn whole_bundle_end_to_end() {
        // Simulate the full M8 flow: refresh against a payload where one
        // family vanished and one gained a weight.
        let mut f = row("Keep", &["400"], "Sans Serif");
        f.family = "Keep".into();
        let payload_v1 = EndpointPayload { family_metadata_list: vec![f.clone(), row("Gone", &["400"], "Serif")] };
        let v1 = build_candidate(&payload_v1);

        let payload_v2 = EndpointPayload {
            family_metadata_list: vec![{
                let mut k = f;
                k.fonts.insert("700".into(), serde_json::json!({}));
                k
            }],
        };
        let v2 = build_candidate(&payload_v2);

        let d = diff_catalog(&v1, &v2);
        assert_eq!(d.removed, 1);
        assert_eq!(d.changed, 1);
        assert_eq!(d.added, 0);

        let merged = apply_merge(&v1, &v2);
        let gone = merged.iter().find(|r| r.family == "Gone").unwrap();
        assert!(gone.removed_from_source);
        let keep = merged.iter().find(|r| r.family == "Keep").unwrap();
        assert_eq!(keep.weights, vec![400, 700]);
        assert!(!keep.removed_from_source);
    }
}
