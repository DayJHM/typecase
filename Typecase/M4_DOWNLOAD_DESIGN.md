# M4 Design — Validated Download Pipeline (temp file → hash → fonts/<id>/)

**Status.** Implemented and live-verified in the Tauri dev window (verification
record in STAGE2_PLAN §11). This is the design record for the M4 download/cache
pipeline: the decisions, the flow, and the M5 hook it leaves for offline
preview.

**Implementation:** `src-tauri/src/downloads/mod.rs` (pipeline + unit tests),
`download_font` command in `src-tauri/src/commands/mod.rs`.

---

## 1. Source endpoint decision (probed 2026-09-27)

`https://fonts.googleapis.com/css2?family=<Name>&display=swap` fetched **without
a browser User-Agent** (reqwest default). Google content-negotiates by UA: a
non-browser UA receives plain **TTF** (magic `00 01 00 00`), one file per
requested weight — no UA spoofing needed. Probed facts that shaped the design:

- Different `:wght@` values of a variable family (Inter 100/400/900) return
  **different instantiated files** (~325 KB each); static families likewise
  differ per weight → **the cache is per-weight**.
- `ital,wght@0,400;1,400` returns both styles; style/weight are per-`@font-face`.
- Still keyless, still the public site endpoint (CONTEXT §14 — no API key).

## 2. Pipeline (per face)

```text
For each face weight (catalog list, deduped, sorted):
  1. CSS2 fetch → parse @font-face blocks → (weight, style, url) tuples
  2. Per tuple: stream GET into <app_data>/staging/<unique>.part
  3. While reading: stream sha256; after reading: check magic
     (00 01 00 00 = TTF, OTTO = OTF) and size (≥ 1 KB, ≤ 64 MB)
  Rollback on first failure: delete staged files; library state untouched.
  Commit (all files OK):
  4. mkdir fonts/<id>/ if needed; rename staged files into place
  5. Write metadata.json {id, family, source, downloadedAt,
     files: [{file, weight, style, size, sha256}], fileCount, totalSize}
  6. Update library state (cached = true) and persist atomically
```

Step ordering: **download and validate everything first, commit only when all
weights succeeded** (§31: an incomplete download must never look like a valid
cache entry). Staging files always live in `<app_data>/staging/`, never in
`fonts/`, so a crash can never leave a half-written face inside the cache tree.

## 3. Hash and file naming (§18 identity)

- `sha256` streamed per file via RustCrypto `sha2` + `hex`. A content hash is
  required here — `std`'s `DefaultHasher` is a map hasher, not content identity.
- Committed filename = `sha256[..8].ttf|.otf`: content-addressed names, so no
  mapping table needs to stay in sync with schema versions. The `files[]`
  metadata carries `{file, weight, style, size, sha256}` so M5 (offline
  preview) and M6 (install) can pick files by weight/style without re-hashing.
- Cache reuse: the pipeline skips weights already present in `files[]` and
  refuses to run when metadata.json already exists — re-download means
  delete the family folder first (a deliberate, explicit M8 concern).

## 4. Cache layout (§32)

```text
<app_data_dir>/
├── staging/            temp area; empty when no download is in flight
└── fonts/<family-id>/
    ├── metadata.json   {id, family, source, downloadedAt, files[], …}
    └── <sha8>.ttf      one file per weight/style
```

## 5. Library state semantics

On success: `cached = true`, `installed` untouched, `managed_by_typecase` and
`install_scope` untouched (installation is M6). The derived `phase` therefore
moves `online → library` (or `installed` stays `installed` when re-adding
files to an installed-but-not-cached face). No fabricated state on failure —
a failed download leaves the face exactly as it was (§31, STAGE2_PLAN §7).

## 6. Harness removal

`dev_set_phase` and `dev_e2e_report` are deleted with M4: real state
transitions now exist. The temporary frontend harness (`src/dev/e2e.ts`) and
its App.tsx hook go with them. The state-transition evidence harness from M2's
verification is superseded by this milestone's live run.

## 7. Frontend integration (no visual redesign)

- `ipc.ts` gains `downloadFont(id)` and `getFontDetails(id)` wrappers.
- `face.ts` gains `refreshFaces()`: re-fetches the catalog and absorbs it into
  the identity-stable store (unchanged faces keep their object identity, so
  the specimen does not reload).
- Sheet's retrieval row gains one button: **"Cache this family"** for
  `phase === "online"`, showing progress while in flight ("Caching… N/M") and
  an error line on failure. Faces already cached/installed keep the existing
  copy. This is the M4-iteration of the prototype's download row — same
  visual language (`lab` buttons), no layout change.
- Rail gains honest per-view empty states (Library/Installed) replacing the
  misleading "Clear the filter to see all N" copy that leaked into filtered
  views.

## 8. Deliberate non-goals (later milestones)

- Install/uninstall (M6), external-font discovery (M7), export and catalog
  refresh (M8).
- Cancel/progress events over IPC: the command is synchronous in v1 (a family
  is a few hundred KB per weight; seconds). An event-driven progress channel
  is added only if real-world latency demands it.
- WOFF2/TTC (§12), binary parsing beyond sfnt magic (M5 adds proper
  name-table parsing if the local-bridge needs it).
