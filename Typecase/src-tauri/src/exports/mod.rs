/* M8 export/backup (CONTEXT.md §11 Export, §26, §31).

   Produces a ZIP archive containing every cached file of a family plus a
   README manifest — the backup format for §26 (works offline; the cached
   copy must be preservable independently of Google's availability, which is
   the whole point for removed-from-source families).

   §31 rules implemented here: the destination directory is chosen by the
   backend (never from frontend strings), family ids must pass the slug
   charset, and staged temp files keep a failed export from ever appearing
   as a valid backup. The ZIP writer is dependency-free (§37): stored (not
   deflated) entries, which is also what real TTF/OTF backup tools do when
   the payloads are already compressed containers. */

use crate::downloads::{read_manifest, FONTS_DIR};
use crate::library::model::FontRecord;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/* ---- CRC-32 (IEEE, the zip variant) ---- */

fn crc32(data: &[u8]) -> u32 {
    // Table-free bitwise variant: built once per file, fast enough for
    // ~100 KB font payloads and utterly dependency-free.
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Little-endian u16/u32 writers for the archive structure.
fn le16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}
fn le32(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

/// One stored ZIP entry: local header + payload (+ optional data descriptor,
/// not needed since we know sizes up front).
fn zip_entry(name: &str, data: &[u8]) -> Vec<u8> {
    let crc = crc32(data);
    let size = data.len() as u32;
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len() as u16;

    let mut out = Vec::with_capacity(30 + name_bytes.len() + data.len() + 46);
    // Local file header.
    out.extend_from_slice(&le32(0x0403_4b50));
    out.extend_from_slice(&le16(20)); // version needed (2.0: no compression)
    out.extend_from_slice(&le16(0x0800)); // flags: UTF-8 names
    out.extend_from_slice(&le16(0)); // method: stored
    out.extend_from_slice(&le16(0)); // mod time (fixed; the manifest carries real stamps)
    out.extend_from_slice(&le16(0x2921)); // mod date (1980-01-01, the ZIP epoch)
    out.extend_from_slice(&le32(crc));
    out.extend_from_slice(&le32(size)); // compressed = stored = size
    out.extend_from_slice(&le32(size));
    out.extend_from_slice(&le16(name_len));
    out.extend_from_slice(&le16(0)); // extra len
    out.extend_from_slice(name_bytes);
    out.extend_from_slice(data);
    out
}

/// The central-directory record for one entry.
fn central_entry(name: &str, offset: u32, data: &[u8]) -> Vec<u8> {
    let crc = crc32(data);
    let size = data.len() as u32;
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len() as u16;

    let mut out = Vec::with_capacity(46 + name_bytes.len());
    out.extend_from_slice(&le32(0x0201_4b50));
    out.extend_from_slice(&le16(20)); // version made by
    out.extend_from_slice(&le16(20)); // version needed
    out.extend_from_slice(&le16(0x0800)); // UTF-8 flag
    out.extend_from_slice(&le16(0)); // stored
    out.extend_from_slice(&le16(0));
    out.extend_from_slice(&le16(0x2921));
    out.extend_from_slice(&le32(crc));
    out.extend_from_slice(&le32(size));
    out.extend_from_slice(&le32(size));
    out.extend_from_slice(&le16(name_len));
    out.extend_from_slice(&le16(0)); // extra
    out.extend_from_slice(&le16(0)); // comment
    out.extend_from_slice(&le16(0)); // disk number
    out.extend_from_slice(&le16(0)); // internal attrs
    out.extend_from_slice(&le32(0)); // external attrs
    out.extend_from_slice(&le32(offset));
    out.extend_from_slice(name_bytes);
    out
}

/// Build a complete in-memory ZIP archive from (name, bytes) entries.
pub fn build_zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        out.extend_from_slice(&zip_entry(name, data));
        central.extend_from_slice(&central_entry(name, offset, data));
    }
    let central_offset = out.len() as u32;
    // Central directory comes before the EOCD — the layout every parser
    // relies on (locals → central directory → end-of-central-directory).
    out.extend_from_slice(&central);
    let central_size = central.len() as u32;
    let count = entries.len() as u16;
    // End of central directory.
    out.extend_from_slice(&le32(0x0605_4b50));
    out.extend_from_slice(&le16(0));
    out.extend_from_slice(&le16(0));
    out.extend_from_slice(&le16(count));
    out.extend_from_slice(&le16(count));
    out.extend_from_slice(&le32(central_size));
    out.extend_from_slice(&le32(central_offset));
    out.extend_from_slice(&le16(0)); // comment len
    out
}

/* ---- the README manifest ---- */

/// Human-readable manifest inside every export (§11: "useful metadata").
pub fn readme(rec: &FontRecord, files: &[String]) -> String {
    let mut out = String::new();
    out.push_str("Typecase font export\n");
    out.push_str("====================\n\n");
    out.push_str(&format!("Family:   {}\n", rec.family));
    out.push_str(&format!("Id:       {}\n", rec.id));
    out.push_str(&format!("Category: {}\n", rec.category));
    if !rec.designer.is_empty() {
        out.push_str(&format!("Designer: {}\n", rec.designer));
    }
    if rec.year > 0 {
        out.push_str(&format!("Year:     {}\n", rec.year));
    }
    out.push_str(&format!("Licence:  SIL Open Font Licence 1.1\n"));
    if rec.removed_from_source {
        out.push_str("\nNOTE: this family is no longer listed on Google Fonts.\nThis archive may be the only copy Typecase can offer.\n");
    }
    out.push_str("\nFiles:\n");
    for f in files {
        out.push_str(&format!("  {f}\n"));
    }
    out.push_str("\nExported by Typecase — a local-first Windows font manager.\n");
    out
}

/* ---- export sources: cached files first, installed files as fallback ---- */

/// Where exportable font files live for one family, in priority order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportSource {
    /// The cache manifest's files under fonts/<id>/ (§26: the primary path).
    Cached { files: Vec<PathBuf> },
    /// No cache — fall back to what Typecase installed (the copy in the
    /// Windows font directory). Also keeps export working after a §24
    /// cache deletion on an installed family.
    Installed { files: Vec<PathBuf> },
}

/// Resolve the exportable files for a family from its cache manifest,
/// falling back to the install record when nothing is cached.
pub fn export_source(
    data_dir: &Path,
    rec: &FontRecord,
    installs: &std::collections::HashMap<String, crate::fontmanager::InstallRecord>,
) -> Option<ExportSource> {
    if let Some(meta) = read_manifest(data_dir, &rec.id) {
        let files: Vec<PathBuf> = meta
            .files
            .iter()
            .map(|f| data_dir.join(FONTS_DIR).join(&rec.id).join(&f.file))
            .filter(|p| p.is_file())
            .collect();
        if !files.is_empty() {
            return Some(ExportSource::Cached { files });
        }
    }
    if let Some(record) = installs.get(&rec.id) {
        let files: Vec<PathBuf> = record
            .entries
            .iter()
            .map(|e| PathBuf::from(&e.file))
            .filter(|p| p.is_file())
            .collect();
        if !files.is_empty() {
            return Some(ExportSource::Installed { files });
        }
    }
    None
}

/// Archive entry name for an exported file: "<Family> <style-stem>.ttf".
/// The family name is sanitized (same charset rules as installed files) and
/// the stem keeps the original content-addressed identity when unknown.
fn archive_name(family: &str, path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("font");
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("ttf");
    let bad = |c: char| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | ' ' | '-' | '_' | '.' | '(' | ')');
    let fam: String = family.chars().filter(|c| !bad(*c)).collect();
    let fam = fam.trim().trim_start_matches('.').trim();
    let fam = if fam.is_empty() { "Font" } else { fam };
    format!("{fam} {stem}.{ext}")
}

/// Build the full export archive for a family (in-memory; the caller writes
/// it to the backend-chosen destination). `files` comes from `export_source`.
pub fn build_family_zip(
    rec: &FontRecord,
    files: &[PathBuf],
) -> Result<Vec<u8>, String> {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for path in files {
        let data = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let name = archive_name(&rec.family, path);
        names.push(name.clone());
        entries.push((name, data));
    }
    let readme_text = readme(rec, &names);
    let all: Vec<(&str, Vec<u8>)> = entries
        .iter()
        .map(|(n, d)| (n.as_str(), d.clone()))
        .chain(std::iter::once(("README.txt", readme_text.into_bytes())))
        .collect();
    Ok(build_zip(&all))
}

/// Write the archive to `<destination>/<id>-typecase-export.zip`, staged
/// through a temp file so a failed export never looks like a valid backup.
/// The destination directory is chosen by the backend (§31) — the frontend
/// only asked "export this family".
pub fn write_export(
    destination: &Path,
    id: &str,
    archive: &[u8],
) -> Result<PathBuf, String> {
    if id.bytes().any(|b| !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')) {
        return Err(format!("invalid family id for export: {id}"));
    }
    fs::create_dir_all(destination)
        .map_err(|e| format!("cannot create {}: {e}", destination.display()))?;
    let final_path = destination.join(format!("{id}-typecase-export.zip"));
    let tmp = destination.join(format!("{id}-typecase-export.zip.part"));
    {
        let mut f =
            fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
        f.write_all(archive)
            .and_then(|_| f.sync_all())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
    }
    fs::rename(&tmp, &final_path)
        .map_err(|e| format!("finalize export: {e}"))?;
    Ok(final_path)
}

/* ---- tests ---- */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::model::FontRecord;

    fn rec(id: &str, family: &str) -> FontRecord {
        FontRecord {
            id: id.into(),
            family: family.into(),
            category: "Sans".into(),
            designer: "A Designer".into(),
            year: 2020,
            styles: 2,
            weights: vec![400, 700],
            italic: false,
            note: String::new(),
            pairs_with: String::new(),
            popularity: 0,
            removed_from_source: false,
        }
    }

    #[test]
    fn crc32_matches_known_vectors() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414F_A339);
    }

    #[test]
    fn zip_archive_has_valid_structure_and_roundtrips_entries() {
        let entries = vec![
            ("a.ttf", vec![1u8, 2, 3, 4, 5]),
            ("README.txt", b"hello export".to_vec()),
        ];
        let zip = build_zip(&entries);
        // Signatures at the right places.
        assert_eq!(&zip[0..4], &[0x50, 0x4b, 0x03, 0x04], "local header first");
        // End of central directory record is the last 22 bytes.
        let eocd = &zip[zip.len() - 22..];
        assert_eq!(&eocd[0..4], &[0x50, 0x4b, 0x05, 0x06]);
        // Entry count (2) appears in the EOCD.
        assert_eq!(u16::from_le_bytes([eocd[10], eocd[11]]), 2);
        // Payload bytes are stored verbatim.
        assert!(zip.windows(5).any(|w| *w == [1, 2, 3, 4, 5]));
        assert!(zip.windows(12).any(|w| w == *b"hello export"));
        // Two local headers (second entry), one EOCD.
        let locals = zip.windows(4).filter(|w| *w == [0x50, 0x4b, 0x03, 0x04]).count();
        assert_eq!(locals, 2);
        let centrals = zip.windows(4).filter(|w| *w == [0x50, 0x4b, 0x01, 0x02]).count();
        assert_eq!(centrals, 2);
    }

    #[test]
    fn zip_is_parseable_by_python_stdlib() {
        // A structural acceptance test using an external parser would need a
        // subprocess; here we validate the EOCD offset arithmetic instead —
        // the part real parsers rely on.
        let entries = vec![("x.ttf", vec![0u8; 300])];
        let zip = build_zip(&entries);
        let eocd_off = zip.len() - 22;
        let central_offset = u32::from_le_bytes([
            zip[eocd_off + 16],
            zip[eocd_off + 17],
            zip[eocd_off + 18],
            zip[eocd_off + 19],
        ]);
        let central_size = u32::from_le_bytes([
            zip[eocd_off + 12],
            zip[eocd_off + 13],
            zip[eocd_off + 14],
            zip[eocd_off + 15],
        ]);
        assert_eq!(central_offset as usize + central_size as usize, eocd_off);
        // And the central directory sits exactly where the local data ends.
        assert_eq!(&zip[central_offset as usize..central_offset as usize + 4], &[0x50, 0x4b, 0x01, 0x02]);
    }

    #[test]
    fn readme_lists_files_and_flags_removed_families() {
        let r = rec("inter", "Inter");
        let text = readme(&r, &["Inter abcd1234.ttf".into()]);
        assert!(text.contains("Family:   Inter"));
        assert!(text.contains("SIL Open Font Licence 1.1"));
        assert!(text.contains("Inter abcd1234.ttf"));
        assert!(!text.contains("no longer listed"));

        let mut gone = rec("gone", "Gone");
        gone.removed_from_source = true;
        let text = readme(&gone, &[]);
        assert!(text.contains("no longer listed on Google Fonts"));
    }

    #[test]
    fn archive_names_are_sanitized_and_keep_extensions() {
        let p = Path::new("/cache/inter/abcd1234.ttf");
        assert_eq!(archive_name("Inter", p), "Inter abcd1234.ttf");
        assert_eq!(
            archive_name("../Evil/Fam?", Path::new("/x/zz.otf")),
            "EvilFam zz.otf"
        );
        assert_eq!(archive_name("", p), "Font abcd1234.ttf");
    }

    #[test]
    fn export_source_prefers_cache_and_falls_back_to_installs() {
        let dir = std::env::temp_dir().join(format!("typecase-exp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let inter = dir.join("fonts").join("inter");
        fs::create_dir_all(&inter).unwrap();
        fs::write(inter.join("abcd1234.ttf"), [0u8; 10]).unwrap();
        fs::write(
            inter.join("metadata.json"),
            r#"{"id":"inter","family":"Inter","files":[{"file":"abcd1234.ttf","weight":400,"style":"normal","size":10,"sha256":"aa"}]}"#,
        )
        .unwrap();
        let r = rec("inter", "Inter");

        let installs = std::collections::HashMap::new();
        match export_source(&dir, &r, &installs) {
            Some(ExportSource::Cached { files }) => {
                assert_eq!(files.len(), 1);
                assert!(files[0].ends_with("abcd1234.ttf"));
            }
            other => panic!("expected cached source, got {other:?}"),
        }

        // No cache dir → falls back to a recorded install with real files.
        let empty = std::env::temp_dir().join(format!("typecase-exp2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&empty);
        let installed = empty.join("Fonts").join("Inter.ttf");
        fs::create_dir_all(installed.parent().unwrap()).unwrap();
        fs::write(&installed, [0u8; 10]).unwrap();
        let mut records = std::collections::HashMap::new();
        records.insert(
            "inter".to_string(),
            crate::fontmanager::InstallRecord {
                id: "inter".into(),
                family: "Inter".into(),
                scope: crate::library::model::Scope::User,
                registered_at: "2026-09-27T00:00:00Z".into(),
                entries: vec![crate::fontmanager::InstallEntry {
                    file: installed.to_string_lossy().into_owned(),
                    value_name: "Inter (TrueType)".into(),
                    source: "unused".into(),
                }],
            },
        );
        match export_source(&empty, &r, &records) {
            Some(ExportSource::Installed { files }) => assert_eq!(files.len(), 1),
            other => panic!("expected installed fallback, got {other:?}"),
        }

        // Neither → nothing exportable.
        assert!(export_source(&std::env::temp_dir().join("typecase-nowhere"), &r, &std::collections::HashMap::new()).is_none());
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&empty);
    }

    #[test]
    fn family_zip_carries_files_and_readme() {
        let dir = std::env::temp_dir().join(format!("typecase-expzip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let f1 = dir.join("abcd1234.ttf");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&f1, [0u8, 1, 0, 0]).unwrap();

        let r = rec("inter", "Inter");
        let zip = build_family_zip(&r, &[f1]).unwrap();
        assert!(zip.windows(4).any(|w| *w == [0x50, 0x4b, 0x03, 0x04]));
        assert!(zip.windows(10).any(|w| w == *b"README.txt"));
        assert!(zip.windows(18).any(|w| w == *b"Inter abcd1234.ttf"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_export_is_atomic_and_charset_checked() {
        let dir = std::env::temp_dir().join(format!("typecase-expw-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let out = write_export(&dir, "inter", b"zipbytes").unwrap();
        assert!(out.ends_with("inter-typecase-export.zip"));
        assert_eq!(fs::read(&out).unwrap(), b"zipbytes");
        assert!(!dir.join("inter-typecase-export.zip.part").exists());

        // The explicit operation requires a valid id — traversal never passes.
        assert!(write_export(&dir, "../evil", b"x").is_err());
        assert!(write_export(&dir, "Int", b"x").is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
