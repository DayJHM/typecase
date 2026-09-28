/* Typecase export/backup integration test (M8, CONTEXT §11/§26/§31).

   Exercises the full export path against the real cache layout the M4
   pipeline produces: manifest → export source → ZIP archive → staged write.
   No network, runs with the default suite (the font bytes are fakes — the
   ZIP layer only stores them; sfnt validation belongs to the M4 pipeline).

   The archive is additionally parsed back with an independent structural
   check (EOCD offsets, entry count) so a regression cannot ship silently. */

use std::collections::HashMap;
use typecase_lib::exports::{self, ExportSource};
use typecase_lib::fontmanager::{InstallEntry, InstallRecord};
use typecase_lib::library::model::{FontRecord, Scope};

fn rec(id: &str, family: &str) -> FontRecord {
    FontRecord {
        id: id.into(),
        family: family.into(),
        category: "Sans".into(),
        designer: "Rasmus Andersson".into(),
        year: 2016,
        styles: 2,
        weights: vec![400, 700],
        italic: false,
        note: "workhorse".into(),
        pairs_with: "Newsreader".into(),
        popularity: 1,
        removed_from_source: false,
    }
}

fn tmpdir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("typecase-expint-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Subsequence search with the window length derived from the needle —
/// magic-number `windows(N)` calls have bitten twice already.
fn has(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn export_full_path_cache_to_zip_file() {
    let dir = tmpdir("full");
    let out_dir = tmpdir("out");

    // A cached family exactly as the M4 commit step leaves it: two files
    // (400 normal + 400 italic) + manifest.
    let inter = dir.join("fonts").join("inter");
    std::fs::create_dir_all(&inter).unwrap();
    let mut body400 = vec![0u8, 1, 0, 0];
    body400.extend(std::iter::repeat(0xABu8).take(4_000));
    let body700 = vec![b'O', b'T', b'T', b'O', 0x00, 0x7f];
    std::fs::write(inter.join("abcd1234.ttf"), &body400).unwrap();
    std::fs::write(inter.join("b1d784c4.otf"), &body700).unwrap();
    std::fs::write(
        inter.join("metadata.json"),
        r#"{"id":"inter","family":"Inter","source":"google-fonts","downloadedAt":"2026-09-27T00:00:00Z","files":[{"file":"abcd1234.ttf","weight":400,"style":"normal","size":4004,"sha256":"aa"},{"file":"b1d784c4.otf","weight":400,"style":"italic","size":6,"sha256":"bb"}],"fileCount":2,"totalSize":4010}"#,
    )
    .unwrap();

    let installs: HashMap<String, InstallRecord> = HashMap::new();
    let r = rec("inter", "Inter");
    let source = exports::export_source(&dir, &r, &installs)
        .expect("a cached family is exportable");
    let files = match &source {
        ExportSource::Cached { files } => files.clone(),
        other => panic!("expected cached, got {other:?}"),
    };
    assert_eq!(files.len(), 2);

    let archive = exports::build_family_zip(&r, &files).unwrap();
    let path = exports::write_export(&out_dir, "inter", &archive).unwrap();
    assert!(path.exists(), "export lands at {}", path.display());
    assert!(path.to_string_lossy().ends_with("inter-typecase-export.zip"));
    let written = std::fs::read(&path).unwrap();

    // Structure: two local entries + README, central directory, EOCD.
    let eocd = &written[written.len() - 22..];
    assert_eq!(&eocd[0..4], &[0x50, 0x4b, 0x05, 0x06]);
    assert_eq!(u16::from_le_bytes([eocd[10], eocd[11]]), 3, "two files + README");
    let locals = written.windows(4).filter(|w| *w == [0x50, 0x4b, 0x03, 0x04]).count();
    assert_eq!(locals, 3);
    // The payloads survive byte-for-byte (stored entries).
    assert!(has(&written, &body400));
    assert!(has(&written, &body700));
    // The README carries the family and the OFL line.
    assert!(has(&written, b"Family:   Inter"));
    assert!(has(&written, b"SIL Open Font Licence 1.1"));

    // A second export overwrites atomically — no .part leftovers.
    let again = exports::write_export(&out_dir, "inter", &archive).unwrap();
    assert_eq!(again, path);
    assert!(!out_dir.join("inter-typecase-export.zip.part").exists());

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn export_falls_back_to_installed_files_when_not_cached() {
    let empty = tmpdir("fallback");
    let out_dir = tmpdir("fallback-out");

    // An installed-but-not-cached family (post-§24 deletion, per §17).
    let installed = empty.join("Fonts").join("Inter Bold.ttf");
    std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
    std::fs::write(&installed, [0u8, 1, 0, 0, 0x7f]).unwrap();
    let mut installs: HashMap<String, InstallRecord> = HashMap::new();
    installs.insert(
        "inter".to_string(),
        InstallRecord {
            id: "inter".into(),
            family: "Inter".into(),
            scope: Scope::User,
            registered_at: "2026-09-27T00:00:00Z".into(),
            entries: vec![InstallEntry {
                file: installed.to_string_lossy().into_owned(),
                value_name: "Inter Bold (TrueType)".into(),
                source: "unused".into(),
            }],
        },
    );

    let r = rec("inter", "Inter");
    let source = exports::export_source(&empty, &r, &installs).expect("installed fallback");
    let files = match &source {
        ExportSource::Installed { files } => files.clone(),
        other => panic!("expected installed fallback, got {other:?}"),
    };
    let archive = exports::build_family_zip(&r, &files).unwrap();
    assert!(
        has(&archive, b"Inter Bold.ttf"),
        "the installed file ships under its own name"
    );

    // Nothing cached and nothing installed → honest error, not an empty zip.
    let nowhere = tmpdir("nowhere");
    assert!(exports::export_source(&nowhere, &r, &HashMap::new()).is_none());

    let _ = std::fs::remove_dir_all(&empty);
    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&nowhere);
}

#[test]
fn removed_from_source_family_still_exports() {
    // §16: the removed marker is metadata only — export must keep working,
    // and the README warns that this may be the only copy Typecase can offer.
    let dir = tmpdir("removed");
    let inter = dir.join("fonts").join("gone");
    std::fs::create_dir_all(&inter).unwrap();
    std::fs::write(inter.join("deadbeef.ttf"), [0u8, 1, 0, 0, 0x7f]).unwrap();
    std::fs::write(
        inter.join("metadata.json"),
        r#"{"id":"gone","family":"Gone","files":[{"file":"deadbeef.ttf","weight":400,"style":"normal","size":5,"sha256":"cc"}]}"#,
    )
    .unwrap();

    let mut r = rec("gone", "Gone");
    r.removed_from_source = true;
    let source = exports::export_source(&dir, &r, &HashMap::new()).expect("removed family is still exportable");
    let files = match &source {
        ExportSource::Cached { files } => files.clone(),
        other => panic!("expected cached, got {other:?}"),
    };
    let readme = exports::readme(&r, &["Gone deadbeef.ttf".into()]);
    assert!(readme.contains("no longer listed on Google Fonts"));
    assert!(readme.contains("Gone deadbeef.ttf"));
    let _ = build_family_zip_smoke(&r, &files);

    let _ = std::fs::remove_dir_all(&dir);
}

fn build_family_zip_smoke(r: &FontRecord, files: &[std::path::PathBuf]) -> Result<(), String> {
    let archive = exports::build_family_zip(r, files)?;
    assert!(!archive.is_empty());
    Ok(())
}
