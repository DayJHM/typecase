/* M7: external font discovery, ownership, removal warning (CONTEXT §19–20,
   §25; M7_EXTERNAL_FONTS_DESIGN.md).

   This file: the payload types and the PURE logic (registry value-name
   parsing, ownership classification, record-driven) — unit-tested on every
   platform. The Windows registry enumeration lives in windows_imp.rs
   (cfg-gated); other platforms compile with an empty enumeration and honest
   command errors, exactly like the M6 FontManager split. */

use serde::{Deserialize, Serialize};

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

/// Every registered font on this platform, classified against `records`.
/// Non-Windows platforms return an empty list (§42: no font management there).
pub fn discover(
    records: &std::collections::HashMap<String, crate::fontmanager::InstallRecord>,
) -> Vec<ExternalFont> {
    #[cfg(windows)]
    {
        windows_imp::discover(records)
    }
    #[cfg(not(windows))]
    {
        let _ = records;
        Vec::new()
    }
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
