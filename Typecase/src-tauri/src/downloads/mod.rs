/* M4 download pipeline (CONTEXT §11 download/cache, §18 identity, §31
   filesystem safety).

   Google Fonts CSS2 (keyless, no auth) → per-weight TTF/OTF files, streamed
   into a staging area, validated (sfnt magic + size bounds + sha256), then
   committed into fonts/<family-id>/ with content-addressed filenames and a
   metadata.json manifest. Everything is downloaded and validated before any
   commit — a failed download leaves neither files nor library state behind.
   See M4_DOWNLOAD_DESIGN.md for the recorded decisions. */

use crate::library::model::FontRecord;
use crate::library::store::States;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const FONTS_DIR: &str = "fonts";
pub const STAGING_DIR: &str = "staging";
const MIN_FILE_BYTES: u64 = 1_000;
const MAX_FILE_BYTES: u64 = 64_000_000;
pub const CSS2_BASE: &str = "https://fonts.googleapis.com/css2";

/* ---- sfnt container check (§12: TTF/OTF only) ---- */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfntKind {
    Ttf,
    Otf,
}

pub fn magic_kind(bytes: &[u8]) -> Option<SfntKind> {
    match bytes {
        [0x00, 0x01, 0x00, 0x00, ..] => Some(SfntKind::Ttf),
        [b'O', b'T', b'T', b'O', ..] => Some(SfntKind::Otf),
        _ => None,
    }
}

fn ext(kind: SfntKind) -> &'static str {
    match kind {
        SfntKind::Ttf => "ttf",
        SfntKind::Otf => "otf",
    }
}

/* ---- CSS2 response parsing ---- */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssFace {
    pub weight: u16,
    pub style: &'static str,
    pub url: String,
}

/// Extract (weight, style, url) from the @font-face blocks of a CSS2
/// response. A block contributes only when both a numeric weight and a URL
/// are present (minimal responses emit neither font-style nor font-weight).
pub fn parse_css_faces(css: &str) -> Vec<CssFace> {
    let mut out = Vec::new();
    for block in css.split("@font-face").skip(1) {
        let weight = find_key_value(block, "font-weight")
            .and_then(|v| v.trim().parse::<u16>().ok());
        let style = find_key_value(block, "font-style")
            .map(|v| if v.trim() == "italic" { "italic" } else { "normal" })
            .unwrap_or("normal");
        let url = block
            .split("url(")
            .nth(1)
            .and_then(|rest| rest.split(')').next())
            .map(|u| u.trim().trim_matches('\'').trim_matches('"').to_string());
        if let (Some(weight), Some(url)) = (weight, url) {
            if !url.is_empty() {
                out.push(CssFace { weight, style, url });
            }
        }
    }
    out
}

/// Value of `key: value;` inside a CSS block (last declaration wins, as in
/// real CSS cascade resolution).
fn find_key_value(block: &str, key: &str) -> Option<String> {
    block
        .split(';')
        .filter_map(|decl| decl.split_once(':'))
        .filter(|(k, _)| k.trim().eq_ignore_ascii_case(key))
        .next_back()
        .map(|(_, v)| v.to_string())
}

/// The CSS2 query for one face: weights deduped/sorted, italic appended.
pub fn css2_url(rec: &FontRecord) -> String {
    let mut weights: Vec<u16> = rec.weights.clone();
    weights.sort_unstable();
    weights.dedup();
    let axes = if rec.italic {
        let italics: Vec<String> = weights.iter().map(|w| format!("1,{w}")).collect();
        let normals: Vec<String> = weights.iter().map(|w| format!("0,{w}")).collect();
        format!(":ital,wght@{};{}", normals.join(";"), italics.join(";"))
    } else {
        format!(":wght@{}", weights.iter().map(|w| w.to_string()).collect::<Vec<_>>().join(";"))
    };
    format!("{CSS2_BASE}?family={}{}&display=swap", urlencode(&rec.family), axes)
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/* ---- metadata.json (M4 manifest; weight/style let M5/M6 pick files) ---- */

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheFileMeta {
    pub file: String,
    pub weight: u16,
    pub style: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontMeta {
    pub id: String,
    pub family: String,
    pub source: &'static str,
    pub downloaded_at: String,
    pub files: Vec<CacheFileMeta>,
    pub file_count: usize,
    pub total_size: u64,
}

impl FontMeta {
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join("metadata.json");
        let tmp = dir.join("metadata.json.tmp");
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialize metadata: {e}"))?;
        fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename into place: {e}"))
    }

    pub fn exists(dir: &Path) -> bool {
        dir.join("metadata.json").is_file()
    }
}

fn now_rfc3339() -> String {
    now_rfc3339_pub()
}

/// RFC3339 timestamp for any module that stamps records (downloads, installs).
pub fn now_rfc3339_pub() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // humantime's format is RFC3339-compatible (…Z).
    humantime::format_rfc3339_seconds(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
        .to_string()
}

/* ---- staging paths (unique per file before any request is made) ---- */

pub fn staged_paths(dir: &Path, id: &str, weights: &[(u16, &'static str)]) -> Vec<(PathBuf, u16, &'static str)> {
    weights
        .iter()
        .map(|(w, style)| (dir.join(STAGING_DIR).join(format!("{id}.{w}.{style}.part")), *w, *style))
        .collect()
}

/* ---- one file: stream → hash → validate ---- */

pub fn download_file(
    client: &reqwest::blocking::Client,
    url: &str,
    tmp: &Path,
) -> Result<(u64, String, SfntKind), String> {
    let mut res = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("fetch failed: {e}"))?;

    if let Some(len) = res.content_length() {
        if len > MAX_FILE_BYTES {
            return Err(format!("file too large: {len} bytes"));
        }
    }

    let mut file = fs::File::create(tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
    let mut hasher = Sha256::new();
    let mut size: u64 = 0;
    let mut buf = [0u8; 65_536];
    let mut head = [0u8; 4];
    let mut head_len = 0usize;

    loop {
        let n = res
            .read(&mut buf)
            .map_err(|e| format!("read stream: {e}"))?;
        if n == 0 {
            break;
        }
        let keep = (4 - head_len).min(n);
        head[head_len..head_len + keep].copy_from_slice(&buf[..keep]);
        head_len += keep;
        hasher.update(&buf[..n]);
        size += n as u64;
        if size > MAX_FILE_BYTES {
            return Err(format!("file exceeds {MAX_FILE_BYTES} bytes"));
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
    }

    if size < MIN_FILE_BYTES {
        return Err(format!("file too small: {size} bytes"));
    }
    let kind = magic_kind(&head[..head_len]).ok_or_else(|| {
        format!("not a TTF/OTF file (magic {:02x?})", &head[..head_len])
    })?;
    let hash = hex::encode(hasher.finalize());
    Ok((size, hash, kind))
}

pub struct DownloadOutcome {
    pub files: Vec<CacheFileMeta>,
    pub total_size: u64,
    /// (staged path, committed path, sha256, weight) for the commit step.
    pub staged: Vec<(PathBuf, PathBuf, String, u16)>,
}

/* ---- the pipeline ---- */

/// Download and validate every weight. The library-state lock must NOT be
/// held across this call (network latency); state is updated after commit.
/// On failure the staged files are deleted and an Err carries the reason.
pub fn download_all(
    client: &reqwest::blocking::Client,
    data_dir: &Path,
    rec: &FontRecord,
) -> Result<DownloadOutcome, String> {
    if FontMeta::exists(&data_dir.join(FONTS_DIR).join(&rec.id)) {
        return Err(format!("{} is already cached — delete its library entry first", rec.family));
    }

    let css = client
        .get(css2_url(rec))
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
        .map_err(|e| format!("css fetch failed: {e}"))?;

    let mut faces = parse_css_faces(&css);
    if faces.is_empty() {
        return Err("css response contained no usable @font-face blocks".into());
    }
    // One file per (weight, style); CSS2 can repeat blocks for unicode ranges.
    faces.sort_by_key(|f| (f.weight, f.style));
    faces.dedup_by_key(|f| (f.weight, f.style));

    let mut weights: Vec<(u16, &'static str)> =
        faces.iter().map(|f| (f.weight, f.style)).collect();
    weights.sort_unstable();
    weights.dedup();
    let staged_paths = staged_paths(data_dir, &rec.id, &weights);
    fs::create_dir_all(data_dir.join(STAGING_DIR))
        .map_err(|e| format!("cannot create staging dir: {e}"))?;

    let mut outcome = DownloadOutcome { files: Vec::new(), total_size: 0, staged: Vec::new() };
    for ((tmp, weight, style), face) in staged_paths.iter().zip(faces.iter()) {
        match download_file(client, &face.url, tmp) {
            Ok((size, hash, kind)) => {
                let name = format!("{}.{}", &hash[..8], ext(kind));
                outcome.staged.push((tmp.clone(), data_dir.join(FONTS_DIR).join(&rec.id).join(&name), hash.clone(), *weight));
                outcome.files.push(CacheFileMeta {
                    file: name,
                    weight: *weight,
                    style: style.to_string(),
                    size,
                    sha256: hash,
                });
                outcome.total_size += size;
            }
            Err(e) => {
                for (tmp, _, _) in &staged_paths {
                    let _ = fs::remove_file(tmp);
                }
                return Err(format!("weight {weight} {style}: {e}"));
            }
        }
    }
    Ok(outcome)
}

/// Move staged files into fonts/<id>/ and write the manifest. Renames within
/// the same volume are atomic; a failure here leaves the face uncached in the
/// library (state is only updated after this succeeds).
pub fn commit(data_dir: &Path, rec: &FontRecord, outcome: &DownloadOutcome) -> Result<FontMeta, String> {
    let face_dir = data_dir.join(FONTS_DIR).join(&rec.id);
    fs::create_dir_all(&face_dir).map_err(|e| format!("create {}: {e}", face_dir.display()))?;
    for (from, to, _, _) in &outcome.staged {
        if let Err(e) = fs::rename(from, to) {
            return Err(format!("commit {}: {e}", to.display()));
        }
    }
    let meta = FontMeta {
        id: rec.id.clone(),
        family: rec.family.clone(),
        source: "google-fonts",
        downloaded_at: now_rfc3339(),
        files: outcome.files.clone(),
        file_count: outcome.files.len(),
        total_size: outcome.total_size,
    };
    meta.save(&face_dir)?;
    Ok(meta)
}

/* ---- M5: serving cached files to the webview (M5_OFFLINE_DESIGN §2–4) ---- */

/// Deserialize mirror of the metadata.json manifest (the write side,
/// FontMeta, is Serialize-only).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredFontMeta {
    pub id: String,
    pub family: String,
    pub files: Vec<StoredFileMeta>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredFileMeta {
    pub file: String,
    pub weight: u16,
    pub style: String,
}
/// Read one family's manifest. Corrupt or missing manifests are skipped by
/// callers — never fabricated (same policy as library.json).
pub fn read_manifest(data_dir: &Path, id: &str) -> Option<StoredFontMeta> {
    let path = data_dir
        .join(FONTS_DIR)
        .join(id)
        .join("metadata.json");
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// One servable face: the backend builds the full scheme URLs so the
/// frontend never constructs filesystem paths.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontSourceFile {
    pub url: String,
    pub weight: u16,
    pub style: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontSource {
    pub id: String,
    pub family: String,
    pub files: Vec<FontSourceFile>,
}

pub const SCHEME_URL_PREFIX: &str = "font://cached/";

pub fn cache_url(id: &str, file: &str) -> String {
    format!("{SCHEME_URL_PREFIX}{id}/{file}")
}

/// Every family that has a readable manifest, as servable sources.
pub fn font_sources(data_dir: &Path) -> Vec<FontSource> {
    let fonts_dir = data_dir.join(FONTS_DIR);
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&fonts_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let Some(meta) = read_manifest(data_dir, &entry.file_name().to_string_lossy()) else {
            continue;
        };
        let files = meta
            .files
            .iter()
            .map(|f| FontSourceFile {
                url: cache_url(&meta.id, &f.file),
                weight: f.weight,
                style: f.style.clone(),
            })
            .collect();
        out.push(FontSource { id: meta.id, family: meta.family, files });
    }
    out
}

/// A scheme URI carries /cached/<family-id>/<filename>. Only the custom-scheme
/// form and WebKitGTK's http://font.localhost rewrite are accepted — any other
/// scheme is rejected outright.
pub fn parse_cache_uri(uri: &str) -> Option<(&str, &str)> {
    let rest = uri
        .strip_prefix("font://cached/")
        .or_else(|| uri.strip_prefix("http://font.localhost/cached/"))?;
    let mut parts = rest.split('/');
    let id = parts.next()?;
    let file = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((id, file))
}

/// Filename must be the M4 content-addressed shape: 8 lowercase hex chars +
/// "." + ttf|otf. Everything else is rejected before touching the filesystem.
pub fn valid_cached_filename(file: &str) -> bool {
    let bytes = file.as_bytes();
    if bytes.len() != 12 || bytes[8] != b'.' {
        return false;
    }
    let (stem, ext) = (&file[..8], &file[9..]);
    stem.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && (ext == "ttf" || ext == "otf")
}

/// Family id must be the slug charset ([a-z0-9-]+) — no dots, no traversal.
pub fn valid_family_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Serve one cached font file. Strict §31 validation: charset-checked id,
/// shape-checked filename, canonicalized path confined to the family
/// directory. Err carries the HTTP status for the scheme handler.
pub fn serve_font_file(data_dir: &Path, uri: &str) -> Result<(Vec<u8>, &'static str), u16> {
    let (id, file) = parse_cache_uri(uri).ok_or(404u16)?;
    if !valid_family_id(id) {
        return Err(403);
    }
    if !valid_cached_filename(file) {
        return Err(404);
    }
    let face_dir = data_dir.join(FONTS_DIR).join(id);
    let canonical_root = face_dir.canonicalize().map_err(|_| 404u16)?;
    let canonical_path = face_dir.join(file).canonicalize().map_err(|_| 404u16)?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(403);
    }
    let bytes = fs::read(&canonical_path).map_err(|_| 404u16)?;
    let ct = if canonical_path.extension().and_then(|e| e.to_str()) == Some("otf") {
        "font/otf"
    } else {
        "font/ttf"
    };
    Ok((bytes, ct))
}

/* ---- §24: cache deletion — the separate explicit operation ----

   Uninstalling never deletes the cache (§24); this is the companion op that
   does: remove fonts/<id>/ and clear `cached` only. Installed/ownership
   state is preserved — installed-but-missing-locally is a valid §17 state. */

/// Bytes used by a directory tree (0 when absent).
pub fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            total += dir_size(&p);
        } else if let Ok(m) = e.metadata() {
            total += m.len();
        }
    }
    total
}

/// Remove fonts/<id>/ entirely and report the bytes freed. The id must pass
/// the slug charset; a missing directory is an error (there is nothing to
/// delete — the explicit operation requires an explicit target).
pub fn delete_cached_dir(data_dir: &Path, id: &str) -> Result<u64, String> {
    if !valid_family_id(id) {
        return Err(format!("invalid family id: {id}"));
    }
    let dir = data_dir.join(FONTS_DIR).join(id);
    if !dir.is_dir() {
        return Err(format!("no cached files for {id}"));
    }
    let size = dir_size(&dir);
    fs::remove_dir_all(&dir).map_err(|e| format!("cannot delete {}: {e}", dir.display()))?;
    Ok(size)
}

/// §24 state transition: cache deletion clears `cached` and nothing else —
/// installed/managed/scope fields are untouched (their files live elsewhere).
pub fn clear_cached_state(states: &mut States, id: &str) -> Result<(), String> {
    match states.get_mut(id) {
        Some(st) if st.cached => {
            st.cached = false;
            Ok(())
        }
        Some(_) => Err(format!("{id} is not cached")),
        None => Err(format!("{id} is not in the library")),
    }
}

/* ---- library-state update (pure, unit-tested) ---- */

/// M4's only state transition: a validated download makes a face cached.
/// Installed/ownership fields are untouched (M6's concern).
pub fn update_library_state(states: &mut States, id: &str) {
    let entry = states.entry(id.to_string()).or_default();
    entry.cached = true;
}

/* ---- tests ---- */

#[cfg(test)]
mod tests {
    use super::*;

    const CSS: &str = r#"
@font-face {
  font-family: 'Inter';
  font-style: normal;
  font-weight: 400;
  src: url(https://fonts.gstatic.com/s/inter/x.ttf) format('truetype');
}
@font-face {
  font-family: 'Inter';
  font-style: italic;
  font-weight: 400;
  src: url(https://fonts.gstatic.com/s/inter/y.ttf) format('truetype');
}
@font-face {
  font-family: 'Inter';
  font-style: normal;
  font-weight: 100 900;
  src: url(https://fonts.gstatic.com/s/inter/v.ttf) format('truetype');
}
@font-face { font-family: 'X'; src: url(https://fonts.gstatic.com/s/x/z.ttf); }
"#;

    #[test]
    fn magic_kind_distinguishes_containers() {
        assert_eq!(magic_kind(&[0x00, 0x01, 0x00, 0x00]), Some(SfntKind::Ttf));
        assert_eq!(magic_kind(b"OTTO"), Some(SfntKind::Otf));
        assert_eq!(magic_kind(b"wOF2"), None);
        assert_eq!(magic_kind(&[0x00]), None);
        assert_eq!(magic_kind(&[]), None);
    }

    #[test]
    fn parse_css_faces_pairs_weight_and_url() {
        let faces = parse_css_faces(CSS);
        assert_eq!(faces.len(), 2, "variable-range and weightless blocks are skipped");
        assert_eq!(faces[0], CssFace { weight: 400, style: "normal", url: "https://fonts.gstatic.com/s/inter/x.ttf".into() });
        assert_eq!(faces[1].style, "italic");
        assert_eq!(faces[1].weight, 400);
    }

    #[test]
    fn parse_css_faces_minimal_block() {
        let faces = parse_css_faces("@font-face{src:url(https://x/z.ttf)}");
        assert_eq!(faces.len(), 0);
    }

    #[test]
    fn css2_url_dedupes_and_handles_italic() {
        let mut rec = FontRecord {
            id: "inter".into(),
            family: "Inter".into(),
            category: "Sans".into(),
            designer: String::new(),
            year: 0,
            styles: 0,
            weights: vec![400, 100, 400],
            italic: false,
            note: String::new(),
            pairs_with: String::new(),
            popularity: 0,
            removed_from_source: false,
        };
        assert_eq!(css2_url(&rec), "https://fonts.googleapis.com/css2?family=Inter:wght@100;400&display=swap");
        rec.italic = true;
        assert_eq!(css2_url(&rec), "https://fonts.googleapis.com/css2?family=Inter:ital,wght@0,100;0,400;1,100;1,400&display=swap");
        rec.family = "Bodoni Moda".into();
        assert!(css2_url(&rec).contains("family=Bodoni%20Moda"));
    }

    #[test]
    fn staged_paths_are_unique_per_weight_and_style() {
        let dir = Path::new("/data");
        let paths = staged_paths(dir, "inter", &[(400, "normal"), (400, "italic"), (700, "normal")]);
        let names: Vec<_> = paths.iter().map(|(p, _, _)| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names.len(), 3);
        assert_eq!(names.len(), names.iter().collect::<std::collections::HashSet<_>>().len());
        assert!(names[0].starts_with("inter.400.normal.") && names[0].ends_with(".part"));
    }

    #[test]
    fn update_library_state_only_sets_cached() {
        let mut states = States::new();
        update_library_state(&mut states, "inter");
        let st = &states["inter"];
        assert!(st.cached && !st.installed && st.managed_by_typecase.is_none());
        // Re-download does not fabricate installation state.
        update_library_state(&mut states, "inter");
        assert!(!states["inter"].installed);
    }

    #[test]
    fn meta_save_roundtrip() {
        let dir = std::env::temp_dir().join(format!("typecase-meta-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let meta = FontMeta {
            id: "inter".into(),
            family: "Inter".into(),
            source: "google-fonts",
            downloaded_at: now_rfc3339(),
            files: vec![CacheFileMeta {
                file: "abcd1234.ttf".into(),
                weight: 400,
                style: "normal".into(),
                size: 123,
                sha256: "abcd".repeat(16),
            }],
            file_count: 1,
            total_size: 123,
        };
        meta.save(&dir).unwrap();
        let text = fs::read_to_string(dir.join("metadata.json")).unwrap();
        assert!(text.contains("\"downloadedAt\""));
        assert!(text.contains("abcd1234.ttf"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha256_is_deterministic() {
        let mut h1 = Sha256::new();
        let mut h2 = Sha256::new();
        h1.update(b"inter-400");
        h2.update(b"inter-400");
        assert_eq!(hex::encode(h1.finalize()), hex::encode(h2.finalize()));
    }

    /* ---- M5: scheme URL + serving validation ---- */

    #[test]
    fn cache_url_shape() {
        assert_eq!(cache_url("inter", "abcd1234.ttf"), "font://cached/inter/abcd1234.ttf");
    }

    #[test]
    fn parse_cache_uri_accepts_both_forms() {
        assert_eq!(
            parse_cache_uri("font://cached/inter/abcd1234.ttf"),
            Some(("inter", "abcd1234.ttf"))
        );
        // WebKitGTK rewrites custom schemes to http://<scheme>.localhost.
        assert_eq!(
            parse_cache_uri("http://font.localhost/cached/inter/abcd1234.ttf"),
            Some(("inter", "abcd1234.ttf"))
        );
        assert_eq!(parse_cache_uri("font://cached/inter"), None);
        assert_eq!(parse_cache_uri("font://cached/a/b/c.ttf"), None);
        assert_eq!(parse_cache_uri("https://example.com/cached/a/b.ttf"), None);
    }

    #[test]
    fn filename_charset_is_enforced() {
        assert!(valid_cached_filename("abcd1234.ttf"));
        assert!(valid_cached_filename("4f70a04a.otf"));
        assert!(!valid_cached_filename("ABCD1234.ttf"), "uppercase hex rejected");
        assert!(!valid_cached_filename("abcd1234.woff"));
        assert!(!valid_cached_filename("../../etc/passwd"));
        assert!(!valid_cached_filename("short.ttf"));
        assert!(!valid_cached_filename("abcd1234..ttf"));
    }

    #[test]
    fn family_id_charset_is_enforced() {
        assert!(valid_family_id("inter"));
        assert!(valid_family_id("bodoni-moda"));
        assert!(!valid_family_id(""));
        assert!(!valid_family_id("../etc"));
        assert!(!valid_family_id("Inter"));
        assert!(!valid_family_id("a.b"));
    }

    #[test]
    fn serve_font_file_streams_and_blocks_traversal() {
        let dir = std::env::temp_dir().join(format!("typecase-serve-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("fonts").join("inter")).unwrap();
        let body = [0u8, 1, 0, 0, 0x7f];
        fs::write(dir.join("fonts").join("inter").join("abcd1234.ttf"), &body).unwrap();
        // A decoy outside the cache tree that traversal attempts must not reach.
        fs::create_dir_all(dir.join("fonts").join("..well")).unwrap();
        fs::write(dir.join("fonts").join("..well").join("abcd1234.ttf"), b"secret").unwrap();

        let (bytes, ct) =
            serve_font_file(&dir, "font://cached/inter/abcd1234.ttf").expect("valid request serves");
        assert_eq!(bytes, body);
        assert_eq!(ct, "font/ttf");

        assert_eq!(serve_font_file(&dir, "font://cached/inter/missing1.ttf"), Err(404));
        assert_eq!(serve_font_file(&dir, "font://cached/nofam/abcd1234.ttf"), Err(404));
        assert_eq!(serve_font_file(&dir, "font://cached/Int../abcd1234.ttf"), Err(403));
        assert_eq!(serve_font_file(&dir, "font://cached/inter/not-a-shape"), Err(404));
        assert_eq!(
            serve_font_file(&dir, "font://cached/inter%2f%2e%2e/abcd1234.ttf"),
            Err(403),
            "percent-encoded dots fail the id charset"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn font_sources_lists_manifest_families_and_skips_broken() {
        let dir = std::env::temp_dir().join(format!("typecase-sources-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let inter = dir.join("fonts").join("inter");
        fs::create_dir_all(&inter).unwrap();
        fs::write(
            inter.join("metadata.json"),
            r#"{"id":"inter","family":"Inter","source":"google-fonts","downloadedAt":"x","files":[{"file":"abcd1234.ttf","weight":400,"style":"normal","size":1,"sha256":"ab"}],"fileCount":1,"totalSize":1}"#,
        )
        .unwrap();
        // Broken manifest family: skipped, never fabricated.
        let broken = dir.join("fonts").join("broken-fam");
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join("metadata.json"), "{ not json").unwrap();

        let sources = font_sources(&dir);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id, "inter");
        assert_eq!(sources[0].family, "Inter");
        assert_eq!(sources[0].files.len(), 1);
        assert_eq!(sources[0].files[0].url, "font://cached/inter/abcd1234.ttf");
        assert_eq!(sources[0].files[0].weight, 400);
        assert_eq!(sources[0].files[0].style, "normal");

        // No fonts dir at all → empty, not an error.
        let empty = dir.join("elsewhere");
        fs::create_dir_all(&empty).unwrap();
        assert!(font_sources(&empty).is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    /* ---- §24 cache deletion ---- */

    #[test]
    fn clear_cached_state_only_clears_cached() {
        use crate::library::model::LibraryState;
        let mut states = States::new();
        // Not in the library at all → error.
        assert!(clear_cached_state(&mut states, "inter").is_err());
        // Cached-only → cleared.
        states.insert(
            "inter".into(),
            LibraryState {
                cached: true,
                ..Default::default()
            },
        );
        clear_cached_state(&mut states, "inter").unwrap();
        assert!(!states["inter"].cached);
        // Already-uncached → error (the explicit op requires a cached target).
        assert!(clear_cached_state(&mut states, "inter").is_err());
        // Installed entry: cached cleared, installed/ownership PRESERVED (§24).
        states.insert(
            "other".into(),
            LibraryState {
                cached: true,
                installed: true,
                managed_by_typecase: Some(true),
                install_scope: Some(crate::library::model::Scope::User),
            },
        );
        clear_cached_state(&mut states, "other").unwrap();
        let st = &states["other"];
        assert!(!st.cached && st.installed);
        assert_eq!(st.managed_by_typecase, Some(true));
        assert_eq!(st.install_scope, Some(crate::library::model::Scope::User));
    }

    #[test]
    fn delete_cached_dir_removes_tree_and_reports_size() {
        let dir = std::env::temp_dir().join(format!("typecase-del-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let inter = dir.join("fonts").join("inter");
        fs::create_dir_all(&inter).unwrap();
        fs::write(inter.join("abcd1234.ttf"), [0u8; 1_000]).unwrap();
        fs::write(inter.join("metadata.json"), "{}").unwrap();

        let freed = delete_cached_dir(&dir, "inter").unwrap();
        assert!(freed >= 1_002, "freed bytes: {freed}");
        assert!(!inter.exists());

        // The explicit target is gone → further deletes are errors.
        assert!(delete_cached_dir(&dir, "inter").is_err());
        // Invalid ids are rejected before any filesystem work.
        assert!(delete_cached_dir(&dir, "../x").is_err());
        assert!(delete_cached_dir(&dir, "").is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
