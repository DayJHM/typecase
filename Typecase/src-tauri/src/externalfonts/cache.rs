/* §25 step 3 / §43 flow 3: caching an EXTERNAL font.

   An external font is registered in Windows but not owned by Typecase — the
   file usually lives in the Windows font directory, and removing the font
   deletes it. §25 therefore requires the removal flow to *offer a copy first*,
   and §43 flow 3 ends with "user may cache it". This module makes that copy
   real: the file is validated with the M4 rules (§31: size bounds + sfnt magic
   + sha256), copied into Typecase's own storage, and recorded with its
   provenance so the user can see the copy exists and discard it deliberately.

   Storage decision: the copy lives in the ordinary cache tree,
   `fonts/<id>/<sha8>.<ttf|otf>` plus `metadata.json`, with
   `id = ext-<family-slug>` and `source = "external-windows"`. Reusing the M4
   layout is deliberate (§28: prefer a small, understandable codebase):

   - the §31 path rules, the content-addressed filename shape and the M5
     `font://` serving path all keep working unchanged, so a preserved copy is
     genuinely usable rather than a dead file the user can only find by hand;
   - the `ext-` prefix keeps the namespace distinguishable from provider
     families, and the manifest's `source` tag is what makes an entry
     *external* for `delete_cached_family` (an external copy has no library
     state to check — §24's cached flag is a provider-cache concept).

   Nothing here is Windows-specific: the platform's only role is to tell us the
   font is registered (`externalfonts::discover`), which the command verifies
   before calling in. */

use crate::downloads::{
    sfnt_ext, sha256_hex, valid_cached_filename, valid_family_id, validate_sfnt_bytes, FONTS_DIR,
    STAGING_DIR,
};
use crate::externalfonts::ExternalFont;
use crate::fontmanager::parse_style_name;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Every external copy's id starts with this, keeping provider families and
/// Typecase-held external copies distinguishable in one cache tree.
pub const ID_PREFIX: &str = "ext-";

/// Manifest `source` for an external copy. This tag — not the id prefix — is
/// the authority (a provider manifest is never treated as external).
pub const SOURCE: &str = "external-windows";

/* ---- identity ---- */

/// Cache id for an external family: `ext-` + the §31 slug charset
/// ([a-z0-9-]+). Empty or fully non-ASCII names still yield a valid id.
pub fn cache_id(family: &str) -> String {
    let mut slug = String::with_capacity(family.len() + ID_PREFIX.len());
    let mut pending_dash = false;
    for c in family.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(c.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
    }
    if slug.is_empty() {
        slug.push_str("font");
    }
    format!("{ID_PREFIX}{slug}")
}

/// True when `id` is a well-formed external-copy id (prefix + slug charset).
pub fn is_external_id(id: &str) -> bool {
    id.starts_with(ID_PREFIX) && valid_family_id(id)
}

/* ---- manifest ---- */

/// One preserved file. `weight`/`style` keep M4's semantics (a numeric weight
/// and the CSS-valid "normal"/"italic") because the M5 serving path and the M6
/// install planner both read them; `registryStyle` keeps Windows' human
/// spelling ("Bold", "900 Italic") and `valueName` is the provenance key that
/// ties the copy back to the registry entry it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedExternalFile {
    pub file: String,
    pub weight: u16,
    pub style: String,
    pub size: u64,
    pub sha256: String,
    #[serde(default)]
    pub registry_style: String,
    #[serde(default)]
    pub value_name: String,
}

/// The external-copy manifest. Serialized with the same core fields as M4's
/// metadata.json (`id`, `family`, `files[]`), so `downloads::read_manifest`
/// parses it and §31's serving validation applies unchanged; the extra keys
/// are provenance the provider pipeline never has.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedExternal {
    pub id: String,
    pub family: String,
    pub source: String,
    pub cached_at: String,
    pub files: Vec<CachedExternalFile>,
    pub file_count: usize,
    pub total_size: u64,
}

impl CachedExternal {
    fn manifest_path(data_dir: &Path, id: &str) -> PathBuf {
        data_dir.join(FONTS_DIR).join(id).join("metadata.json")
    }

    /// Read an external-copy manifest. Only external ids whose manifest is
    /// tagged `source: external-windows` are returned — a provider family's
    /// manifest is never misread as an external copy (and vice versa).
    pub fn read(data_dir: &Path, id: &str) -> Option<CachedExternal> {
        if !is_external_id(id) {
            return None;
        }
        let text = fs::read_to_string(Self::manifest_path(data_dir, id)).ok()?;
        let meta: CachedExternal = serde_json::from_str(&text).ok()?;
        if meta.source != SOURCE || meta.id != id {
            return None;
        }
        Some(meta)
    }

    /// Atomic save (temp file in the same directory, then rename) — the same
    /// rule as library.json and the M4 manifest: a failed write must never
    /// leave a half-written manifest looking valid.
    fn save(&self, data_dir: &Path) -> Result<(), String> {
        let path = Self::manifest_path(data_dir, &self.id);
        let tmp = path.with_extension("json.tmp");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let text =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialize metadata: {e}"))?;
        {
            let mut f = fs::File::create(&tmp)
                .map_err(|e| format!("create {}: {e}", tmp.display()))?;
            f.write_all(text.as_bytes())
                .and_then(|_| f.sync_all())
                .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        }
        fs::rename(&tmp, &path).map_err(|e| format!("rename into place: {e}"))
    }
}

/// Registry value name → cache id for every copy Typecase holds. Derived from
/// the manifests on demand (§32: one source of truth — there is no second
/// index that could go stale, and a copy the user deletes by hand simply
/// stops appearing).
pub fn inventory(data_dir: &Path) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Ok(entries) = fs::read_dir(data_dir.join(FONTS_DIR)) else {
        return out;
    };
    for entry in entries.flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        let Some(meta) = CachedExternal::read(data_dir, &id) else {
            continue;
        };
        for file in meta.files {
            if !file.value_name.is_empty() {
                out.insert(file.value_name, meta.id.clone());
            }
        }
    }
    out
}

/* ---- caching one registered font ---- */

/// What the UI gets back: identity and evidence, never a filesystem path the
/// frontend could then pass around (§30/§31).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheOutcome {
    pub id: String,
    pub family: String,
    pub file: String,
    pub size: u64,
    pub sha256: String,
    pub file_count: usize,
    /// The exact copy was already present — the operation is idempotent.
    pub already_cached: bool,
}

/// Copy one registered external font file into Typecase's cache.
///
/// Idempotent per file: the content-addressed name means re-caching unchanged
/// bytes rewrites nothing, and the manifest keeps one entry per registry value
/// name (re-caching a font the user replaced in Windows updates the entry
/// rather than accumulating orphans). Other styles of the same family are
/// preserved — families are cached file by file, which is exactly how the
/// Installed view lists them.
pub fn cache_registered(data_dir: &Path, entry: &ExternalFont) -> Result<CacheOutcome, String> {
    if entry.ownership != crate::externalfonts::Ownership::External {
        return Err(format!(
            "{} is managed by Typecase — use the managed cache instead",
            entry.family
        ));
    }
    let id = cache_id(&entry.family);
    if !is_external_id(&id) {
        return Err(format!("cannot derive a cache id for {}", entry.family));
    }

    // Read the source before touching the cache: everything is validated
    // first, so a rejected font leaves no files and no manifest behind.
    let source = Path::new(&entry.file_path);
    if entry.file_path.trim().is_empty() {
        return Err(format!("{} has no file path in the registry", entry.family));
    }
    let metadata = fs::metadata(source)
        .map_err(|e| format!("cannot read {}: {e}", source.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a file", source.display()));
    }
    let bytes = fs::read(source).map_err(|e| format!("cannot read {}: {e}", source.display()))?;
    let kind = validate_sfnt_bytes(&bytes)?;
    let digest = sha256_hex(&bytes);
    let file = format!("{}.{}", &digest[..8], sfnt_ext(kind));
    let (weight, style) = parse_style_name(&entry.style);

    let staging = data_dir.join(STAGING_DIR);
    let face_dir = data_dir.join(FONTS_DIR).join(&id);
    fs::create_dir_all(&staging).map_err(|e| format!("cannot create {}: {e}", staging.display()))?;
    fs::create_dir_all(&face_dir).map_err(|e| format!("cannot create {}: {e}", face_dir.display()))?;

    let dest = face_dir.join(&file);
    let already_cached = dest.is_file();
    if !already_cached {
        // Stage then rename (§31): an interrupted copy must never appear as a
        // valid cache entry.
        let tmp = staging.join(format!("{id}.{file}.part"));
        {
            let mut f =
                fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
            f.write_all(&bytes)
                .and_then(|_| f.sync_all())
                .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        }
        if let Err(e) = fs::rename(&tmp, &dest) {
            let _ = fs::remove_file(&tmp);
            return Err(format!("commit copy: {e}"));
        }
    }

    // Merge into the family manifest: replace the entry for this registry
    // value name (or an identical file name), keep the other styles.
    let existing = CachedExternal::read(data_dir, &id);
    let mut files = existing.map(|m| m.files).unwrap_or_default();
    let mut superseded: Vec<String> = Vec::new();
    files.retain(|f| {
        let drop = f.value_name == entry.value_name || f.file == file;
        if drop && f.file != file {
            superseded.push(f.file.clone());
        }
        !drop
    });
    files.push(CachedExternalFile {
        file: file.clone(),
        weight,
        style: style.to_string(),
        size: bytes.len() as u64,
        sha256: digest.clone(),
        registry_style: entry.style.clone(),
        value_name: entry.value_name.clone(),
    });
    let total_size = files.iter().map(|f| f.size).sum();
    let record = CachedExternal {
        id: id.clone(),
        family: entry.family.clone(),
        source: SOURCE.to_string(),
        cached_at: crate::downloads::now_rfc3339_pub(),
        file_count: files.len(),
        total_size,
        files,
    };
    let file_count = record.file_count;
    record.save(data_dir)?;

    // A replaced copy (the user updated the font in Windows) is no longer
    // referenced by the manifest: delete it so the family directory does not
    // keep unreachable bytes forever. The name is re-validated first (§31).
    for name in superseded {
        if !record.files.iter().any(|f| f.file == name) && valid_cached_filename(&name) {
            let _ = fs::remove_file(face_dir.join(&name));
        }
    }

    Ok(CacheOutcome {
        id,
        family: record.family,
        file,
        size: bytes.len() as u64,
        sha256: digest,
        file_count,
        already_cached,
    })
}

/* ---- tests (pure/storage logic, runs on every platform) ---- */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::externalfonts::Ownership;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("typecase-extcache-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A minimal but genuinely valid-looking sfnt payload (magic + size).
    fn sfnt(prefix: &[u8; 4], fill: u8) -> Vec<u8> {
        let mut v = prefix.to_vec();
        v.resize(1_500, fill);
        v
    }

    fn entry(family: &str, style: &str, value_name: &str, path: &str) -> ExternalFont {
        ExternalFont {
            value_name: value_name.into(),
            family: family.into(),
            style: style.into(),
            file_path: path.into(),
            scope: "user".into(),
            ownership: Ownership::External,
            id: None,
            cached_id: None,
        }
    }

    #[test]
    fn cache_ids_follow_the_slug_charset() {
        assert_eq!(cache_id("Inter"), "ext-inter");
        assert_eq!(cache_id("Bodoni Moda"), "ext-bodoni-moda");
        assert_eq!(cache_id("  ..Evil/Fam  "), "ext-evil-fam");
        assert_eq!(cache_id("900 Sans!"), "ext-900-sans");
        // Non-ASCII names still produce a usable id rather than an empty one.
        assert_eq!(cache_id("Müller"), "ext-m-ller");
        assert_eq!(cache_id(""), "ext-font");
        assert_eq!(cache_id("///"), "ext-font");

        for fam in ["Inter", "", "///", "Müller", "A:B<C>D"] {
            let id = cache_id(fam);
            assert!(is_external_id(&id), "id for {fam:?} was {id:?}");
        }
        // Provider ids are never mistaken for external ones.
        assert!(!is_external_id("inter"));
        assert!(!is_external_id("ext-../evil"));
        assert!(!is_external_id("ext-Upper"));
    }

    #[test]
    fn caching_a_file_writes_a_provider_compatible_manifest() {
        let dir = tmpdir("write");
        let src = dir.join("Inter.ttf");
        fs::write(&src, sfnt(&[0x00, 0x01, 0x00, 0x00], 7)).unwrap();

        let e = entry("Inter", "Regular", "Inter (TrueType)", src.to_str().unwrap());
        let out = cache_registered(&dir, &e).unwrap();
        assert_eq!(out.id, "ext-inter");
        assert_eq!(out.file_count, 1);
        assert!(!out.already_cached);
        assert_eq!(out.size, 1_500);
        assert_eq!(out.sha256.len(), 64);
        // Content-addressed file name, valid for the M5 serving validator.
        assert_eq!(out.file, format!("{}.ttf", &out.sha256[..8]));
        assert!(crate::downloads::valid_cached_filename(&out.file));

        // The M4 reader sees a normal manifest — id, family, weight, style.
        let stored = crate::downloads::read_manifest(&dir, "ext-inter").unwrap();
        assert_eq!(stored.id, "ext-inter");
        assert_eq!(stored.family, "Inter");
        assert_eq!(stored.files.len(), 1);
        assert_eq!(stored.files[0].weight, 400);
        assert_eq!(stored.files[0].style, "normal");
        assert_eq!(stored.files[0].file, out.file);

        // Provenance survives in the external view of the manifest.
        let meta = CachedExternal::read(&dir, "ext-inter").unwrap();
        assert_eq!(meta.source, SOURCE);
        assert_eq!(meta.files[0].registry_style, "Regular");
        assert_eq!(meta.files[0].value_name, "Inter (TrueType)");
        assert_eq!(meta.total_size, 1_500);
        // No staging leftovers.
        assert!(!dir.join(STAGING_DIR).join(format!("ext-inter.{}.part", out.file)).exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn re_caching_is_idempotent_and_merges_styles() {
        let dir = tmpdir("merge");
        let regular = dir.join("Inter.ttf");
        let bold = dir.join("Inter Bold.ttf");
        fs::write(&regular, sfnt(&[0x00, 0x01, 0x00, 0x00], 1)).unwrap();
        fs::write(&bold, sfnt(&[0x00, 0x01, 0x00, 0x00], 2)).unwrap();

        let r = entry("Inter", "Regular", "Inter (TrueType)", regular.to_str().unwrap());
        let b = entry("Inter", "Bold", "Inter Bold (TrueType)", bold.to_str().unwrap());

        let first = cache_registered(&dir, &r).unwrap();
        assert!(!first.already_cached);
        // Same bytes again: no rewrite, same single entry.
        let again = cache_registered(&dir, &r).unwrap();
        assert!(again.already_cached);
        assert_eq!(again.file, first.file);
        assert_eq!(again.file_count, 1);

        // A second style joins the same family entry.
        let second = cache_registered(&dir, &b).unwrap();
        assert_eq!(second.id, "ext-inter");
        assert_eq!(second.file_count, 2);
        let meta = CachedExternal::read(&dir, "ext-inter").unwrap();
        assert_eq!(meta.file_count, 2);
        assert_eq!(meta.total_size, 3_000);
        let bold_file = meta.files.iter().find(|f| f.value_name == "Inter Bold (TrueType)").unwrap();
        assert_eq!(bold_file.weight, 700);
        assert_eq!(bold_file.style, "normal");
        // M4 reader still agrees on both files.
        assert_eq!(crate::downloads::read_manifest(&dir, "ext-inter").unwrap().files.len(), 2);

        // Replacing the same registry entry (user updated the font in Windows)
        // updates it in place instead of accumulating a second copy of it.
        fs::write(&regular, sfnt(&[0x00, 0x01, 0x00, 0x00], 9)).unwrap();
        let changed = cache_registered(&dir, &r).unwrap();
        assert!(!changed.already_cached);
        assert_ne!(changed.file, first.file);
        let meta = CachedExternal::read(&dir, "ext-inter").unwrap();
        assert_eq!(meta.file_count, 2, "one entry per registry value name");
        // The superseded copy is removed with the entry it replaced.
        assert!(meta.files.iter().all(|f| f.file != first.file));
        assert!(
            !dir.join(FONTS_DIR).join("ext-inter").join(&first.file).exists(),
            "a replaced copy must not linger as unreachable bytes"
        );
        // The Bold entry (a different registry value name) is untouched.
        assert!(meta.files.iter().any(|f| f.file == second.file));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn italic_styles_keep_css_semantics_and_the_registry_spelling() {
        let dir = tmpdir("italic");
        let src = dir.join("Inter 900 Italic.otf");
        fs::write(&src, sfnt(b"OTTO", 3)).unwrap();

        let e = entry("Inter", "900 Italic", "Inter 900 Italic (OpenType)", src.to_str().unwrap());
        let out = cache_registered(&dir, &e).unwrap();
        assert!(out.file.ends_with(".otf"));
        let meta = CachedExternal::read(&dir, "ext-inter").unwrap();
        let f = &meta.files[0];
        assert_eq!(f.weight, 900);
        assert_eq!(f.style, "italic");
        assert_eq!(f.registry_style, "900 Italic");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_non_fonts_non_files_and_managed_fonts() {
        let dir = tmpdir("reject");
        // A real file that is not a font: refused, and nothing is written.
        let not_a_font = dir.join("notes.txt");
        fs::write(&not_a_font, vec![b'x'; 1_500]).unwrap();
        let e = entry("Notes", "Regular", "Notes (TrueType)", not_a_font.to_str().unwrap());
        let err = cache_registered(&dir, &e).unwrap_err();
        assert!(err.contains("sfnt"), "unexpected error: {err}");
        assert!(!dir.join(FONTS_DIR).exists(), "a rejected copy must write nothing");

        // Too small to be a font even with the right magic.
        let tiny = dir.join("tiny.ttf");
        fs::write(&tiny, [0x00, 0x01, 0x00, 0x00]).unwrap();
        let e = entry("Tiny", "Regular", "Tiny (TrueType)", tiny.to_str().unwrap());
        assert!(cache_registered(&dir, &e).unwrap_err().contains("too small"));
        assert!(!dir.join(FONTS_DIR).exists());

        // Missing file / directory / empty path are honest errors.
        let e = entry("Gone", "Regular", "Gone (TrueType)", dir.join("gone.ttf").to_str().unwrap());
        assert!(cache_registered(&dir, &e).unwrap_err().contains("cannot read"));
        let e = entry("Dir", "Regular", "Dir (TrueType)", dir.to_str().unwrap());
        assert!(cache_registered(&dir, &e).unwrap_err().contains("not a file"));
        let e = entry("Empty", "Regular", "Empty (TrueType)", "  ");
        assert!(cache_registered(&dir, &e).unwrap_err().contains("no file path"));
        assert!(!dir.join(FONTS_DIR).exists());

        // Managed fonts have their own cache path (§20) — never this one.
        let font = dir.join("Managed.ttf");
        fs::write(&font, sfnt(&[0x00, 0x01, 0x00, 0x00], 4)).unwrap();
        let mut e = entry("Managed", "Regular", "Managed (TrueType)", font.to_str().unwrap());
        e.ownership = Ownership::Managed;
        e.id = Some("managed".into());
        assert!(cache_registered(&dir, &e).unwrap_err().contains("managed"));
        assert!(!dir.join(FONTS_DIR).exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn inventory_lists_copies_by_registry_value_name() {
        let dir = tmpdir("inventory");
        assert!(inventory(&dir).is_empty(), "no cache tree yet");

        let one = dir.join("Inter.ttf");
        let two = dir.join("Other.ttf");
        fs::write(&one, sfnt(&[0x00, 0x01, 0x00, 0x00], 1)).unwrap();
        fs::write(&two, sfnt(&[0x00, 0x01, 0x00, 0x00], 2)).unwrap();
        cache_registered(&dir, &entry("Inter", "Regular", "Inter (TrueType)", one.to_str().unwrap()))
            .unwrap();
        cache_registered(&dir, &entry("Other Fam", "Regular", "Other Fam (TrueType)", two.to_str().unwrap()))
            .unwrap();

        let inv = inventory(&dir);
        assert_eq!(inv.get("Inter (TrueType)").map(String::as_str), Some("ext-inter"));
        assert_eq!(inv.get("Other Fam (TrueType)").map(String::as_str), Some("ext-other-fam"));
        assert_eq!(inv.len(), 2);

        // A registrar-provider manifest in the same tree is never listed.
        let provider = dir.join(FONTS_DIR).join("provider");
        fs::create_dir_all(&provider).unwrap();
        fs::write(
            provider.join("metadata.json"),
            r#"{"id":"provider","family":"Provider","source":"google-fonts","downloadedAt":"x","files":[{"file":"abcd1234.ttf","weight":400,"style":"normal","size":1,"sha256":"aa"}],"fileCount":1,"totalSize":1}"#,
        )
        .unwrap();
        let inv = inventory(&dir);
        assert_eq!(inv.len(), 2);
        assert!(CachedExternal::read(&dir, "provider").is_none(), "provider manifests are not external");
        assert!(CachedExternal::read(&dir, "inter").is_none(), "unprefixed ids are not external");
        assert!(CachedExternal::read(&dir, "ext-missing").is_none());
        assert!(CachedExternal::read(&dir, "../evil").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_manifest_is_absent_rather_than_fabricated() {
        let dir = tmpdir("corrupt");
        let face = dir.join(FONTS_DIR).join("ext-inter");
        fs::create_dir_all(&face).unwrap();
        fs::write(face.join("metadata.json"), "{ not json !!!").unwrap();
        assert!(CachedExternal::read(&dir, "ext-inter").is_none());
        assert!(inventory(&dir).is_empty());
        // And the corrupt entry does not block caching a good copy.
        let src = dir.join("Inter.ttf");
        fs::write(&src, sfnt(&[0x00, 0x01, 0x00, 0x00], 5)).unwrap();
        let out = cache_registered(
            &dir,
            &entry("Inter", "Regular", "Inter (TrueType)", src.to_str().unwrap()),
        )
        .unwrap();
        assert_eq!(out.file_count, 1);
        assert!(CachedExternal::read(&dir, "ext-inter").is_some());
        let _ = fs::remove_dir_all(&dir);
    }
}
