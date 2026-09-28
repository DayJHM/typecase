# M8 Design — Export/Backup, Catalog Refresh, Removed-from-Source Marking

**Status.** Implemented and verified (see CONTEXT.md §45). This is the design
record: the decisions the implementation made, so future work builds against a
spec. §38 line throughout: the refresh merge and the ZIP layer are fully
runtime-verified (69 unit tests + the live-endpoint integration test); the
*end-to-end UI flows* run in the Tauri window on Windows and are otherwise
compile-validated by CI.

**New modules:**
`src-tauri/src/catalog/refresh.rs` — Rust port of the M3 generator + diff +
apply-merge
`src-tauri/src/exports/mod.rs` — dependency-free ZIP writer + export sources
**New commands:** `refresh_catalog`, `apply_catalog_refresh`, `export_font`
**New integration test:** `tests/export_backup.rs` (default suite) +
`refresh_candidate_tracks_the_live_endpoint` (ignored, network, runs on
nightly CI)

---

## 1. Export / backup (§11, §26)

### Archive format and dependency policy

ZIP with **stored (uncompressed) entries**, built by a ~120-line writer in
`exports/mod.rs`: CRC-32 (table-free bitwise), local headers, central
directory, EOCD. No new crates (§37). Rationale: TTF/OTF are already
compressed containers — deflating them inside a ZIP buys ~0 — and stored
entries make the writer trivially auditable and the archive byte-predictable
in tests. UTF-8 filename flag set; fixed DOS epoch timestamps (the README
carries the real stamps).

**Acceptance tests:** CRC vectors (`"123456789"` → 0xCBF43926), EOCD offset
arithmetic (what every real parser walks), payload byte-identity, and the
integration test's full cache→ZIP→file round-trip.

### What goes in

Every cached file of the family (content-addressed `<sha8>.ttf|otf`) renamed
to `<Family> <sha8>.<ext>` inside the archive, plus `README.txt` — the §11
"useful metadata": family, id, category, designer, year, OFL licence line,
and the file list. Removed-from-source families get an explicit warning
block in the README (§26: backups matter most exactly there).

### Source resolution: cache first, installed as fallback

`export_source()` returns `Cached { files }` from the manifest, else
`Installed { files }` from the M6 install record (files that still exist).
This keeps export working after a §24 cache deletion on an installed family.
Neither → honest error, never an empty archive.

### Destination policy (§31)

The backend chooses `exports/` under the app data dir (§32 layout — a sixth
top-level entry alongside catalog/, fonts/, staging/, state/); the frontend
supplies only the family id. File name `<id>-typecase-export.zip`, staged
through `.part` + fsync + rename (§31: a failed export never looks like a
valid backup). Id charset enforced before any filesystem work. A cache
deletion can therefore never eat a backup, and no frontend-supplied path ever
reaches the filesystem (§30/§31).

### UI

`ExportButton` in the Sheet's retrieval row — always visible (every phase can
export), with outcome feedback (N files · cached/installed copy). §25's
external-removal dialog keeps pointing at export as the backup route;
`cache_external_font` (§29) remains unimplemented and the dialog copy still
says so honestly.

## 2. Catalog refresh (§15)

### The merge is a port, not a reimplementation

`catalog/refresh.rs` re-expresses `tools/generate-catalog.mjs` in Rust:
slug, category mapping, weight derivation, curated overlay, family code-point
sort. The guard is a **parity test**: `slug()` must reproduce the id of every
one of the 1,946 embedded records. Two port subtleties the tests forced into
the open:

- **Combining marks** are stripped explicitly (the JS regex
  `[\u0300-\u036f]`); without that, ñ → `n-andu` instead of `nandu`.
- **Sort order**: JS `<` on families is UTF-16 code-unit order; Rust
  `String::cmp` is UTF-8 byte order. They agree on BMP characters (all
  current family names) — the test asserts the actual Rust order
  ("Alpha" < "aB" < "beta"), and the embedded-vs-refresh contract holds
  because both sides of the diff compare Rust-sorted sequences. Revisit only
  if a source ever emits astral-plane family names.

Endpoint details preserved: the `)]}'` junk-prefix tolerance, keyless fetch,
fail-loudly parse (a malformed payload is an Err, never an empty catalog —
the M4 staging discipline applied to metadata).

### The refresh flow (§15: manual, non-destructive, notified)

1. **`refresh_catalog`** (async): fetch + parse + `build_candidate` on a
   blocking thread (no lock across network I/O), then diff against the
   active catalog.
2. **Diff** (`RefreshDiff`): added/changed/removed counts plus capped
   name samples for the notice; `unchanged` when all three are zero.
   "Changed" is whole-record metadata equality (weights, designer, year,
   styles, italic, popularity) — popularity changes count, deliberately:
   they are the most common metadata movement and cheap to apply.
3. **Park, don't apply**: a non-unchanged diff parks `PendingRefresh {
   merged, diff }` in the state. The merged vector is precomputed via
   `apply_merge` so applying is O(save).
4. **Notice**: dismissible border box — "Catalog update: N new, M updated,
   R removed" with sampled names. **Apply** replaces
   `catalog/catalog.json` on disk (atomic tmp+fsync+rename) and swaps the
   `RwLock<Catalog>`; **Not now** clears nothing on the backend (the parked
   candidate is simply replaced by the next refresh — parking one slot is
   the whole state machine).
5. Load order at startup is `Catalog::for_dir`: disk catalog if present and
   parseable and non-empty, else embedded. Corrupt disk → embedded + a log
   line (§4's never-fabricate policy). The embedded snapshot remains the
   offline floor (§15).

### State shape change

`TypecaseState.catalog` became `RwLock<Catalog>` (commands only read; the
apply path is the single writer, swapping the whole struct atomically). All
`state.catalog` accesses became `state.catalog.read()`. Catalog access never
nests inside the library-state mutex in the new code; existing lock order
(states → installs) is unchanged.

## 3. Removed-from-source marking (§16)

- `FontRecord.removedFromSource: bool` — `serde(default)`, skipped when
  false, camelCase on the wire; the TS `Face` mirror gains
  `removedFromSource` and `toFace` normalizes it.
- `apply_merge` sets the flag on families absent from the candidate,
  **clears** it for families that return (§43's "Typecase detects catalog
  change" works in both directions), and is idempotent. It never deletes a
  record; cache/install state lives in `library.json` and is not touched by
  any of this.
- Presentation: Sheet banner (vermilion box + what the user can still do),
  Rail row marker (✝ superscript with the badge as tooltip), and a summary
  count in the catalog action bar when any exist. Phase logic is untouched —
  a cached removed family still reads `library`, an installed one still
  reads `installed`, preview/install/export all keep working (§16).
- Verification: unit tests for all three flag transitions; the live network
  test runs a diff (never an apply) against the real endpoint.

## 4. What this deliberately does NOT do (§42)

- No automatic/background refresh — the only trigger is the user's button.
- No `cancel_download`-style cancellation; refresh is a single fast request.
- No diff viewer UI beyond the sampled names; the notice names the changes.
- No `cache_external_font` (§29 lists it; it is still future work).
- No schema version bump: the catalog file gains only the optional per-record
  flag, which old parsers ignore by `serde(default)` contract.
