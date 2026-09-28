/* Integration tests: the download pipeline (M4/M5) and the §24 cache
   lifecycle. The download tests drive the real Google Fonts endpoint into a
   temp data dir and verify the committed cache layout, the content hashes,
   and the library-state write; they are ignored by default (network) and run
   explicitly with:

       cargo test --test download_pipeline -- --ignored --nocapture

   The §24 uninstall/downgrade test needs no network and runs with the default
   suite. These are the repeatable replacement for the retired
   dev_set_phase/e2e harnesses: everything except the Tauri command wrappers. */

use sha2::{Digest, Sha256};
use typecase_lib::catalog::Catalog;
use typecase_lib::downloads::{self, CacheFileMeta, FontMeta};
use typecase_lib::library::model::{FontRecord, LibraryState, Scope};
use typecase_lib::library::store::{Store, States};

fn rec() -> FontRecord {
    FontRecord {
        id: "inter".into(),
        family: "Inter".into(),
        category: "Sans".into(),
        designer: "Rasmus Andersson".into(),
        year: 2016,
        styles: 18,
        weights: vec![400],
        italic: false,
        note: String::new(),
        pairs_with: String::new(),
        popularity: 5,
        removed_from_source: false,
    }
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("http client")
}

#[test]
#[ignore = "touches the network; run with --ignored"]
fn downloads_validates_and_commits_a_family() {
    let dir = std::env::temp_dir().join(format!("typecase-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // 1. The pipeline: css2 → parse → stage → validate.
    let outcome = downloads::download_all(&client(), &dir, &rec())
        .expect("download_all must succeed for Inter");
    assert!(!outcome.files.is_empty(), "at least one cached file");
    assert!(!outcome.staged.is_empty());
    assert!(outcome.total_size > 1_000);
    for f in &outcome.files {
        assert_eq!(f.sha256.len(), 64, "sha256 hex");
        assert!(f.size >= 1_000 && f.size <= 64_000_000);
        assert!(f.weight == 400 && f.style == "normal");
    }

    // 2. Commit: staged → fonts/<id>/ + metadata.json.
    let meta: FontMeta = downloads::commit(&dir, &rec(), &outcome).expect("commit");
    assert_eq!(meta.id, "inter");
    assert_eq!(meta.file_count, meta.files.len());
    assert_eq!(meta.total_size, outcome.total_size);

    // 3. On-disk reality: files exist, sizes match, hashes are real.
    for f in &meta.files {
        let path = dir.join("fonts").join("inter").join(&f.file);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert_eq!(bytes.len() as u64, f.size, "size for {}", f.file);
        let mut h = Sha256::new();
        h.update(&bytes);
        assert_eq!(hex::encode(h.finalize()), f.sha256, "hash for {}", f.file);
        // sfnt magic survived the trip (TTF or OTF, never woff2).
        assert!(bytes.len() > 4);
        let ok = &bytes[..4] == [0x00, 0x01, 0x00, 0x00] || &bytes[..4] == b"OTTO";
        assert!(ok, "magic for {}: {:02x?}", f.file, &bytes[..4]);
    }

    // 4. Library-state transition: cached = true, nothing else fabricated.
    let mut states = States::new();
    downloads::update_library_state(&mut states, "inter");
    assert!(states["inter"].cached && !states["inter"].installed);

    // 5. Re-download is refused while the cache entry exists (explicit
    //    delete-first, per M4_DOWNLOAD_DESIGN §3).
    let again = downloads::download_all(&client(), &dir, &rec());
    assert!(again.is_err(), "duplicate cache entry must be refused");

    let _ = std::fs::remove_dir_all(&dir);
}

/// Bulk layout check across diverse families: one static-style script face,
/// one two-style serif, one multi-weight mono. Verifies the per-weight file
/// layout, content hashes on disk, the manifest, and the servable-source
/// projection (what M5 serves to the webview).
#[test]
#[ignore = "touches the network; run with --ignored"]
fn bulk_download_layout_across_categories() {
    let dir = std::env::temp_dir().join(format!("typecase-bulk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let cat = Catalog::embedded();
    let ids = ["pacifico", "lora", "jetbrains-mono"];
    let mut total_files = 0usize;
    for id in ids {
        let rec = cat
            .get(id)
            .unwrap_or_else(|| panic!("{id} must exist in the embedded catalog"))
            .clone();
        let outcome = downloads::download_all(&client(), &dir, &rec)
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        let meta: FontMeta = downloads::commit(&dir, &rec, &outcome).unwrap();

        // Per-weight layout: one file per (weight, style) the CSS2 offered,
        // at least the catalog's weights (italics add files).
        assert!(
            meta.files.len() >= rec.weights.len(),
            "{}: {} files for weights {:?}",
            id,
            meta.files.len(),
            rec.weights
        );
        let mut seen: Vec<(u16, &str)> = meta
            .files
            .iter()
            .map(|f| (f.weight, f.style.as_str()))
            .collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), meta.files.len(), "{id}: (weight, style) pairs are unique");
        for w in &rec.weights {
            assert!(
                seen.iter().any(|(fw, _)| fw == w),
                "{id}: weight {w} missing from {:?}",
                seen
            );
        }

        // Files exist, sizes match, hashes are real, magic is sfnt.
        for f in &meta.files {
            let path = dir.join("fonts").join(id).join(&f.file);
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(bytes.len() as u64, f.size);
            let mut h = Sha256::new();
            h.update(&bytes);
            assert_eq!(hex::encode(h.finalize()), f.sha256, "{}: {}", id, f.file);
            assert!(&bytes[..4] == [0x00, 0x01, 0x00, 0x00] || &bytes[..4] == b"OTTO");
        }
        total_files += meta.files.len();
    }

    // The servable-source projection sees every family and every file.
    let sources = downloads::font_sources(&dir);
    assert_eq!(sources.len(), ids.len());
    let src_files: usize = sources.iter().map(|s| s.files.len()).sum();
    assert_eq!(src_files, total_files);
    for s in &sources {
        assert!(s.files.iter().all(|f| f.url.starts_with("font://cached/")));
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// §24/§17: uninstalling never deletes the cache — but deleting the cache of
/// an installed font must work the other way too: `installed` and ownership
/// survive, yielding installed-but-missing-locally. Exercises the real
/// filesystem and a real Store round-trip; no network, runs in the default
/// suite.
#[test]
fn cache_downgrade_preserves_installed_state() {
    let dir = std::env::temp_dir().join(format!("typecase-down-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // A cached + installed face with real files and a manifest, exactly as
    // the M4 pipeline's commit step would have left it.
    let inter = dir.join("fonts").join("inter");
    std::fs::create_dir_all(&inter).unwrap();
    std::fs::write(inter.join("abcd1234.ttf"), [0u8; 2_048]).unwrap();
    let meta = FontMeta {
        id: "inter".into(),
        family: "Inter".into(),
        source: "google-fonts",
        downloaded_at: "2026-09-27T00:00:00Z".into(),
        files: vec![CacheFileMeta {
            file: "abcd1234.ttf".into(),
            weight: 400,
            style: "normal".into(),
            size: 2_048,
            sha256: "ab".repeat(32),
        }],
        file_count: 1,
        total_size: 2_048,
    };
    meta.save(&inter).unwrap();

    let mut states = States::new();
    states.insert(
        "inter".into(),
        LibraryState {
            cached: true,
            installed: true,
            managed_by_typecase: Some(true),
            install_scope: Some(Scope::User),
        },
    );
    let store = Store::new(&dir);
    store.save(&states).unwrap();

    // Sanity before the downgrade: servable, and the shell reads "installed".
    assert_eq!(downloads::font_sources(&dir).len(), 1);
    assert_eq!(states["inter"].phase(), "installed");

    // The §24 operation: delete the files, clear `cached` only.
    let freed = downloads::delete_cached_dir(&dir, "inter").unwrap();
    assert!(freed >= 2_048, "freed bytes: {freed}");
    downloads::clear_cached_state(&mut states, "inter").unwrap();
    store.save(&states).unwrap();

    // Reload from disk — persistence must reflect the downgrade exactly.
    let reloaded = store.load().unwrap();
    let st = &reloaded["inter"];
    assert!(!st.cached, "§24: the cache flag is cleared");
    assert!(st.installed, "§17: installed survives cache deletion");
    assert_eq!(st.managed_by_typecase, Some(true), "ownership survives");
    assert_eq!(st.install_scope, Some(Scope::User), "scope survives");
    assert_eq!(
        st.phase(),
        "installed",
        "installed-but-missing-locally still reads as installed for the shell"
    );

    // The servable projection follows the filesystem: no files → nothing to
    // serve, even though the library still lists the face.
    assert!(downloads::font_sources(&dir).is_empty());
    assert!(
        downloads::delete_cached_dir(&dir, "inter").is_err(),
        "a second delete has nothing left to target"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/* ---- M8: live catalog refresh (§15–16) — network, ignored by default ----

   Drives the real metadata endpoint through the Rust port of the M3
   generator: the candidate must be plausible against the embedded snapshot
   (same order of magnitude, slug parity on a sample), and a removed-family
   apply-merge must never delete state. Run with --ignored alongside the
   download tests (nightly CI). */
#[test]
#[ignore = "touches the network; run with --ignored"]
fn refresh_candidate_tracks_the_live_endpoint() {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("http client");
    let res = client
        .get(typecase_lib::catalog::refresh::METADATA_URL)
        .send()
        .expect("metadata endpoint reachable")
        .error_for_status()
        .expect("endpoint returns 200");
    let raw = res.text().expect("body reads as text");
    let payload = typecase_lib::catalog::refresh::parse_payload(&raw)
        .expect("the live payload parses (incl. junk-prefix handling)");
    let candidate = typecase_lib::catalog::refresh::build_candidate(&payload);

    // Plausibility against the embedded snapshot: the live family count is
    // within 15% of the bundled one (Google adds families steadily; a huge
    // delta means the merge or endpoint drifted).
    let cat = Catalog::embedded();
    let delta = candidate.len().abs_diff(cat.records.len());
    assert!(
        delta * 100 / cat.records.len() < 15,
        "live catalog {} vs bundled {} — drift beyond 15%",
        candidate.len(),
        cat.records.len()
    );

    // Slug parity on a live sample (the whole-catalog parity test runs
    // offline in the unit suite; this proves the endpoint still behaves).
    for r in candidate.iter().take(50) {
        assert_eq!(
            typecase_lib::catalog::refresh::slug(&r.family),
            r.id,
            "slug parity for {}",
            r.family
        );
    }

    // §16 dry run: diff the live candidate against the embedded snapshot.
    // Counts only — an apply is never executed from a test.
    let diff = typecase_lib::catalog::refresh::diff_catalog(&cat.records, &candidate);
    assert_eq!(diff.total, candidate.len());
    assert!(diff.added + diff.changed + diff.removed < candidate.len());
}
