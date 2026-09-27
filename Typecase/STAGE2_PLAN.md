# Typecase — Stage 2 Plan: Backend-Owned Library State

Replaces the demo library phases (`src/data/library.ts`, the `phase`/`local` fields
stamped onto `Face`) with real library state owned by the Rust backend and delivered
over Tauri IPC.

**Numbering note.** The Implementation Assessment numbered its stages 1–10. The
normalized data model (assessment stage 2) was folded into stage 1, so this plan covers
**assessment stage 3** and is labelled **milestone 2 (M2)** in the project sequence.
It deliberately does **not** include the generated catalog (assessment stage 4, M3):
state plumbing first, scale second.

---

## 0. Goals and non-goals

**Goals**

1. The Rust backend is the single authority for: the font catalog, the library
   (cached/installed/ownership state), and face details. The frontend holds no
   library state of its own.
2. The frontend consumes that state over intent-level IPC (`get_catalog`,
   `get_library`, `get_font_details`) and renders the same three views it renders
   today, with **zero visual redesign** — status wording may sharpen, layout does not.
3. A durable, human-inspectable store under `%LOCALAPPDATA%\Typecase\` (Linux dev:
   XDG equivalent) with the §32 layout: `catalog/catalog.json`, `fonts/<family-id>/`
   (metadata + files), `state/`.
4. Real per-face status (`Available online / Cached / Installed / …`) replacing the
   three pinned demo phases, including the empty states for Library and Installed.

**Non-goals (later milestones)**

- Downloading (`download_font`), installing (`install_font`), uninstall, export,
  external-font discovery — M4/M5/M6. M2 only models their state.
- The generated full catalog — M3 (M2 ships the same 60-family demo catalog,
  now **from the backend**).
- Offline local-font preview via the asset protocol — M5, after files actually
  exist on disk. M2 keeps the current remote-CSS preview path.

---

## 1. Target end state (acceptance demo)

```text
Launch (online)
  → Catalog loads from the backend, 60 families, statuses computed from disk state
  → Library view: empty, with a real empty state ("nothing cached yet")
  → Installed view: empty for now (Windows discovery is M6); caption explains why
Select any face → specimen works exactly as today (remote CSS path)
Restart the app → identical state (state loaded from disk, not defaults)
```

Everything above must be demonstrable with `npm run tauri dev` on Linux, with the
dev-store inspectable as plain JSON on disk.

---

## 2. Data model (Rust-owned, TS-mirrored)

```rust
// src-tauri/src/library/model.rs
pub struct FontRecord {          // catalog entry (backend-normalized)
    pub id: String,              // slug, stable
    pub family: String,
    pub category: String,        // Serif | Sans | Display | Mono | Script
    pub designer: String,
    pub year: u16,
    pub styles: u16,
    pub weights: Vec<u16>,
    pub italic: bool,
    pub note: String,
    pub pairs_with: String,
}

pub struct LibraryState {        // per-face runtime status, keyed by face id
    pub cached: bool,
    pub installed: bool,
    pub managed_by_typecase: Option<bool>,  // None = unknown (matters from M6)
    pub install_scope: Option<Scope>,       // User | System
}

pub struct FaceStatus {          // IPC payload = catalog record + library state
    pub record: FontRecord,
    pub state: LibraryState,
}
```

**Derived phase (computed in Rust, one place):**

```text
cached && installed → "installed"
cached && !installed → "library"
!cached → "online"
```

This matches today's `Phase` type exactly, so the UI switch stays untouched. The
richer §17 statuses (`Removed from Google Fonts`, `Installed externally`,
`Missing locally`) are fields on `LibraryState` now but only become visible in the
UI in later milestones.

Serialization: `serde` with `camelCase` so the TS mirror is idiomatic.

## 3. Backend structure (new Rust modules)

```text
src-tauri/src/
├── lib.rs               existing; registers the new commands
├── catalog/
│   ├── mod.rs           Catalog { records: Vec<FontRecord> } + load/embed
│   └── demo.rs          the 60-family data, transcribed from src/data/catalog.ts
├── library/
│   ├── mod.rs           Library { states: HashMap<String, LibraryState> }
│   ├── model.rs         FontRecord / LibraryState / FaceStatus / Scope
│   └── store.rs         JSON persistence under the app data dir
└── commands/
    ├── mod.rs
    ├── catalog.rs       get_catalog
    └── library.rs       get_library, get_font_details
```

Commands (already the §29 intent-level shape):

```rust
get_catalog() -> Vec<FaceStatus>          // records + computed state
get_library() -> Vec<FaceStatus>          // convenience: only non-online faces
get_font_details(id) -> Option<FaceStatus>
```

`ping`/`app_version` stay (used by tests/devtool checks) but move to `commands/mod.rs`.

## 4. Persistence design (store.rs)

- Location: Tauri's `app_data_dir()` → `%LOCALAPPDATA%\Typecase\` on Windows,
  XDG equivalent on Linux dev. Created on first run; all writes atomic
  (temp file + rename) so a crash never corrupts state.
- `state/library.json` — map of face id → `LibraryState`. The catalog itself is
  **embedded at compile time** (`include_str!` of a JSON file in the crate) for M2;
  `catalog/catalog.json` on disk is reserved for the M3 refresh story.
- Corruption handling: unparsable/missing `library.json` → log, start from the
  default (everything online), **never** fabricate cached/installed entries.
- No SQLite, no config files beyond this — §32/§39: simplest mechanism first.

## 5. Frontend changes

```text
src/
├── data/
│   ├── types.ts         Face gains nothing; Phase stays; + IPC payload types
│   ├── catalog.ts       DELETED (moves to src-tauri/src/catalog/demo.rs)
│   ├── library.ts       DELETED (replaced by backend state)
│   ├── face.ts          becomes a thin client: allFaces()/faceById()/facesInPhase()
│   │                    now call Tauri commands; keeps the same export surface
│   └── ipc.ts           NEW — typed invoke() wrappers + camelCase normalization
├── lib/fonts.ts         loadFace() keys the remote cache on face.id now (today it
│                        re-requests per Face object identity — harmless but sloppy)
└── components/*         NO changes (phase/status flow through unchanged)
```

`App.tsx` gains an async load (`get_catalog` on mount, small `lab`-style status line
while loading) and a refresh affordance is *prepared* (state + handler) but the
visible refresh button ships with M3's `refresh_catalog`.

**Race-condition note:** `Sheet`'s effect calls `loadFace(face)` keyed on object
identity; with IPC-served faces a re-render must not re-trigger font loads. `face.ts`
must return **stable object identities** (memoize by id) — same contract as today.

## 6. Migration steps (ordered, each ends green)

1. **Rust skeleton** — modules per §3, `FontRecord`/`LibraryState`/`FaceStatus` +
   serde camelCase; demo catalog transcribed; `get_catalog` returns all 60 with
   default (online) state. `cargo test` for serialization round-trip.
2. **Store** — `library.json` load/save, atomic writes, corruption fallback;
   unit tests: fresh dir, corrupted file, valid file.
3. **IPC** — commands registered, `invoke` wrappers in `src/data/ipc.ts`, TS mirror
   types; **frontend keeps using demo data** until this is proven with a devtool
   call. `cargo check` + `tsc --noEmit` green.
4. **Flip the source** — `face.ts` delegates to IPC with identity-stable memoization;
   App loads on mount with a loading state; delete `data/catalog.ts` + `data/library.ts`.
5. **States on disk** — seed one cached face (hand-written `library.json` entry)
   to prove Library/Installed views + phases reflect real state; verify restart
   persistence. (Seeding is a dev harness, not a feature.)
6. **Empty states** — Library/Installed zero-state copy in the existing visual
   language ("Nothing cached yet — pull a family from Discover. M2 has no
   download path yet."), honest about the milestone.
7. **Cleanup + docs** — `lib/fonts.ts` cache-key fix; update IMPLEMENTATION_ASSESSMENT
   checklist; run full verification suite (§8).

Steps 1–3 are Rust-first and don't touch the UI; 4 is the flip; 5–7 are polish.
Each step keeps `tsc`, `vite build`, and `cargo check` green so we can stop anywhere.

## 7. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| IPC payload shape drift between Rust and TS | Single serde model + TS mirror next to the invoke wrappers; round-trip unit test |
| Face identity instability re-triggers `Sheet` effects (specimen flicker/reload) | Memoize faces by id in `face.ts`; verify specimen switching in dev run |
| Store written to the wrong dir (dev vs installed) | Use `app_data_dir()` only; log the resolved path at startup |
| Corrupt state bricks the app | Corruption → default state + console warning; state file is plain JSON, hand-editable |
| Demo data drift between TS and Rust transcriptions | Transcribe once into Rust, delete TS immediately in the same step |
| Linux/Windows path divergence | Store code uses Tauri path APIs, no manual `%LOCALAPPDATA%` string building |

## 8. Verification checklist for M2

- `cargo test` — serde round-trip, store load/save/corruption, derived-phase logic
- `cargo check` / `tsc --noEmit` / `vite build` — all green
- Dev run on Linux: catalog from IPC, demo-seeded cached face shows in Library,
  restart preserves it, specimen behavior unchanged (spot-check 2–3 families)
- `library.json` hand-corruptible → app recovers to defaults
- No `phase`/`local` props left in the frontend; `data/catalog.ts`/`library.ts` gone
- CONTEXT.md stage checklist updated (see §9 of this plan)

---

## 10. Verification record — native-window E2E run (2026-09-27)

Steps 5 (states on disk + restart persistence) and the IPC catalog path were
verified end-to-end in the **native Tauri dev window** on Linux, closing the gap
that had only ever been proven in plain-browser dev:

- `get_catalog` over IPC served all 1,946 catalog faces inside the native
  webview (harness read + `[typecase] get_catalog: 1946 faces` in the dev log).
- `dev_set_phase` drove **online → library → installed → online** across three
  app launches; each phase was re-read over IPC after the transition and
  re-confirmed from `state/library.json` on disk after a real restart
  (camelCase payload, `managedByTypecase`/`installScope` set for installed).
- Evidence: `<app data dir>/state/e2e.log`, written through the debug-only
  `dev_e2e_report` IPC command from the temporary harness `src/dev/e2e.ts`
  (one idempotent step per launch; inert outside Tauri dev builds).
- Harness hygiene: `src/dev/e2e.ts`, `dev_set_phase`, and `dev_e2e_report` are
  debug-only surfaces (refusing bodies in release builds) and are deleted or
  replaced when M4's real download/install path lands. Step 6 (empty states)
  shipped with the M2 UI; step 7's final checklist sweep happens with M4.
- Dev-infrastructure note: long-running dev processes are hosted under a
  systemd user unit (`systemd-run --user`) because detached shell children are
  reaped unpredictably in this environment; plain `npm run tauri dev` is
  unaffected for human use.

---

## 11. M4 verification record — live download run (2026-09-27)

The M4 download pipeline (M4_DOWNLOAD_DESIGN.md) was verified end-to-end,
first at the network layer, then live inside the native Tauri window:

- **Network integration test** (`src-tauri/tests/download_pipeline.rs`, run
  with `--ignored`): real CSS2 endpoint → staged files → sfnt/size/sha256
  validation → commit → on-disk hash re-verification → library transition →
  duplicate-download refusal. It caught the missing `staging/` mkdir before
  the app ever ran. This test is the repeatable replacement for the retired
  dev_set_phase/e2e harnesses (now deleted, including their Rust commands).
- **Live run in the native window**: a dev-only bootstrap check (added for
  the run, removed after) invoked `download_font("abeezee")` from the
  webview. Result: 2 files committed (regular + italic, 44,828 + 45,824
  bytes, sha256-named), metadata.json written, `library.json` →
  `cached: true`, staging emptied — and the state survived a real app
  restart.
- **Bug found and fixed by the live run**: building a
  `reqwest::blocking::Client` inside an async command panics at drop with
  "Cannot drop a runtime in a context where blocking is not allowed". The
  client is now built once at app setup (main thread) and shared via state;
  commands clone the handle. Recorded here because it is the canonical
  reqwest-blocking pitfall for Tauri async commands.
- **Frontend integration**: Sheet's "Cache this family" button (Alt+C
  shortcut) with success/error copy; `refreshFaces()` re-absorbs the
  catalog identity-stably so the specimen never reloads spuriously;
  Library/Installed views gained honest empty states; `loadFace` falls back
  from local to remote resolution until M5 registers cached files as
  @font-face.

---

## 12. M5 verification record — offline preview (2026-09-27)

M5 (M5_OFFLINE_DESIGN.md) was verified live in two native-window runs:

- **Run 1 (online)**: with ABeeZee cached from M4, the webview fetched both
  files through the `font://` scheme — `[typecase] font:// served
  font://cached/abeezee/{b1d784c4,4f70a04a}.ttf` with byte counts matching
  the manifest (45,824 / 44,828). FontFace registration used the manifest's
  weight/style; the specimen needed no changes.
- **Run 2 (offline simulation)**: the app relaunched with all proxy env vars
  pointed at a dead port (`HTTP(S)_PROXY=http://127.0.0.1:9`, `NO_PROXY`
  covering localhost so the dev page still loads). The scheme served the
  same bytes from disk — the cached family rendered with **zero network**;
  uncached families fall back to the honest offline state (§13).
- API notes recorded for maintenance: tauri 2's scheme handler returns the
  `Response` directly (not a Result), and `register_uri_scheme_protocol`
  runs before `setup`, so the handler resolves state via
  `ctx.app_handle().state::<…>()`. Unit tests cover the strict path
  validation (charset/shape checks, canonicalized containment, traversal
  rejection) — 29 tests green in total.

---

## 13. §24 cache-deletion record (2026-09-27)

The separate, explicit cache-deletion operation CONTEXT §24 reserves (an
uninstall never deletes the cache — only this does):

- **Backend**: `delete_cached_family(id)` — validates the target is cached,
  removes `fonts/<id>/` (reporting bytes freed via `delete_cached_dir`),
  clears `cached` only (`clear_cached_state`), and persists. Installed and
  ownership fields are preserved: installed-but-missing-locally is a valid
  §17 state. Refuses non-cached targets and non-slug ids before touching
  the filesystem.
- **Frontend**: Library-view faces show "✕ Delete cached files", which opens
  an explicit confirmation dialog (Typecase visual language, focus lands on
  "Keep the cache", Escape cancels) whose copy states exactly what is lost —
  including the removed-from-Google caveat that the cache may be the only
  copy Typecase can re-offer. On success `forgetLocalFace` drops the
  registered FontFaces so the specimen falls back to remote resolution.
- **Verified live** (one-shot in-webview check, removed after): delete →
  `fonts/abeezee/` gone, `library.json` `cached:false` → restart →
  re-download → files restored, `cached:true`. 31 unit tests green
  (new: state-preserving clear, tree deletion + size reporting, invalid-id
  refusal).

---

## 14. Bulk-cache verification record (2026-09-27)

Scale exercise of the M4/M5 pipeline: 11 diverse families (Sans, Serif, Mono,
Script; 1–18 files each) cached and previewed offline.

- **Permanent vehicle**: `bulk_download_layout_across_categories` in
  `tests/download_pipeline.rs` (`--ignored`) — downloads pacifico/lora/
  jetbrains-mono against the real endpoint and asserts per-weight layout,
  unique (weight, style) pairs, on-disk sha256 re-verification, sfnt magic,
  and that `font_sources` projects every family/file with valid `font://`
  URLs. Both integration tests green (8.8s).
- **Live run 1 (online)**: 10 further families bulk-downloaded through the
  real `download_font` IPC. Final cache: 11 families, 104 files, per-weight
  layout matching the catalog exactly (Inter 18 = 9w×2s, JetBrains Mono 16,
  Merriweather/Roboto Mono 14, Open Sans/Playfair Display 12, Lora 8,
  Oswald 6 = weights-only no-italic, ABeeZee 2, Pacifico/Bebas Neue 1);
  `library.json` shows 11 × `cached: true`.
- **Live run 2 (dead-proxy offline)**: zero download attempts, zero
  googleapis/gstatic hits, and all **104 files served from disk** via the
  `font://` scheme (per-family counts identical to the layout) — every
  cached family previews fully offline (§13).
- Process notes: the temp in-webview check was idempotent (skips cached
  faces), making the offline run a pure preview probe; removed after the
  run. The one-off ABeeZee entry from earlier milestones now sits inside the
  11-family cache and is treated as legitimate local data.
- **§24/§17 downgrade path** added to the integration tests
  (`cache_downgrade_preserves_installed_state`, no network, default suite):
  real files + manifest as the pipeline's commit would leave them, real
  `Store` round-trip, then delete + clear — asserting `installed`,
  `managedByTypecase`, and `installScope` survive, the phase still reads
  `installed` (installed-but-missing-locally), the servable projection
  follows the filesystem (empty), and a second delete errors.

## 9. CONTEXT.md stage checklist

CONTEXT.md has no stage tracking today; add a living checklist (§45) so progress is
recorded against the plan rather than rediscovered. The section to add is drafted in
`STAGE2_PLAN` delivery — apply it verbatim as CONTEXT.md **§45**:

- M1 ✅ scaffold + shell + normalized model (stage 1)
- M2 (this plan) backend-owned library state (assessment stage 3)
- M3 generated catalog + browsing at scale (assessment stage 4)
- M4 downloads + cache (assessment stage 6)
- M5 source-agnostic specimen / offline preview (assessment stage 5, resequenced
  after files exist on disk)
- M6 Windows install/uninstall behind FontManager (assessment stage 7)
- M7 external fonts (assessment stage 8)
- M8 export/backup + catalog refresh + removed-font marking (assessment stage 9)
- M9 distribution: portable + installer (assessment stage 10)
- Windows-only validation (assessment stages 7–8, §38) happens at M6+ on real
  Windows; never claimed from Linux.
