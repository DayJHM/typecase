/* Typecase font record — the backend-normalized, source-agnostic model.
   Serialized camelCase over IPC; the TS mirror lives in src/data/types.ts. */

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    User,
    System,
}

/// Static catalog record: what the source knows about a family.
/// Produced by tools/generate-catalog.mjs (CONTEXT.md §14).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontRecord {
    pub id: String,
    pub family: String,
    pub category: String,
    #[serde(default)]
    pub designer: String,
    #[serde(default)]
    pub year: u16,
    #[serde(default)]
    pub styles: u16,
    #[serde(default)]
    pub weights: Vec<u16>,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub pairs_with: String,
    #[serde(default)]
    pub popularity: u32,
}

/// Per-face library state, owned by the backend (CONTEXT.md §17).
/// Persisted in state/library.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryState {
    pub cached: bool,
    pub installed: bool,
    /// None = unknown (externally installed vs Typecase-managed is resolved from M6 on).
    pub managed_by_typecase: Option<bool>,
    pub install_scope: Option<Scope>,
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            cached: false,
            installed: false,
            managed_by_typecase: None,
            install_scope: None,
        }
    }
}

impl LibraryState {
    /// Coarse lifecycle phase for the shell views. Mirrors the frontend's
    /// `Phase` type exactly so the UI switch is unaffected (STAGE2_PLAN §2).
    pub fn phase(&self) -> &'static str {
        match (self.cached, self.installed) {
            (_, true) => "installed",
            (true, false) => "library",
            (false, false) => "online",
        }
    }
}

/// IPC payload: catalog record + live library state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceStatus {
    #[serde(flatten)]
    pub record: FontRecord,
    #[serde(flatten)]
    pub state: FlatState,
}

/// Flattened view of LibraryState + derived phase for the wire format
/// (the frontend Face type carries phase/local at the top level).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlatState {
    pub cached: bool,
    pub installed: bool,
    pub managed_by_typecase: Option<bool>,
    pub install_scope: Option<Scope>,
    pub phase: &'static str,
    pub local: bool,
}

impl From<&LibraryState> for FlatState {
    fn from(s: &LibraryState) -> Self {
        Self {
            cached: s.cached,
            installed: s.installed,
            managed_by_typecase: s.managed_by_typecase,
            install_scope: s.install_scope,
            phase: s.phase(),
            local: s.cached,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_matches_frontend_type() {
        assert_eq!(LibraryState::default().phase(), "online");
        assert_eq!(
            LibraryState { cached: true, ..Default::default() }.phase(),
            "library"
        );
        assert_eq!(
            LibraryState {
                cached: true,
                installed: true,
                ..Default::default()
            }
            .phase(),
            "installed"
        );
        // installed-but-not-cached still reads as installed for the shell
        assert_eq!(
            LibraryState { installed: true, ..Default::default() }.phase(),
            "installed"
        );
    }

    #[test]
    fn serializes_camel_case() {
        let st = LibraryState {
            cached: true,
            managed_by_typecase: Some(true),
            install_scope: Some(Scope::User),
            ..Default::default()
        };
        let json = serde_json::to_string(&st).unwrap();
        assert!(json.contains("managedByTypecase"));
        assert!(json.contains("installScope"));
        assert!(!json.contains("managed_by_typecase"));
    }

    #[test]
    fn face_status_flattens() {
        let rec = FontRecord {
            id: "inter".into(),
            family: "Inter".into(),
            category: "Sans".into(),
            designer: "Rasmus Andersson".into(),
            year: 2016,
            styles: 18,
            weights: vec![400, 700],
            italic: true,
            note: "workhorse".into(),
            pairs_with: "Newsreader".into(),
            popularity: 1,
        };
        let fs = FaceStatus {
            record: rec,
            state: FlatState::from(&LibraryState::default()),
        };
        let json = serde_json::to_value(&fs).unwrap();
        assert_eq!(json["family"], "Inter");
        assert_eq!(json["phase"], "online");
        assert_eq!(json["pairsWith"], "Newsreader");
        assert!(json.get("record").is_none()); // flattened, not nested
    }
}
