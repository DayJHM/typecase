/* M6 FontManager (CONTEXT §21): the application layer talks to this
   abstraction only — Windows API details never leak upward (§21), and the
   §38 boundary is explicit: the Windows implementation is compile-validated
   by CI and runtime-validated solely by the WINDOWS_VALIDATION.md §5 VM
   session, never claimed from Linux.

   This file: shared types, the trait, and the pure helpers both platforms
   use (naming, records, plans) — the helpers carry the unit tests that run
   on every OS. Platform implementations live in windows.rs / unsupported.rs. */

use crate::library::model::Scope;
use serde::{Deserialize, Serialize};

/* ---- types ---- */

/// One concrete write the platform manager performed for a family: the
/// installed file and (Windows) the registry value name. Uninstall reverses
/// exactly these — never heuristics (§20: external fonts are never touched).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallEntry {
    /// Absolute path of the installed file.
    pub file: String,
    /// Registry value name written for this file, e.g. "Inter Italic (TrueType)".
    pub value_name: String,
    /// Absolute path of the cached source file it was copied from.
    pub source: String,
}

/// What an install/uninstall call plans or performed, per family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecord {
    pub id: String,
    pub family: String,
    pub scope: Scope,
    #[serde(rename = "registeredAt")]
    pub registered_at: String,
    pub entries: Vec<InstallEntry>,
}

/* ---- pure naming helpers (unit-tested cross-platform) ---- */

/// Human style name for a (weight, style) pair, following the convention the
/// CSS2 pipeline tags files with. Drives both filenames and registry names.
pub fn style_name(weight: u16, style: &str) -> String {
    let italic = style == "italic";
    match (weight, italic) {
        (400, false) => "Regular".into(),
        (700, false) => "Bold".into(),
        (400, true) => "Italic".into(),
        (700, true) => "Bold Italic".into(),
        (w, false) => format!("{w}"),
        (w, true) => format!("{w} Italic"),
    }
}

/// Registry value name: "<Family> <Style> (TrueType)" — the Windows convention
/// elides "Regular" for the default style ("Inter (TrueType)",
/// "Inter Bold (TrueType)").
pub fn value_name(family: &str, weight: u16, style: &str) -> String {
    let st = style_name(weight, style);
    if st == "Regular" {
        format!("{family} (TrueType)")
    } else {
        format!("{family} {st} (TrueType)")
    }
}

/// Installed filename: "<Family> <Style>.ttf". Sanitized — family/style names
/// are data, never allowed to become path segments (§31).
pub fn installed_file_name(family: &str, weight: u16, style: &str) -> String {
    let bad = |c: char| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | ' ' | '-' | '_' | '.' | '(' | ')');
    let fam: String = family.chars().filter(|c| !bad(*c)).collect();
    let st: String = style_name(weight, style).chars().filter(|c| !bad(*c)).collect();
    // Leading dots make hidden/path-hostile names ("..Evil") — strip them.
    let fam = fam.trim().trim_start_matches('.');
    let st = st.trim();
    let fam = if fam.is_empty() { "Font" } else { fam };
    if st == "Regular" {
        format!("{fam}.ttf")
    } else {
        format!("{fam} {st}.ttf")
    }
}

/// The registry hive root for a scope (string form for logging/tests; the
/// Windows impl maps this to HKEY_CURRENT_USER / HKEY_LOCAL_MACHINE).
pub fn hive_root(scope: Scope) -> &'static str {
    match scope {
        Scope::User => "HKCU",
        Scope::System => "HKLM",
    }
}

/// Directory where fonts of `scope` are installed on this platform
/// (Windows: per-user or system Fonts dir; other platforms: irrelevant,
/// the manager refuses anyway — but the path must exist for planning).
pub fn fonts_root(scope: Scope) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        windows_imp::fonts_dir(scope)
    }
    #[cfg(not(windows))]
    {
        let _ = scope;
        std::env::temp_dir().join("typecase-unsupported-fonts")
    }
}

pub mod store;

/* ---- plan construction (pure; the platform impl executes it) ---- */

/// Build the install plan for one family from its M4 cache manifest entries.
/// `targets(file_name)` maps a sanitized file name to its absolute installed
/// path (platform-provided), keeping path construction out of the pure layer.
pub fn plan_install(
    id: &str,
    family: &str,
    scope: Scope,
    registered_at: &str,
    files: &[(u16, String, String)], // (weight, style, cached absolute path)
    targets: impl Fn(&str) -> String,
) -> InstallRecord {
    let entries = files
        .iter()
        .map(|(w, st, src)| {
            let name = installed_file_name(family, *w, st);
            InstallEntry {
                file: targets(&name),
                value_name: value_name(family, *w, st),
                source: src.clone(),
            }
        })
        .collect();
    InstallRecord {
        id: id.to_string(),
        family: family.to_string(),
        scope,
        registered_at: registered_at.to_string(),
        entries,
    }
}

/* ---- trait (§21) ---- */

pub trait FontManager: Send + Sync {
    /// Copy + register every entry of `record` for its scope. Idempotent per
    /// (family, scope): an existing record is replaced.
    fn install(&self, record: &InstallRecord) -> Result<(), String>;
    /// Reverse exactly the entries of `record`.
    fn uninstall(&self, record: &InstallRecord) -> Result<(), String>;
    /// Whether the exact registry values of `record` are present.
    fn is_installed(&self, record: &InstallRecord) -> bool;
}

#[cfg(windows)]
pub mod windows_imp;
#[cfg(windows)]
pub use windows_imp::WindowsFontManager;

#[cfg(not(windows))]
pub mod unsupported;
#[cfg(not(windows))]
pub use unsupported::UnsupportedFontManager;

/// The manager for this platform.
pub fn manager() -> std::sync::Arc<dyn FontManager> {
    #[cfg(windows)]
    {
        std::sync::Arc::new(WindowsFontManager)
    }
    #[cfg(not(windows))]
    {
        std::sync::Arc::new(UnsupportedFontManager)
    }
}

/// Scope-aware install/uninstall: user scope runs in-process, system scope
/// re-execs elevated for that one operation (§23). Non-Windows: honest error.
pub fn install_scoped(record: &InstallRecord) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_imp::install_scoped(record)
    }
    #[cfg(not(windows))]
    {
        let _ = record;
        Err("font installation is only supported on Windows 10/11".into())
    }
}

pub fn uninstall_scoped(record: &InstallRecord) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_imp::uninstall_scoped(record)
    }
    #[cfg(not(windows))]
    {
        let _ = record;
        Err(format!(
            "font uninstallation is only supported on Windows 10/11 ({} not uninstalled)",
            record.family
        ))
    }
}

/* ---- tests (run on every platform) ---- */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_names_follow_convention() {
        assert_eq!(style_name(400, "normal"), "Regular");
        assert_eq!(style_name(700, "normal"), "Bold");
        assert_eq!(style_name(400, "italic"), "Italic");
        assert_eq!(style_name(700, "italic"), "Bold Italic");
        assert_eq!(style_name(300, "normal"), "300");
        assert_eq!(style_name(900, "italic"), "900 Italic");
    }

    #[test]
    fn value_names_match_windows_convention() {
        assert_eq!(value_name("Inter", 400, "normal"), "Inter (TrueType)");
        assert_eq!(value_name("Inter", 400, "italic"), "Inter Italic (TrueType)");
        assert_eq!(value_name("Bodoni Moda", 700, "normal"), "Bodoni Moda Bold (TrueType)");
    }

    #[test]
    fn installed_names_are_sanitized() {
        assert_eq!(installed_file_name("Inter", 400, "normal"), "Inter.ttf");
        assert_eq!(installed_file_name("Inter", 400, "italic"), "Inter Italic.ttf");
        // Path-hostile family names are defanged before touching the FS (§31).
        assert_eq!(
            installed_file_name("..\\Evil/Fam", 400, "normal"),
            "EvilFam.ttf"
        );
        assert_eq!(
            installed_file_name("A:B<C>D", 700, "italic"),
            "ABCD Bold Italic.ttf"
        );
    }

    #[test]
    fn hive_roots_map_to_scopes() {
        assert_eq!(hive_root(Scope::User), "HKCU");
        assert_eq!(hive_root(Scope::System), "HKLM");
    }

    #[test]
    fn plan_builds_entries_from_manifest_files() {
        let rec = plan_install(
            "inter",
            "Inter",
            Scope::User,
            "2026-09-27T00:00:00Z",
            &[
                (400, "normal".into(), "C:\\cache\\inter\\abcd1234.ttf".into()),
                (400, "italic".into(), "C:\\cache\\inter\\b1d784c4.ttf".into()),
            ],
            |name| format!("C:\\Users\\t\\Fonts\\{name}"),
        );
        assert_eq!(rec.entries.len(), 2);
        assert_eq!(rec.entries[0].file, "C:\\Users\\t\\Fonts\\Inter.ttf");
        assert_eq!(rec.entries[0].value_name, "Inter (TrueType)");
        assert_eq!(rec.entries[1].file, "C:\\Users\\t\\Fonts\\Inter Italic.ttf");
        assert_eq!(rec.entries[1].source, "C:\\cache\\inter\\b1d784c4.ttf");
    }

    #[test]
    fn records_roundtrip_camel_case() {
        let rec = plan_install(
            "inter",
            "Inter",
            Scope::System,
            "2026-09-27T00:00:00Z",
            &[(400, "normal".into(), "s.ttf".into())],
            |n| n.to_string(),
        );
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"registeredAt\""));
        assert!(json.contains("\"valueName\""));
        let back: InstallRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, rec);
    }
}
