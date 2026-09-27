# Typecase — Implementation Assessment

Produced per CONTEXT.md §4, from inspection of `frontend/font-browser-and-installer.zip`.
The ZIP was extracted to `Typecase/frontend/_extracted/` for reference (temporary; safe to delete
or gitignore once migration begins). **No implementation work has been done yet.**

---

## A. Current architecture

**Stack:** React 19.2.6 · TypeScript 5.9.3 (strict mode) · Vite 7.3.2 · Tailwind CSS 4.1.17 via
`@tailwindcss/vite` · `vite-plugin-singlefile` · `clsx` + `tailwind-merge` (the `cn()` helper).
No router, no state library, no tests, no lint config. `package.json` has no `typecheck` script.

**Files (all verified, matching §6 of CONTEXT.md):**

```text
index.html              loads Bodoni Moda + IBM Plex Sans/Mono from Google Fonts CDN for the UI chrome
src/App.tsx             landing page: marketing hero (composing-stick photo), the tool itself, a
                        "type case" photo plate, <Dossier/>, colophon footer
src/components/Rail.tsx sidebar: search box, category filter buttons, family list; each row renders
                        its own name in its own face, lazily loading the webfont via
                        IntersectionObserver + blur-in animation
src/components/Sheet.tsx the specimen room: sticky control bar (size/weight/tracking/leading sliders
                        + preset switcher), editable contentEditable specimen line, size gauge,
                        waterfall, two-column body block, glyph case, specimen note, pairing link,
                        retrieval row (raw Google download link, copy CSS / family name, GF page link)
                        and a static "installer notes" panel containing a copy-paste PowerShell script
src/components/Dossier.tsx pure marketing/documentation: competitor survey, stack comparison table
                        ("Rust over Tauri" verdict incl. the AddFontResourceExW/FR_PRIVATE recipe),
                        requirements list. Zero product logic.
src/data/catalog.ts     hand-curated 60-family catalog with terse single-letter fields
                        (n/c/d/y/s/w/i/t/p), PRESETS (essay/headline/…), INSTALL_PER_USER
                        (PowerShell string), EXISTING/REQS/STACK (Dossier data)
src/lib/fonts.ts        Google CSS2 API loader: injects <link> tags with a 3-stage fallback
                        (italics+weights → weights → default), per-family promise cache;
                        specimenUrl/downloadUrl/cssSnippet/copyText helpers
src/utils/cn.ts         clsx + tailwind-merge
src/index.css           the design system: paper/ink/vermilion tokens as CSS vars, SVG-noise grain
                        overlay, .disp/.lab/.num/.mono type classes, setline/slipin animations,
                        custom range-slider styling, contenteditable placeholder/caret styling
public/images/*.jpg     three letterpress photos used by the hero/plate/verdict sections
```

**State flow:** everything is local `useState`/`useMemo` inside `App` (selected face, query,
category, preset id) and `Sheet` (size/weight/track/lead/typed text). No persistence of any kind.

**Font loading:** runtime injection of `fonts.googleapis.com` stylesheet links keyed by family,
with a promise cache and 7-second timeout, then graceful fallback to system serif. This is the
only "network" behavior; "download" is a plain `<a href>` to `fonts.google.com/download?family=…`.

---

## B. Reusable components (retain)

* **The entire design system** (`index.css`): tokens, `.disp`/`.lab`/`.num` classes, grain,
  animations, slider styling. This is the distinctive editorial identity §5 mandates preserving.
* **Sheet's specimen core**: the control bar, editable specimen line, waterfall, body block,
  glyph case, pairing, preset system, blur-in `setline` animation. This is the product's heart.
* **Rail** including the lazy `FaceName` IntersectionObserver pattern (must be re-pointed at a
  source-agnostic loader).
* **`cn.ts`**, the **PRESETS** concept, the App grid skeleton (masthead / rail / sheet).
* The letterpress imagery is part of the visual identity; whether the *marketing* framing around
  it survives is a product decision (see D).

## C. Components requiring modification

* **`data/catalog.ts`** → replaced by a normalized **Typecase font resource** type (stable id,
  family, categories, designer, weights, italics, style count, description, pairing, plus runtime
  status: remote-availability / cached / installed / scope / managed-by-Typecase). The 60-face
  hand-curated list is demo data (§8) and must stop being the source of truth.
* **`lib/fonts.ts`** → split by responsibility. `copyText` (and possibly `cssSnippet`) stay in the
  frontend. All Google-specific network behavior (CSS2 loading for specimens, download URLs) moves
  behind Tauri IPC into Rust (§9), or becomes a source-agnostic resource resolution layer: remote
  CSS URL for online fonts, local `@font-face` via Tauri custom asset protocol for cached/installed
  fonts. `loadFace` must stop assuming Google.
* **`Sheet.tsx`** → consume the normalized resource (§33); remove the Google download `<a>`, GF
  page link, and the static PowerShell panel; replace with real intent-level actions
  (download with progress/cancel, install with scope choice + confirmation, export). Keep it
  visually recognizable.
* **`Rail.tsx`** → status facets (Available online / Cached / Installed / External / Removed) in
  addition to category; **virtualization or windowing** — the real catalog is ~1,800+ families vs
  the prototype's 60; DOM-per-row with per-row observers won't scale as-is.
* **`App.tsx`** → strip hero/dossier/colophon marketing; become the app shell with navigation
  between Discover / Library / Installed views while keeping the masthead typography.
* **`index.html`** → the UI chrome fonts (Bodoni Moda, IBM Plex) are fetched from Google at
  launch; for offline-first behavior (§13) they must be bundled as local assets instead.
* **Build**: drop `vite-plugin-singlefile` (unnecessary inside Tauri); add `@tauri-apps/api`;
  add a `typecheck` script; add `src-tauri/` (Tauri 2 config + capabilities).

## D. Components/files that should eventually be removed

* **`Dossier.tsx`** — prototype marketing/documentation, explicitly non-authoritative (§10). Its
  Windows-install "recipe" (copy → HKCU registry → `AddFontResourceExW` + **FR_PRIVATE** →
  `WM_FONTCHANGE`) is a hypothesis; per §22 it must **not** be implemented as-is and certainly not
  with `FR_PRIVATE` as the persistence mechanism. Remove with the component.
* `EXISTING`, `REQS`, `STACK`, `INSTALL_PER_USER` in `catalog.ts` — same category.
* The 60-family curated catalog once a generated catalog exists (§8).
* The three marketing JPGs (~800 KB): decide during the shell rework — keep as first-run/About
  texture if the identity wants them, otherwise drop for size. Low-risk either way.

## E. Required backend additions (Rust / Tauri 2)

* **Commands** (intent-level, per §29): `get_catalog`, `refresh_catalog`, `get_library`,
  `get_installed_fonts`, `get_font_details`, `download_font` (+progress events, `cancel_download`),
  `install_font(id, scope)`, `uninstall_font`, `cache_external_font`, `export_font`,
  `export_family`. No generic shell/registry/file commands (§30).
* **Font identity/parsing**: TTF/OTF metadata extraction (family, subfamily, full/PS name,
  version) + content hashing; this powers duplicate detection, external matching, cache
  validation (§18). Candidate: `ttf-parser` (already named in the prototype's stack table and a
  good fit — pure parsing, no rendering needed for v1).
* **Windows font discovery** (enumerating actually-installed fonts incl. external ones) and
  **install/uninstall** behind a `FontManager` abstraction (§21). The concrete per-user vs
  system-wide mechanism (registry vs DWrite vs shell APIs, elevation strategy for system-wide)
  must be **validated on real Windows** before being treated as settled (§22, §38). Note: Tauri 2
  has no built-in "elevate just this operation" — the elevation approach needs a deliberate design
  (e.g. a separate elevated helper process) and Windows testing.
* **Download pipeline**: `reqwest` → temp file → validate (parse + hash) → move into cache; never
  expose partial downloads as valid entries (§31). ZIP extraction with path-traversal protection.
* **Library state persistence** under `%LOCALAPPDATA%\Typecase\` with the §32 layout; simplest
  reliable mechanism first — JSON files, no SQLite unless a concrete need appears (§32, §39).
* **Catalog generator**: a small build-time tool that turns Google Fonts metadata into a bundled
  `catalog.json` (§14) — no API key, no full repo clone. Refresh fetches/compares against this.
* **Export**: ZIP assembly (font files + metadata/licence) fully offline (§26).

## F. Migration risks

1. **Windows-only behavior cannot be verified here** (Linux dev). Per §38 nothing install-related
   may be claimed as verified; the install/uninstall/elevation design must be treated as
   provisional until tested on Windows 10/11.
2. **CSP/WebView2**: the prototype loads webfonts from `fonts.gstatic.com` and would continue to
   for remote fonts; Tauri's default CSP must be configured for that, while cached fonts use the
   custom protocol. A broken CSP manifests exactly like "offline" — confusing during bring-up.
3. **Catalog scale**: 60 → ~1,800 families breaks naive list rendering and naive `loadFace`
   (sixty `<link>` tags is fine; thousands is not). Requires windowing + deliberate loading
   strategy (e.g. load on selection/hover, cache aggressively).
4. **Terse field names** (`n/c/d/y/s/w/i/t/p`) are baked into every component; the rename to the
   normalized model touches everything — do it in one dedicated migration step, not piecemeal.
5. **contentEditable specimen line** interacts with React re-renders via manual `textContent`
   syncing; changes to how/when the face re-renders (async local-font loading) can regress the
   typing experience. Test custom-text editing after every loader change.
6. **Offline-first inversion**: today everything is online-only; the app must render usefully
   with zero network from day one of the Tauri port, which affects the UI-chrome fonts (C) and
   empty states.

## G. Recommended implementation order

1. **Tauri scaffold + shell rework**: wrap the extracted frontend in a Tauri 2 app; strip
   marketing (Dossier/hero/colophon) into a clean Discover/Library/Installed shell; keep the
   design system intact; bundle UI-chrome fonts locally. Milestone: app launches in Tauri,
   specimen still works online, no visual regression in the specimen room.
2. **Normalized data model**: define `TypecaseFontResource`; migrate Rail/Sheet/App onto it in
   one pass; keep the demo catalog as temporary data source behind the new interface.
3. **Rust foundation**: storage layout, font parsing/identity, library state; IPC for
   `get_catalog`/`get_library`/`get_font_details`.
4. **Catalog generator + full catalog browsing**: build-time generation of `catalog.json`,
   bundled snapshot, virtualized Rail with search/filter.
5. **Source-agnostic specimen**: local `@font-face` via Tauri asset protocol for cached/installed
   fonts, remote CSS path for online ones, explicit offline state. Milestone: preview works with
   networking disabled.
6. **Downloads**: validated temp-file pipeline, progress/cancel events, cache layout.
7. **Install/uninstall (per-user first)**: implement behind `FontManager`, then validate on real
   Windows (visibility to other apps, persistence across exit/reboot, duplicates). System-wide +
   elevation design comes after per-user is proven.
8. **External fonts**: discovery, ownership classification (managed/external/unknown), cache/
   backup offer, warning + explicit confirmation on removal.
9. **Cache/uninstall split semantics, export ZIP, catalog refresh + removed-from-Google marking,
   error/confirmation states.**
10. **Distribution**: portable build + installer via Tauri bundler; Windows test pass for the §43
    success workflows.

Stages 1–5 and most of 3–4 are verifiable on Linux (TS build, `cargo test` for parsing/catalog/
state logic); 7–8 and final acceptance require real Windows runs (§38).

---

**Immediate next step** when implementation is authorized: stage 1 (Tauri scaffold + shell rework),
keeping every change incremental and the specimen room untouched except for its data source.
