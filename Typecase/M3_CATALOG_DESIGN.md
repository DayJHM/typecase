# M3 Design — Catalog Generator (Google Fonts metadata → catalog.json)

**Status.** Implemented and verified during the M2/M3 push (see CONTEXT.md §45).
This document is the design record: it fixes the decisions the implementation
made, so the M8 refresh work and future maintenance build against a spec rather
than reverse-reading the script. Where reality diverged from an early idea, the
divergence is recorded, not papered over.

**Implementation:** `tools/generate-catalog.mjs`
**Inputs:** Google Fonts public metadata endpoint + `tools/curated-notes.json`
**Outputs:** `src-tauri/resources/catalog.json` (backend-embedded) and
`public/catalog.json` (plain-browser dev fallback)

---

## 1. Goals and constraints (from CONTEXT.md)

- §14: a **lightweight generated catalog** bundled with the application; metadata
  only, never the font repository.
- No Google API key, no user-facing Google API dependency.
- Manual, user-controlled refresh (§15) — the generator is build-time; runtime
  refresh lands in M8 on the same format.
- Source-agnostic architecture (§44): the generator is the *first source
  adapter*, not the definition of the catalog.

## 2. Source endpoint

`https://fonts.google.com/metadata/fonts` — the public JSON payload used by the
Google Fonts site itself. Chosen because it requires **no API key and no auth**,
satisfying the §14 constraint directly.

Shape observed (September 2026):

```text
{ axisRegistry: [...], promotedScript: ..., familyMetadataList: [...] }
```

Per family (relevant fields only):

```text
family, category, designers[], fonts{"400","400i",...}, axes[],
dateAdded "YYYY-MM-DD", lastModified, popularity, subsets[], isNoto
```

**Robustness note:** the endpoint prefixes the JSON with a junk `)]}'` line; the
generator strips everything before the first newline. If Google changes the
payload shape, the generator fails loudly at build time (missing required fields
throw) rather than emitting a degraded catalog.

**Known trade-off:** this is an undocumented endpoint. If it ever disappears, the
fallback plan is the documented Google Fonts Developer API (still keyless for
the *user* — the key would live in CI) or the github.com/google/fonts repo
listing. The catalog format does not change; only the adapter does. This is the
§44 "first provider, not the architectural definition" principle in practice.

## 3. Catalog format (v1)

```jsonc
{
  "generated": "2026-09-27",       // ISO date of generation
  "source": "google-fonts",        // source adapter id
  "count": 1946,
  "records": [
    {
      "id": "inter",               // stable slug (see §4)
      "family": "Inter",
      "category": "Sans",          // Typecase taxonomy (see §5)
      "designer": "Rasmus Andersson",
      "year": 2016,
      "styles": 18,                // named-instance count (§6)
      "weights": [100,200,...,900],// derived, see §6
      "italic": true,
      "note": "...",               // curated editorial note (see §7), often ""
      "pairsWith": "...",          // curated pairing, often ""
      "popularity": 5              // Google's popularity rank (Inter's actual
                                   // value); unbounded, lower = more popular
                                   // (observed range 2–2110) — for future sort
    }
  ]
}
```

Size at current library: 389 KB (1,946 records). This is bundled metadata —
acceptable per §14; it compresses to ~51 KB gzipped (measured) for the dev
fallback.

## 4. Identity: the id slug

`id = family → NFKD-normalize → strip diacritics → [^a-z0-9]+ → "-" → trim "-"`

Examples: `Inter → inter`, `Bodoni Moda → bodoni-moda`, `ABeeZee → abeezee`.

Stability rules:

- The slug is a **presentation identifier**, not the deep font identity of
  CONTEXT.md §18 (that is family/subfamily/PS-name/content-hash, computed by the
  backend from real font files in M4+). Two families that slug to the same id
  would collide. **Drift note (2026-09-27 review):** the generator as written has
  *no* uniqueness assertion — the guarantee lives only in the Rust test
  `catalog::tests::ids_are_unique_and_sorted`, so a slug collision would ship
  into `resources/catalog.json` if the generator is re-run without `cargo test`
  catching it. The fail-loudly check in the generator itself is the recorded
  intent and remains to be implemented.
- Slugs are never localized or re-derived at runtime; the backend treats the
  embedded catalog as the authority.

## 5. Category mapping

Google's five categories map onto Typecase's taxonomy:

```text
Sans Serif → Sans        Serif → Serif        Display → Display
Monospace → Mono         Handwriting → Script
```

Unknown categories map to `Sans` **with a build-time warning** (never silent).
The mapping table is the single extension point when a source adds categories.

## 6. Weight derivation

The metadata's `fonts` map lists **named instances** ("400", "700i", …), which
for variable families enumerates the full instance set (e.g. Fraunces 100–900).
So:

```text
weights = sorted unique integer prefixes of the instance keys   (fallback [400])
italic   = any instance key ends with "i"
```

This yields correct weight ranges for both static and variable families without
parsing font binaries at build time. `styles` = number of named instances.

Limitation recorded: for variable fonts, `weights` describes *instances*, not
the continuous axis range; the CSS2 API accepts interpolated weights between
named instances for `wght`-axis families, so specimen requests at intermediate
weights work. The specimen already requests the derived list.

## 7. Curated editorial layer (two-tier data model)

`tools/curated-notes.json` (extracted once from the prototype catalog by
`tools/extract-curated-notes.mjs`) holds human editorial data for 74 families:
`designer`, `year`, `note`, `pairsWith`.

Merge rule: **curated values override generated values** per field; everything
else comes from the endpoint. Rationale:

- For non-curated families, the endpoint's `designers` (list) and `dateAdded`
  are acceptable defaults; the prototype's curated strings are better editorial
  (e.g. "Mike Abbink / Bold Monday" vs "Bold Monday; Abbott, Mike"). Every one
  of the 74 curated rows carries `designer` and `year`, so curated values
  always win where a family is covered — the defaults only fill the other
  1,872.
- `note`/`pairsWith` exist nowhere in the endpoint — the overlay is their only
  source. 74 of 1,946 families carry notes; the rest render the documented
  empty states.

The overlay is data, not code: adding notes for new families means appending
JSON entries and re-running the generator.

## 8. Ordering contract (the collation lesson)

Records are sorted by `family` in **code-point order** (JS `<`/`>` comparison),
*not* `localeCompare` — and **only** `family` is ordered: ids are *not*
globally sorted (lowercasing makes slug order diverge from family order in
~1,050 of 1,946 positions), so id lookup stays a linear scan (`Catalog::get`),
never a binary search. The Rust backend sorts strings by UTF-8 byte order
(`String: Ord`), and the two collations disagree on case and punctuation
(`localeCompare` puts "eB" before "EB"; code-point order matches Rust).

Contract: the embedded file is byte-identical to what a Rust re-sort would
produce, and `catalog::tests::ids_are_unique_and_sorted` enforces it. Any future
generator or catalog-patcher must preserve this ordering or break that test.

## 9. Outputs and consumers

```text
src-tauri/resources/catalog.json   embedded via include_str! at compile time —
                                   the bundled snapshot (§15: ships with the app)
public/catalog.json                served to plain-browser dev (vite) so the UI
                                   is verifiable without the Tauri runtime
```

Runtime authority in the Tauri app is the backend (`get_catalog` over IPC);
`public/catalog.json` is a dev convenience. **Drift note (2026-09-27 review):**
the earlier claim that the CSP would block a packaged-app fetch was wrong —
`default-src 'self'` *permits* a same-origin fetch of `/catalog.json`; what
actually prevents it is the `isTauri()` branch in `src/data/ipc.ts`. Also note
vite copies `public/` into `dist/` on `tauri build`, so packaged builds
currently ship a redundant ~389 KB copy nothing reads — a `publicDir: false`
variant for the Tauri build is the clean fix (candidate for M9).

## 10. Determinism and regeneration

- `node tools/generate-catalog.mjs` fetches and writes; `--in <file>` re-runs
  offline from a saved payload (intended for tests/CI and air-gapped rebuilds;
  nothing automated invokes it yet).
- Output ordering and field set are deterministic; the only variable field is
  `generated`. Downstream (the embedded snapshot) should treat byte-diffs as
  meaningful: a changed catalog is a changed bundled snapshot and should be
  reviewed like code.

## 11. M8 refresh design (specified now, implemented in M8)

CONTEXT.md §15–16 define refresh behavior; this is how the existing pieces
satisfy it:

1. **Backend command `refresh_catalog`** (M8) fetches the same endpoint at
   runtime, runs the *same merge* (port of this generator into Rust, overlay
   included), and produces a candidate catalog in memory.
2. **Diff against embedded**: classify each family as
   `unchanged | added | changed | removed-from-source`.
3. **Non-intrusive notification** (§15): a `lab`-style notice — "Catalog update
   available: N new, M removed" — never auto-applied.
4. **User accepts** → candidate replaces `catalog/catalog.json` on disk (§32
   layout, already reserved); the embedded snapshot remains the offline floor.
   Load order at startup: disk catalog if present, else embedded.
5. **Removed families** (§16): marked `removedFromSource: true`, never deleted;
   cached/installed state is untouched; preview/install/export keep working.
   This adds one optional field to the record schema — v1 format anticipates it
   without breaking parsers that ignore unknown fields.

The frontend `refresh_catalog` affordance (state handler already stubbed in
App.tsx planning) ships with M8, not before.

## 12. What this design deliberately does NOT do

- No runtime fetching outside the M8 refresh command (§30: network constrained
  to known functionality).
- No font binaries in the catalog (§14).
- No user-facing configuration of sources or endpoints.
- No incremental/automatic sync (§15: manual, user-controlled only).
- No schema version field yet — single-source v1; the `source` field plus the
  M8 diff flow makes a version field necessary only when a second source or a
  breaking format change arrives, at which point `catalogVersion` is added.
