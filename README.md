# Typecase

**A local-first Windows font manager whose first online provider is Google Fonts.**

Typecase is a lightweight desktop application for discovering, previewing,
caching, installing, and managing fonts — built as a specimen room first, not
a settings panel. It is built with [Tauri 2](https://tauri.app) (Rust backend,
React + TypeScript + Tailwind frontend) and targets **Windows 10 (1809+) and
Windows 11**.

The editorial specimen UI is carried over from the original prototype and is
the product's heart: type is set against foundry paper, presets ask "which
face for which job", and the whole room works in English and Spanish, light
and dark.

- No account, **no telemetry**, no tray agent
- **No Google API key** — the catalog is a bundled, keyless-generated snapshot
- Font files are only ever written through the app's own validated pipeline

## Features

- **Discover** — browse and search 1,946 Google Fonts families with a
  windowed index that stays fast at scale
- **Specimen room** — editable specimen line, waterfall, text block, glyph
  case, document presets, pairing suggestions
- **Cache** — one click downloads every weight of a family, streamed through
  SHA-256 + sfnt validation into a content-addressed local cache
  (`fonts/<family-id>/<sha8>.ttf` + manifest); all-or-nothing commit
- **Offline preview** — cached families render from disk via the custom
  `font://` scheme, registered as real `FontFace` objects; the app stays
  useful with the network fully cut
- **Install / uninstall (Windows)** — per-user (no UAC) or system-wide (single
  elevation for the operation) through a `FontManager` abstraction using the
  Windows 10 1809+ per-user font mechanism; uninstall reverses exactly what
  Typecase wrote
- **EN / ES**, dark mode with live OS-scheme follow, both persisted

## Milestones

The living checklist is [Typecase/CONTEXT.md §45](Typecase/CONTEXT.md).

| | Milestone | Status |
| --- | --- | --- |
| M1 | Tauri scaffold, shell, normalized face model | ✅ |
| M2 | Backend-owned library state over IPC | ✅ |
| M3 | Generated 1,946-family catalog + browsing at scale | ✅ |
| M4 | Validated download pipeline (temp file → hash → cache) | ✅ |
| M5 | Offline preview (`font://` + FontFace) | ✅ |
| M6 | Windows install/uninstall behind `FontManager` | ✅ * |
| M7 | External font discovery, ownership, removal warning | planned |
| M8 | Export/backup, catalog refresh, removed-from-source marking | planned |
| M9 | Distribution: portable + installer | planned |

\* M6 is implemented and unit-tested (40 tests) with the Win32 code paths
compile-validated on Windows CI; it is **done** when the §5 VM session in
[WINDOWS_VALIDATION.md](Typecase/WINDOWS_VALIDATION.md) passes on real
Windows. Linux testing never substitutes for Windows validation (CONTEXT §38).

## CI

[![Windows CI](https://github.com/DayJHM/typecase/actions/workflows/windows.yml/badge.svg)](https://github.com/DayJHM/typecase/actions/workflows/windows.yml)

`.github/workflows/windows.yml` runs on **windows-latest** on every push/PR:

1. `npm ci` → `tsc` → `vite build`
2. `cargo test --locked` (unit + non-network integration tests)
3. `tauri build --bundles nsis,msi` — installers uploaded as artifacts
4. **Nightly (03:23 UTC)** and on-demand (`workflow_dispatch` → *network
   tests*): also runs the network integration suite — the real Google Fonts
   endpoint exercised from a Windows IP

Every nightly run produces fresh installers, so a validation VM session can
always start from a current Windows-built binary.

## Validation workflow

Windows-only behavior is validated in two tiers (CONTEXT §38):

- **Tier 1 — automated (this repo's CI)**: compilation, unit/integration
  tests, and packaging on real Windows on every change.
- **Tier 2 — manual, scripted**: [Typecase/WINDOWS_VALIDATION.md](Typecase/WINDOWS_VALIDATION.md)
  turns CONTEXT §38 into an evidence-based VM session — installer/UAC checks,
  UxTheme title-bar flips, ES + RTL on WebView2, offline preview via real
  Flight mode, per-user install with registry captures (`HKCU\...\Fonts`, no
  `FR_PRIVATE`), reboot persistence, external-font handling, and a
  release-candidate sweep. Items gate their milestones (`5.x [M6+]`,
  `6.x [M7/M8]`).

## Development

Prerequisites: Node 22, a stable Rust toolchain, and the
[Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```bash
cd Typecase
npm ci

npm run tauri dev        # native window (vite on :1420 + Rust watcher)
npm run dev              # browser-only dev (catalog served from public/)
npm run typecheck        # tsc --noEmit
npm run build            # tsc + vite production build

cd src-tauri
cargo test                                        # 40 unit + integration tests
cargo test --test download_pipeline -- --ignored  # network-tier tests (real endpoint)
```

Regenerate the catalog (requires network; `--in <file>` re-runs offline):

```bash
node tools/generate-catalog.mjs
```

### Layout

```text
Typecase/
├── src/                  React UI (components, i18n, theme, data/IPC layer)
├── src-tauri/            Rust backend
│   └── src/
│       ├── catalog/      embedded generated catalog
│       ├── commands/     intent-level IPC (§29)
│       ├── downloads/    M4 pipeline + M5 font:// serving + §24 deletion
│       ├── fontmanager/  M6 abstraction + Windows/unsupported impls
│       └── library/      records, store, state model
├── tools/                catalog generator + curated-notes extraction
├── docs                  CONTEXT.md (spec), M*_DESIGN.md, STAGE2_PLAN.md,
│                         WINDOWS_VALIDATION.md, .github/workflows/
└── frontend/             the original prototype ZIP (reference, untouched)
```

## Licences

- Bundled UI fonts (Bodoni Moda, IBM Plex Sans/Mono) and every family in the
  catalog are released under the **SIL Open Font Licence 1.1**.
- The Google Fonts metadata used to generate the catalog is fetched from the
  public keyless endpoint at build time; no Google API key is involved.
- Application code licence: see CONTEXT.md (to be fixed before first release).
