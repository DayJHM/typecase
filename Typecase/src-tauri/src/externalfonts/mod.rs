/* M7: external font discovery, ownership, removal warning (CONTEXT §19–20,
   §25; M7_EXTERNAL_FONTS_DESIGN.md).

   This file: the payload types and the PURE logic (registry value-name
   parsing, ownership classification, record-driven) — unit-tested on every
   platform. The Windows registry enumeration lives in windows_imp.rs
   (cfg-gated); other platforms compile with an empty enumeration and honest
   command errors, exactly like the M6 FontManager split. */

use serde::{Deserialize, Serialize};
use std::path::Path;

/// §25 step 3 / §43 flow 3: Typecase-held copies of external fonts.
pub mod cache;

#[cfg(windows)]
pub mod windows_imp;

/* ---- payload (§29 get_installed_fonts) ---- */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalFont {
    /// Raw registry value name, e.g. "Inter Bold (TrueType)".
    pub value_name: String,
    pub family: String,
    pub style: String,
    /// Path from the registry data, when readable.
    pub file_path: String,
    pub scope: String, // "user" | "system"
    /// record-based classification (§20) — never inferred from visibility.
    pub ownership: Ownership,
    /// Typecase face id for managed fonts; None otherwise.
    pub id: Option<String>,
    /// §25 step 3: the cache id of the copy Typecase holds for this entry
    /// (`ext-<slug>`, M9), or None when no copy has been made yet.
    pub cached_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ownership {
    /// An install record exists in state/installed.json.
    Managed,
    /// Registered in Windows, no Typecase record (§25 removal flow applies).
    External,
    /// Reserved for content-hash matching (§18) — not produced yet.
    Unknown,
}

/* ---- pure: registry value-name parsing ---- */

/// Split a raw value name into (family, style). Strips the type marker and a
/// trailing style token; unparseable names degrade gracefully (whole string
/// as family, "Regular" style) so enumeration is never fatal.
pub fn parse_value_name(value_name: &str) -> (String, String) {
    let base = value_name
        .trim()
        .trim_end_matches(" (TrueType)")
        .trim_end_matches(" (OpenType)")
        .trim();
    match style_suffix(base) {
        Some((family, style)) => (family.trim_end().to_string(), style.to_string()),
        None => (base.to_string(), "Regular".to_string()),
    }
    // Note: "Serif POSIX (OpenType)" keeps its full name as family — an
    // OpenType marker carries no style information.
}

/// Recognized style suffixes mirror the M6 writer and common Windows practice.
fn style_suffix(base: &str) -> Option<(&str, &str)> {
    // Longest-match order: "Bold Italic", numeric-weight compound ("900
    // Italic"), then "Italic"/"Bold", then a plain numeric weight ("300").
    // This keeps "Inter 900 Italic" from splitting as ("Inter 900", "Italic").
    const STYLES: [&str; 2] = ["Bold Italic", "Italic"];
    for st in STYLES {
        if let Some(family) = base.strip_suffix(&format!(" {st}")) {
            // "Inter 900 Italic": re-check whether the remaining tail ends
            // in a weight, and prefer the compound numeric style.
            if st == "Italic" {
                if let Some((fam, w)) = family.rsplit_once(' ') {
                    if is_weight(w) {
                        return Some((fam, &base[base.len() - (w.len() + " Italic".len())..]));
                    }
                }
            }
            return Some((family, st));
        }
    }
    if let Some(family) = base.strip_suffix(" Bold") {
        return Some((family, "Bold"));
    }
    let mut it = base.rsplitn(2, ' ');
    let last = it.next()?;
    let rest = it.next()?;
    if is_weight(last) {
        return Some((rest, last));
    }
    None
}

fn is_weight(s: &str) -> bool {
    matches!(s, "100" | "200" | "300" | "400" | "500" | "600" | "700" | "800" | "900")
}

/* ---- pure: ownership classification (§20 — record-based) ---- */

/// Classify one registry entry against Typecase's install records. A record
/// entry is "ours" when its registry value name appears in the record — the
/// same contract M6's uninstaller reverses by.
pub fn classify(
    value_name: &str,
    records: &std::collections::HashMap<String, crate::fontmanager::InstallRecord>,
) -> (Ownership, Option<String>) {
    for (id, rec) in records {
        if rec.entries.iter().any(|e| e.value_name == value_name) {
            return (Ownership::Managed, Some(id.clone()));
        }
    }
    (Ownership::External, None)
}

/// Every registered font on this platform, classified against `records` and
/// annotated with the id of any copy Typecase already holds.
/// Non-Windows platforms return an empty list (§42: no font management there).
pub fn discover(
    records: &std::collections::HashMap<String, crate::fontmanager::InstallRecord>,
    data_dir: &Path,
) -> Vec<ExternalFont> {
    #[cfg(windows)]
    {
        let mut rows = windows_imp::discover(records);
        annotate_cached(&mut rows, data_dir);
        rows
    }
    #[cfg(not(windows))]
    {
        let _ = (records, data_dir);
        Vec::new()
    }
}

/// Attach the cache id of Typecase's own copy (§25 step 3) to each row, matched
/// by registry value name — the same identity the M6 record path reverses by.
/// Derived from the cache manifests, so a copy deleted outside Typecase simply
/// stops being reported.
pub fn annotate_cached(rows: &mut [ExternalFont], data_dir: &Path) {
    let inventory = cache::inventory(data_dir);
    for row in rows.iter_mut() {
        row.cached_id = inventory.get(&row.value_name).cloned();
    }
}

/// §31: caching accepts a value name and path from the UI, but only for a font
/// Windows actually has registered. The triple is verified against live
/// discovery before any file is read, so the command can never be turned into a
/// generic copy interface (§30). Windows paths are case-insensitive, so the
/// path is compared case-folded; value name and scope must match exactly.
pub fn resolve_registered<'a>(
    rows: &'a [ExternalFont],
    value_name: &str,
    file_path: &str,
    scope: &str,
) -> Result<&'a ExternalFont, String> {
    rows.iter()
        .find(|r| {
            r.value_name == value_name && r.scope == scope && r.file_path.eq_ignore_ascii_case(file_path)
        })
        .ok_or_else(|| {
            format!(
                "{value_name} is not registered in Windows ({scope} scope) — only a font Windows has installed can be cached"
            )
        })
}

/// Remove one external font registration: delete the registry value and the
/// file it points to. System scope elevates via the M6 one-shot helper.
/// Only for fonts WITHOUT a Typecase record — managed fonts go through the
/// record-based uninstall (§20).
pub fn remove_external(
    scope: crate::library::model::Scope,
    value_name: &str,
    file_path: &str,
) -> Result<(), String> {
    if value_name.trim().is_empty() {
        return Err("empty registry value name".into());
    }
    #[cfg(windows)]
    {
        windows_imp::remove_external(scope, value_name, file_path)
    }
    #[cfg(not(windows))]
    {
        let _ = (scope, file_path);
        Err("font removal is only supported on Windows 10/11".into())
    }
}

/// System-scope external removal through the one-shot elevation helper (§23).
pub fn remove_external_elevated(value_name: &str, file_path: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_imp::remove_external_elevated(value_name, file_path)
    }
    #[cfg(not(windows))]
    {
        let _ = (value_name, file_path);
        Err("font removal is only supported on Windows 10/11".into())
    }
}

/// Dispatches elevation-only subcommands for this module (M6 main.rs hook
/// pattern). Returns Some(exit code) when this process is an elevated
/// external-removal helper.
pub fn run_elevated_from_args() -> Option<i32> {
    #[cfg(windows)]
    {
        windows_imp::run_elevated_external_from_args()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fontmanager::{plan_install, InstallRecord};
    use crate::library::model::Scope;
    use std::collections::HashMap;

    fn records() -> HashMap<String, InstallRecord> {
        let mut m = HashMap::new();
        m.insert(
            "inter".into(),
            plan_install(
                "inter",
                "Inter",
                Scope::User,
                "2026-09-27T00:00:00Z",
                &[
                    (400, "normal".into(), "C:\\cache\\a.ttf".into()),
                    (400, "italic".into(), "C:\\cache\\b.ttf".into()),
                ],
                |n| format!("C:\\Users\\t\\Fonts\\{n}"),
            ),
        );
        m
    }

    #[test]
    fn parses_type_markers_and_styles() {
        assert_eq!(
            parse_value_name("Inter Bold (TrueType)"),
            ("Inter".into(), "Bold".into())
        );
        assert_eq!(
            parse_value_name("Inter Italic (TrueType)"),
            ("Inter".into(), "Italic".into())
        );
        assert_eq!(
            parse_value_name("Inter Bold Italic (TrueType)"),
            ("Inter".into(), "Bold Italic".into())
        );
        // An OpenType marker carries no style info — the full name stays.
        assert_eq!(
            parse_value_name("Serif POSIX (OpenType)"),
            ("Serif POSIX".into(), "Regular".into())
        );
        assert_eq!(
            parse_value_name("Inter (TrueType)"),
            ("Inter".into(), "Regular".into())
        );
    }

    #[test]
    fn parses_numeric_weight_styles() {
        assert_eq!(
            parse_value_name("Inter 300 (TrueType)"),
            ("Inter".into(), "300".into())
        );
        assert_eq!(
            parse_value_name("Inter 900 Italic (TrueType)"),
            ("Inter".into(), "900 Italic".into())
        );
        // A family that merely ends in digits stays intact.
        assert_eq!(
            parse_value_name("Foo 42 Bar (TrueType)"),
            ("Foo 42 Bar".into(), "Regular".into())
        );
    }

    #[test]
    fn unparseable_degrades_without_panic() {
        assert_eq!(parse_value_name(""), ("".into(), "Regular".into()));
        assert_eq!(
            parse_value_name("(TrueType)"),
            ("(TrueType)".into(), "Regular".into())
        );
    }

    fn external_row(value_name: &str, family: &str, path: &str, scope: &str) -> ExternalFont {
        ExternalFont {
            value_name: value_name.into(),
            family: family.into(),
            style: "Regular".into(),
            file_path: path.into(),
            scope: scope.into(),
            ownership: Ownership::External,
            id: None,
            cached_id: None,
        }
    }

    #[test]
    fn cached_ids_come_from_the_cache_manifests() {
        let dir = std::env::temp_dir().join(format!("typecase-annot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut rows = vec![
            external_row("Inter (TrueType)", "Inter", "C:\\W\\Inter.ttf", "user"),
            external_row("Other (TrueType)", "Other", "C:\\W\\Other.ttf", "user"),
        ];
        // Nothing cached yet.
        annotate_cached(&mut rows, &dir);
        assert!(rows.iter().all(|r| r.cached_id.is_none()));

        // A real copy makes exactly its own row report the cache id.
        let src = dir.join("Inter.ttf");
        let mut payload = vec![0x00u8, 0x01, 0x00, 0x00];
        payload.resize(1_500, 0);
        std::fs::write(&src, &payload).unwrap();
        let owner = external_row("Inter (TrueType)", "Inter", src.to_str().unwrap(), "user");
        cache::cache_registered(&dir, &owner).unwrap();
        annotate_cached(&mut rows, &dir);
        assert_eq!(rows[0].cached_id.as_deref(), Some("ext-inter"));
        assert_eq!(rows[1].cached_id, None);

        // The payload rides the wire as cachedId.
        let json = serde_json::to_value(&rows[0]).unwrap();
        assert_eq!(json["cachedId"], "ext-inter");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_registered_verifies_the_exact_triple() {
        let rows = vec![
            external_row("Inter (TrueType)", "Inter", "C:\\Windows\\Fonts\\Inter.ttf", "user"),
            external_row("Inter (TrueType)", "Inter", "C:\\Windows\\Fonts\\Inter.ttf", "system"),
        ];
        // The discovered triple resolves.
        assert!(resolve_registered(&rows, "Inter (TrueType)", "C:\\Windows\\Fonts\\Inter.ttf", "user")
            .is_ok());
        // Scope disambiguates the same value name.
        assert!(resolve_registered(&rows, "Inter (TrueType)", "C:\\Windows\\Fonts\\Inter.ttf", "system")
            .is_ok());
        // Windows paths are case-insensitive.
        assert!(resolve_registered(&rows, "Inter (TrueType)", "c:\\windows\\fonts\\inter.TTF", "user")
            .is_ok());
        // A path that is not what the registry says is refused (§31): the
        // command cannot be pointed at an arbitrary file.
        let err = resolve_registered(&rows, "Inter (TrueType)", "C:\\Users\\t\\secrets.ttf", "user")
            .unwrap_err();
        assert!(err.contains("not registered"), "unexpected error: {err}");
        // Unknown fonts and unknown scopes are refused too.
        assert!(resolve_registered(&rows, "Nope (TrueType)", "C:\\Windows\\Fonts\\Inter.ttf", "user").is_err());
        assert!(resolve_registered(&rows, "Inter (TrueType)", "C:\\Windows\\Fonts\\Inter.ttf", "elsewhere").is_err());
        assert!(resolve_registered(&[], "Inter (TrueType)", "C:\\Windows\\Fonts\\Inter.ttf", "user").is_err());
    }

    #[test]
    fn classification_is_record_based() {
        let r = records();
        // M6 wrote "Inter (TrueType)" and "Inter Italic (TrueType)".
        let (own, id) = classify("Inter (TrueType)", &r);
        assert_eq!(own, Ownership::Managed);
        assert_eq!(id.as_deref(), Some("inter"));
        let (own, id) = classify("Inter Italic (TrueType)", &r);
        assert_eq!(own, Ownership::Managed);
        assert_eq!(id.as_deref(), Some("inter"));
        // Anything else is external — even if it looks like ours.
        let (own, id) = classify("Inter Bold (TrueType)", &r);
        assert_eq!(own, Ownership::External);
        assert_eq!(id, None);
        // Empty records ⇒ everything external.
        let (own, _) = classify("Inter (TrueType)", &HashMap::new());
        assert_eq!(own, Ownership::External);
    }
}
