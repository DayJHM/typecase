# Typecase — Agent Context

## 1. Read This First

This file is the primary project context and instruction document for the Typecase implementation.

Before making any code changes:

1. Read this entire file.
2. Inspect the existing frontend prototype at:
   `frontend/font-browser-and-installer.zip`
3. Understand the existing frontend architecture and visual design.
4. Compare the existing implementation against the requirements in this document.
5. Produce an implementation assessment before beginning substantial implementation work.

**Do not immediately rewrite or restructure the project.**

The existing frontend is a prototype that contains useful working UI and behavior. The goal is to evolve it into Typecase, not discard it and build an unrelated application.

---

# 2. Project Identity

**Project:** Typecase
**Platform:** Windows 10/11
**Desktop framework:** Tauri 2
**Backend:** Rust
**Frontend:** React + TypeScript + Vite + Tailwind
**Initial font source:** Google Fonts
**Distribution:** Portable executable + normal installer
**Telemetry:** None

Typecase is a lightweight, local-first Windows font discovery, specimen, caching, installation, and management application.

Core workflow:

> Discover → Preview → Download/Cache → Install → Manage → Export

Google Fonts is the **initial source**, not the architectural definition of the application.

The architecture must allow additional font sources in the future without requiring a major rewrite.

---

# 3. Existing Frontend Prototype

The existing frontend prototype is provided at:

```text
frontend/
└── font-browser-and-installer.zip
```

This ZIP is the current frontend reference implementation.

It must be inspected before implementation begins.

The prototype is important because it contains the existing:

* visual language
* specimen experience
* component structure
* typography
* interactions
* frontend state flow
* Google Fonts integration concepts
* styling
* assets

Do not assume the prototype is architecturally correct for the final application.

Do not assume every existing implementation detail should be preserved.

Instead:

> Preserve what is good and functional while replacing prototype-specific architecture where necessary.

---

# 4. Required Initial Agent Phase

Before modifying the project, inspect:

```text
frontend/font-browser-and-installer.zip
```

Determine:

* frontend framework/version
* dependency structure
* component architecture
* current state management
* current font-loading mechanism
* current catalog structure
* existing specimen behavior
* existing download behavior
* existing styling system
* build configuration
* potential reuse opportunities
* technical debt
* prototype-only code
* conflicts with this document

Then produce an **Implementation Assessment** containing:

### A. Current architecture

What exists and how it works.

### B. Reusable components

What should be retained.

### C. Components requiring modification

What needs to change and why.

### D. Components/files that should eventually be removed

For example, prototype-only data or documentation.

### E. Required backend additions

What Rust/Tauri functionality is required.

### F. Migration risks

Anything that could cause regressions.

### G. Recommended implementation order

A staged plan.

**Do not begin major implementation until this assessment has been produced.**

---

# 5. Existing Prototype UI

The existing specimen UI is a major product asset.

Preserve its distinctive editorial/type-specimen visual identity.

Do not turn Typecase into a generic:

* Windows Settings clone
* file manager
* dashboard
* SaaS interface
* generic font-manager UI

Functional UI additions are expected for:

* Discover
* Library
* Installed
* download status
* cache status
* installation status
* external font status
* offline state
* confirmations
* errors

However, additions should visually belong to the existing Typecase design.

Do not perform a wholesale redesign unless explicitly requested.

---

# 6. Existing Frontend Components

The prototype is expected to contain components similar to:

```text
src/
├── App.tsx
├── components/
│   ├── Rail.tsx
│   ├── Sheet.tsx
│   └── Dossier.tsx
├── data/
│   └── catalog.ts
├── lib/
│   └── fonts.ts
├── utils/
│   └── cn.ts
├── index.css
└── main.tsx
```

These names describe the known prototype architecture, but the agent must verify the actual ZIP contents rather than relying on this list.

---

# 7. Specimen Component

The existing `Sheet.tsx` specimen experience should be retained where practical.

It currently represents functionality such as:

* custom specimen text
* font size
* weight
* tracking
* leading
* presets
* waterfall
* body text
* glyph/case views
* font pairing
* download-related actions

The production specimen should remain recognizable.

However:

> `Sheet.tsx` must become font-source agnostic.

It must not contain assumptions that a font necessarily comes directly from Google Fonts.

The specimen should consume a normalized Typecase font resource.

Conceptually:

```text
Font source
    ↓
Typecase font resource
    ↓
Specimen renderer
```

The specimen should eventually work with:

* Google Fonts online
* cached Typecase fonts
* installed Typecase fonts
* externally installed fonts where technically appropriate

---

# 8. Prototype Catalog

The existing prototype catalog is not production data.

If the prototype contains:

```text
src/data/catalog.ts
```

treat it as demo/placeholder data.

Do not continue expanding it manually.

Production should use a generated Google Fonts catalog.

Once the generated catalog is working, the prototype catalog should no longer be the source of truth.

---

# 9. Prototype Font Utilities

If the prototype contains Google-specific font utilities such as:

* CSS URL generation
* Google Fonts loading
* Google download URLs
* loaded-family tracking
* specimen URL creation
* clipboard helpers

inspect them carefully.

Do not blindly delete them.

Determine which functionality belongs in:

```text
React
```

and which should move behind:

```text
Tauri IPC → Rust
```

Google-specific network/download logic should generally move to the backend.

---

# 10. Dossier / Prototype Documentation

If the prototype contains a `Dossier.tsx` or similar architectural/documentation component, it is not authoritative.

Older prototype assumptions about:

* application size
* Windows font installation
* registry operations
* `AddFontResourceExW`
* `FR_PRIVATE`
* `WM_FONTCHANGE`

must not be treated as final implementation requirements.

Validate Windows behavior before implementing it.

---

# 11. Product Requirements

## Discovery

Users must be able to:

* browse fonts
* search fonts
* filter fonts
* open a family
* inspect styles/weights
* preview custom text

---

## Specimen

Users must be able to:

* enter custom text
* adjust font size
* adjust weight
* adjust style where applicable
* adjust tracking
* adjust leading
* use existing specimen/preset functionality

Cached-font previews must work without Internet access.

---

## Download / Cache

Users can download fonts from Google Fonts.

Downloaded fonts become part of the Typecase local library.

The local copy remains available even if Google later removes the font.

---

## Installation

Default:

> Install for current Windows user.

Optional:

> Install for everyone on this PC.

System-wide installation may require UAC/elevation.

Do not run the entire application permanently elevated.

Only elevate operations that require it.

---

## Installed Fonts

The Installed view should show fonts currently installed in Windows.

This includes fonts Typecase did not install.

Distinguish:

```text
Typecase-managed
External
Unknown
```

Do not assume that every Windows-installed font belongs to Typecase.

---

## Uninstallation

Typecase-managed fonts can be uninstalled normally.

Externally installed fonts require a warning because removal may affect applications/documents.

Before removing an external font, offer the opportunity to cache/backup it.

---

## Export

Users must be able to export:

* an individual font
* an entire family

Export format:

> ZIP

The archive should contain usable TTF/OTF files and useful metadata where appropriate.

---

# 12. Supported Font Formats

For v1:

```text
.ttf
.otf
```

Do not add v1 requirements for:

```text
.ttc
.woff
.woff2
```

The internal architecture should not make future format support impossible.

---

# 13. Offline-First Behavior

Typecase must remain useful without Internet access.

Offline functionality:

* launch application
* browse local catalog
* view local library
* view installed fonts
* preview cached fonts
* install cached fonts
* uninstall fonts
* export fonts
* manage local state

Internet-dependent functionality:

* downloading uncached fonts
* refreshing remote catalog

Do not make the application appear broken merely because it is offline.

---

# 14. Google Fonts Catalog

Do not require users to configure a Google Fonts API key.

Do not make the Google Fonts Developer API a user-facing dependency.

Typecase should use a lightweight generated Google Fonts catalog.

Conceptually:

```text
Google Fonts metadata
        ↓
catalog generator
        ↓
Typecase catalog.json
        ↓
bundled with application
```

The catalog contains metadata, not the complete Google Fonts repository.

Do not bundle or clone the full Google Fonts repository into Typecase.

---

# 15. Catalog Refresh

Typecase ships with a lightweight catalog snapshot.

The user may manually refresh the catalog.

Refresh is:

* manual
* user-controlled
* non-destructive

Do not force catalog updates.

Do not continuously synchronize in the background.

If newer catalog information is available, notify the user non-intrusively.

---

# 16. Removed Google Fonts

If Google no longer lists a previously known font:

```text
Remote:
Removed

Local:
Still available
```

Do not automatically:

* delete it
* uninstall it
* delete its cache

Mark it:

> Removed from Google Fonts

The user should still be able to:

* preview it
* install it
* uninstall it
* export it

---

# 17. Local Library Model

Do not collapse the library into a single status.

A font can simultaneously be:

```text
remote = removed
cached = true
installed = true
managedByTypecase = true
```

Another can be:

```text
remote = available
cached = false
installed = true
managedByTypecase = false
```

The model should distinguish at least:

```text
Remote availability
Cached
Installed
Installation scope
Managed by Typecase
Local files
```

Useful conceptual statuses include:

```text
Available online
Cached
Installed
Installed externally
Removed from Google Fonts
Missing locally
```

These statuses are not necessarily mutually exclusive.

---

# 18. Font Identity

Filename is not sufficient to identify a font.

Use normalized font metadata plus exact file/content identity.

Relevant data includes:

* family name
* subfamily
* full name
* PostScript name
* version
* content hash

This supports:

* duplicate detection
* external-font matching
* cache validation
* uninstall tracking
* backup
* future updates

---

# 19. Windows Font Discovery

Typecase should query Windows to determine what is actually installed.

The Installed view must include fonts installed:

* manually
* by other software
* before Typecase existed
* after Typecase was installed

The Installed view represents the Windows font environment, not merely Typecase's database.

---

# 20. Font Ownership

For each discovered installed font, track whether Typecase owns/manages it.

Conceptually:

```text
Typecase-managed
External
Unknown
```

Never infer ownership merely because Typecase can see the font.

---

# 21. Windows Font Installation

Keep Windows-specific implementation behind a dedicated backend abstraction.

Conceptually:

```text
Application layer
        ↓
FontManager
        ↓
Windows implementation
        ↓
Windows APIs
```

Do not scatter Windows registry/API logic throughout React or the general application layer.

---

# 22. Persistent Installation

Do not blindly implement the original prototype recipe:

```text
copy file
→ registry
→ AddFontResourceExW
→ WM_FONTCHANGE
```

That was a hypothesis, not a final requirement.

In particular:

**Do not use `FR_PRIVATE` as the mechanism for persistent installation.**

The actual Windows implementation must be tested for:

* visibility to other applications
* persistence after Typecase exits
* persistence after reboot
* current-user installation
* system-wide installation
* uninstall
* duplicate handling

Use the correct Windows mechanisms after validation.

---

# 23. Installation Scope

Default:

```text
Current user
```

Optional:

```text
Everyone on this PC
```

System-wide installation may require UAC.

Do not run the entire application elevated.

Request elevation only for the operation requiring it.

---

# 24. Uninstallation and Cache

Uninstalling a font does not automatically delete its Typecase cache.

This is valid:

```text
Installed = false
Cached = true
```

This allows:

```text
uninstall
    ↓
retain local copy
    ↓
reinstall later without downloading
```

Deleting cached files should be a separate explicit operation.

---

# 25. External Font Safety

If Typecase did not install a font:

```text
managedByTypecase = false
```

If the user requests removal:

1. Warn that the font was externally installed.
2. Explain that removal may affect applications/documents.
3. Offer caching/export first.
4. Require explicit confirmation.

---

# 26. Export / Backup

Export must work offline.

A cached font must be preservable independently of Google's current availability.

This is particularly important for removed fonts.

---

# 27. Tauri / Rust Architecture

Use Tauri 2.

Use Rust for:

* catalog management
* network/download operations
* local library management
* font parsing/validation
* Windows font discovery
* Windows installation
* Windows uninstallation
* export/backup
* privileged operations

React should remain responsible for:

* UI
* interaction
* visual state
* specimen presentation
* navigation

---

# 28. Suggested Rust Organization

A reasonable structure is:

```text
src-tauri/
├── src/
│   ├── lib.rs
│   │
│   ├── commands/
│   │   ├── catalog.rs
│   │   ├── library.rs
│   │   ├── fonts.rs
│   │   ├── downloads.rs
│   │   └── exports.rs
│   │
│   ├── catalog/
│   ├── library/
│   ├── downloads/
│   ├── fonts/
│   │   ├── parser.rs
│   │   └── windows.rs
│   └── exports/
│
├── capabilities/
└── tauri.conf.json
```

This is a structural guideline, not a requirement to create unnecessary modules.

Prefer a small, understandable codebase.

---

# 29. Frontend ↔ Rust API

Use intent-level commands.

Conceptually:

```text
get_catalog()
refresh_catalog()

get_library()
get_installed_fonts()
get_font_details(id)

download_font(id)
cancel_download(id)

install_font(id, scope)
uninstall_font(id)

cache_external_font(id)

export_font(id, destination)
export_family(id, destination)
```

Names may change if a better implementation convention is justified.

The principle is fixed:

> React requests Typecase operations. It must not request arbitrary system operations.

---

# 30. Security

Do not expose generic privileged commands such as:

```text
execute_shell()
execute_powershell()
write_registry()
delete_file()
copy_file()
```

to the React frontend.

Expose high-level operations instead.

Do not turn Typecase into a generic arbitrary file downloader.

Network operations should be constrained to known application functionality.

---

# 31. Filesystem Safety

Downloads should use temporary files until validation succeeds.

Do not leave incomplete downloads appearing to be valid cache entries.

Archive extraction must protect against path traversal.

Never allow frontend-supplied paths to become unrestricted filesystem operations.

---

# 32. Local Storage

A reasonable initial structure:

```text
%LOCALAPPDATA%\Typecase\
├── catalog\
│   └── catalog.json
│
├── fonts\
│   └── <family-id>\
│       ├── metadata.json
│       └── *.ttf / *.otf
│
└── state\
    └── local state
```

The exact storage technology is intentionally not predetermined.

Do not introduce SQLite without a concrete need.

Choose the simplest reliable persistence mechanism that supports:

* persistence
* querying
* library state
* corruption handling
* future extension

---

# 33. Specimen Architecture

The specimen engine must consume normalized Typecase font resources.

Avoid source-specific logic such as:

```text
if Google:
    ...
else:
    ...
```

inside the specimen component.

The intended architecture is:

```text
Font source
     ↓
Normalized Typecase resource
     ↓
Specimen renderer
```

---

# 34. Performance

There is no 6–12 MB application-size requirement.

That was an old prototype assumption.

Priorities:

1. responsive UI
2. fast startup
3. fast local search/filtering
4. reliable caching
5. reliable Windows integration
6. low background resource usage
7. reasonable application size

Correctness is more important than arbitrary binary-size optimization.

---

# 35. Distribution

Provide:

## Portable

A usable portable Typecase executable/package.

## Installer

A conventional Windows installer.

Do not assume portable executable means portable data.

Initially, portable Typecase may use:

```text
%LOCALAPPDATA%\Typecase
```

for user data.

A separate portable-data mode can be considered later.

---

# 36. Telemetry

No telemetry.

Do not add:

* analytics
* usage tracking
* crash-reporting services
* advertising SDKs
* tracking endpoints

unless explicitly requested.

---

# 37. Dependency Philosophy

Prefer mature, focused dependencies.

Do not add large frameworks for trivial functionality.

Every significant dependency should solve a real problem.

Avoid dependency proliferation.

Do not replace Tauri/Rust/React/Vite/Tailwind simply because another technology looks interesting.

---

# 38. Testing

## Local/Linux testing

Can validate:

* TypeScript
* React build
* Rust compilation
* Rust unit tests
* catalog generation
* catalog parsing
* metadata normalization
* font validation
* ZIP extraction
* path traversal protection
* state transitions
* IPC structure

## Windows testing

Must eventually validate on actual Windows:

* font discovery
* per-user installation
* system-wide installation
* UAC
* uninstall
* persistence
* reboot behavior
* visibility to other applications
* external-font detection
* external-font removal warning
* offline behavior
* export
* catalog refresh
* removed-font behavior

Do not claim Windows font functionality is verified by Linux testing.

---

# 39. Development Philosophy

This is a solo-developer application supported by an AI coding agent.

Optimize for:

* understandable architecture
* maintainability
* reliability
* incremental implementation
* low operational complexity
* minimal unnecessary infrastructure

Avoid:

* enterprise architecture
* microservices
* cloud infrastructure
* unnecessary databases
* unnecessary abstraction layers
* elaborate build systems
* speculative features

The objective is a **small, maintainable desktop application**, not an enterprise platform.

---

# 40. Agent Rules

Before major architectural changes:

1. Inspect the existing implementation.
2. Determine whether existing code can be reused.
3. Preserve working UI and behavior unless there is a concrete reason to change it.
4. Prefer incremental modifications.
5. Test after meaningful milestones.
6. Do not silently introduce product features.
7. Do not silently change requirements.
8. Do not replace technologies without justification.
9. Do not treat prototype code as authoritative when this document supersedes it.
10. If an implementation detail is uncertain, investigate/test it rather than inventing certainty.

---

# 41. Do Not Let the Agent Decide

The following are fixed decisions:

* Tauri 2
* Rust backend
* React/Vite/Tailwind frontend
* Windows 10/11 target
* Google Fonts as initial provider
* TTF/OTF for v1
* local-first library
* offline cached-font preview
* per-user installation by default
* optional system-wide installation
* external font discovery
* external font removal warning
* export/backup
* no telemetry
* no end-user Google API key
* bundled lightweight catalog
* manual catalog refresh
* no automatic deletion when Google removes a font
* existing specimen UI should be preserved
* prototype catalog is not production data
* `Sheet.tsx` should become source-agnostic
* no arbitrary shell/PowerShell interface
* no generic privileged filesystem interface
* no unnecessary redesign

Do not override these decisions without explicit user instruction.

---

# 42. Do Not Overbuild

Do not implement unless explicitly requested:

* macOS
* Linux font management
* cloud synchronization
* user accounts
* subscriptions
* telemetry
* social functionality
* AI recommendations
* font editing
* font conversion
* WOFF/WOFF2/TTC support
* automatic background synchronization
* automatic font deletion
* automatic installed-font updates
* additional font providers

Design for future extensibility, but do not implement future products now.

---

# 43. Definition of Success

Typecase v1 succeeds when a user can:

```text
Launch Typecase
      ↓
Search Google Fonts
      ↓
Open a family
      ↓
Preview custom text
      ↓
Download
      ↓
Font is cached locally
      ↓
Preview works offline
      ↓
Install for current user
      ↓
Font works in Windows applications
      ↓
Restart Typecase
      ↓
Font remains installed
      ↓
Reboot Windows
      ↓
Font remains installed
      ↓
Uninstall
      ↓
Cached copy remains
      ↓
Reinstall from cache
      ↓
Export backup
```

And:

```text
Google removes a font
      ↓
Typecase detects catalog change
      ↓
Font marked "Removed from Google Fonts"
      ↓
Local copy remains
      ↓
Installed copy remains
      ↓
User can preview/install/export it
```

And:

```text
External font exists in Windows
      ↓
Typecase discovers it
      ↓
Shows it as externally installed
      ↓
User may cache it
      ↓
User requests uninstall
      ↓
Typecase warns about external ownership
      ↓
User explicitly confirms
```

These workflows define the product more accurately than any individual implementation detail.

---

# 44. Final Principle

The application should be understood as:

> **A local-first Windows font manager whose first online provider is Google Fonts.**

It is not merely:

> "A Google Fonts downloader with a Windows installer."

That distinction is fundamental to the architecture.

Build the smallest reliable implementation that satisfies this context while preserving the strengths of the supplied frontend prototype.

---

# 45. Stage Checklist

Living progress record. Milestone numbers (M1…) are the implementation sequence;
where the sequence was adjusted from the Implementation Assessment's stage order,
the reason is noted — sequencing changes are recorded here, never made silently.

Project documents:

```text
IMPLEMENTATION_ASSESSMENT.md   pre-implementation assessment (§4)
STAGE2_PLAN.md                 backend-owned library state plan
M3_CATALOG_DESIGN.md           catalog generator design record (incl. M8 refresh design)
```

```text
M1  DONE      Tauri 2 scaffold, Discover/Library/Installed shell, normalized
              face model, bundled UI-chrome fonts, specimen room preserved.
              (Assessment stage 1; the stage-2 data-model work was folded in.)

M2  DONE      Backend-owned library state (assessment stage 3). Rust catalog
              + library store + get_catalog / get_library / get_font_details
              over IPC; demo data deleted from the frontend; dev_set_phase
              harness (debug builds only) stands in for the M4 download
              path when exercising Library/Installed states. Verified
              end-to-end in the native Tauri window: get_catalog served all
              1,946 faces over IPC and dev_set_phase drove
              online→library→installed→online across three launches, with
              each phase persisting across restarts via state/library.json
              (see STAGE2_PLAN §10).

M3  DONE      Generated Google Fonts catalog + browsing at scale
              (assessment stage 4). tools/generate-catalog.mjs merges the
              keyless Google Fonts metadata endpoint with the curated
              editorial layer (tools/curated-notes.json) into a 1,946-family
              catalog.json, embedded in the binary and served to plain-browser
              dev; Rail is windowed (fixed-row virtualization, no new deps).
              Catalog refresh / removed-font marking remains M8. Verified in
              plain-browser dev and, after the M2/M3 merge, in the Tauri dev
              window: the IPC catalog path served all 1,946 families inside
              the native webview (STAGE2_PLAN §10).

M4  DONE      Validated download pipeline (assessment stage 6). Keyless
              Google Fonts CSS2 fetch (no UA spoofing needed: non-browser
              clients receive TTF), streamed sha256 + sfnt validation,
              staging→commit into fonts/<family-id>/ with content-addressed
              filenames and a metadata.json manifest; download_font marks
              the face cached and persists. dev_set_phase, dev_e2e_report
              and the temporary frontend harnesses are retired — the
              network integration test (src-tauri/tests/
              download_pipeline.rs, run with --ignored) is the repeatable
              verification vehicle. Verified live in the Tauri dev window:
              real download → library.json cached=true → restart
              persistence (see STAGE2_PLAN §11).

M5  DONE      Source-agnostic offline preview (assessment stage 5,
              resequenced after M4 because offline preview requires files
              on disk). Backend serves fonts/<family-id>/ over the custom
              font:// scheme (strict §31 path validation: slug-charset ids,
              sha8.ttf|otf filenames, canonicalized containment);
              get_font_sources returns backend-built URLs from the
              manifests; the frontend registers them as FontFace objects
              with real weight/style (specimen untouched — §33), falling
              back to the remote stylesheet when local registration fails.
              CSP gains font: and http://font.localhost. Verified live in
              the Tauri window: cached ABeeZee served from disk online,
              and again with the app's network dead via proxy env —
              offline preview works (§13). Design record:
              M5_OFFLINE_DESIGN.md.

M6  DONE*     Windows install/uninstall behind FontManager (assessment
              stage 7), per M6_FONTMANAGER_DESIGN.md. §21 abstraction with
              pure tested helpers (style/value naming, sanitized filenames,
              install records + plans); Windows impl: per-user 1809+
              mechanism (HKCU value + %LOCALAPPDATA%\Microsoft\Windows\Fonts
              + AddFontResourceW + WM_FONTCHANGE — NOT FR_PRIVATE, §22);
              system scope via one-shot self-elevation (ShellExecuteExW
              "runas" on the app's own binary with an elevation-only
              subcommand; the app never runs elevated, §23);
              state/installed.json records exactly what was written and
              uninstall reverses only that (§20). install_font/uninstall_font
              commands + Library-view UI (install user/system, uninstall,
              each behind explicit confirmation, EN+ES). Linux: 40 unit
              tests incl. 9 new FontManager tests. *The Windows runtime
              behavior is compile-validated in CI only; it is DONE when the
              WINDOWS_VALIDATION.md §5 VM session passes (Notepad/Word
              visibility, registry captures, reboot persistence, §38).

M7  DONE*     External font discovery, ownership, removal warning
              (assessment stage 8), per M7_EXTERNAL_FONTS_DESIGN.md.
              Registry enumeration (HKCU + HKLM fonts keys, tolerant of
              malformed values) feeding get_installed_fonts with
              record-based ownership classification (managed/external,
              §20 — never inferred from visibility); Installed tab is now
              the real Windows font environment with ownership badges;
              §25 removal flow for external fonts (warning dialog naming
              cross-application dependence, explicit confirm, registry
              value + file deletion, system scope via the M6 one-shot
              elevation helper, managed-font refusal guard on the external
              path). Pure parsing/classification logic unit-tested (44
              tests). *Runtime registry behavior is compile-validated in
              CI only until the WINDOWS_VALIDATION §6 items run on a VM.

M8  DONE*     Export/backup, catalog refresh, removed-from-source marking
              (assessment stage 9), per M8_EXPORT_REFRESH_DESIGN.md.
              exports/: dependency-free ZIP writer (stored entries; CRC-32
              checked against test vectors) + README manifest; export_font
              uses cached files first, installed files as fallback (export
              survives a §24 cache deletion on installed families); the
              destination is chosen by the backend — exports/ under the app
              data dir (§31), staged atomic write, frontend never supplies
              paths. catalog/refresh.rs: Rust port of the M3 generator
              (slug parity asserted against all 1,946 embedded records;
              combining-mark stripping and code-point sort were the subtle
              bits); refresh_catalog diffs the live endpoint against the
              active catalog and PARKS the result (§15 — nothing
              auto-applies); apply_catalog_refresh is the only write path,
              atomically to catalog/catalog.json, loaded disk-over-embedded
              with corrupt-file fallback. FontRecord.removedFromSource:
              set for families the source drops (never deleted — §16),
              cleared when they return; Sheet banner, Rail ✝ marker and a
              summary count, all EN+ES. State: TypecaseState.catalog →
              RwLock (apply is the single writer; atomic swap). 69 unit
              tests + tests/export_backup.rs + a live-endpoint network
              test (nightly CI). Verified live in the Tauri dev window
              via a one-shot check (removed after the run): real export
              (19-entry ZIP of Inter), real refresh (+0 ~1939 −0 popularity
              drift vs the M3 snapshot), apply, and a 1,946-family reload
              from the disk catalog. *Windows-end-to-end UI flows compile-
              validate in CI; §6.3–6.5 run in the VM session.

M9  DONE*     Distribution (assessment stage 10): portable + installer +
              tag-driven GitHub releases (M9_DISTRIBUTION_DESIGN.md).
              Build body extracted to .github/workflows/build-windows.yml
              (workflow_call) so the push/nightly gate (windows.yml, triggers
              unchanged) and release builds (release.yml, tags v*, network
              tier always on) cannot drift. New artifact
              typecase-windows-portable: the raw release exe — Tauri 2 has
              no portable bundle target on Windows; WebView2 Evergreen covers
              10 1809+/11; shares the installed build's per-user data root by
              design. Release job publishes NSIS+MSI+portable+checklist as
              permanent assets (-rc tags → prerelease). No code signing
              (§7.2 records unsigned) and no auto-updater (§42) by decision.
              §43 acceptance mapping added as WINDOWS_VALIDATION §8. Flow-3's
              "cache external font" deviation was then closed in code: §29
              cache_external_font exists (M7_EXTERNAL_FONTS_DESIGN.md §6) —
              ext-<slug> ids in the ordinary fonts tree, manifest source
              "external-windows", M4 validation rules, copies deletable via
              delete_cached_family — and the removal dialog now offers a real
              copy before the warning (§25 step 3). That is app code landing
              after the RC, so the v0.1.0-rc.1 binaries predate it and the
              §7/§8 VM sweep must run on a later RC.
              release.yml is now exercised: tag v0.1.0-rc.1
              (commit 6dcd87d) published release 398720167 with
              prerelease:true and all four assets (NSIS 3.65 MB, MSI
              5.00 MB, portable 13.30 MB raw exe, checklist 12 KB) — run
              36501073516 green, full network tier included (see
              M9_DISTRIBUTION_DESIGN.md §7; two release-job fixes: gh
              needs --repo without a checkout, and artifacts must be
              flattened to files before upload). DONE* pending only the
              §7/§8 VM test pass gating v0.1.0.
```

Windows-only validation (assessment stages 7–8, §38) is performed at M6 and
later on real Windows 10/11 and is never claimed from Linux testing.

Addenda (post-M5 product work):

```text
A1  DONE      Spanish UI + dark mode (user request). src/i18n.ts: typed
              EN/ES dictionaries covering all chrome and the editorial
              preset copy (no i18n dependency; §37); src/theme.ts: persisted
              dark preference, system-scheme default, .dark class flipping
              semantic palette variables. All components themed via Tailwind
              tokens (bg-paper/text-ink/…) bound to CSS variables — zero
              hardcoded hex classes remain; contrast pairs re-tuned per
              scheme. Toggles in the masthead (theme ☀/☾, language EN|ES);
              both persist across restarts. Verified in the browser: full
              ES rendering incl. presets, both schemes, persistence.
              Native chrome follows the theme: apply_window_theme (runtime
              set_theme) is invoked at startup and on every toggle; both
              toggles verified inside the Tauri window via a one-shot
              in-webview check (startup sync, dark flip, lang flip, html.lang
              all evidenced in the dev log). Per §38, the Windows title-bar
              UxTheme effect itself is confirmed only on real Windows.
              Live OS follow: with no stored preference the app tracks
              prefers-color-scheme changes at runtime (listener re-reads the
              preference at event time; an explicit toggle always wins).
              Verified in the browser via scheme emulation: guard with a
              stored pref, live follow without one, and mid-session
              preference stick.

A2  DONE      i18n conventions pass + RTL smoke check (user request).
              Dictionary audited against RAE conventions — already correct:
              «angular quotes», comma decimals (10,5 pt), space-grouped
              thousands (14 000 / 1 284), spaced percent (125 %). Gaps
              closed: locale-aware formatting for COMPUTED values (new
              i18n num() — tracking/leading now render -0,010em / 1,05 in
              es instead of toFixed periods) and <html lang> now follows
              the UI language (spellcheck/hyphenation/AT). RTL smoke:
              dir="auto" on the specimen line + waterfall rows — Arabic
              input renders right-to-left with proper bidi (embedded
              digits at visual left), flips back live for Latin; verified
              computed direction both ways in the browser.

A3  DONE      Windows validation path (user request). Tier 1 automated:
              .github/workflows/windows.yml — windows-latest runner runs
              npm ci → tsc → vite build → cargo test --locked → tauri
              build (NSIS + MSI) with artifacts uploaded. Nightly
              schedule (03:23 UTC) adds the network integration tests
              (real Google Fonts endpoint from a Windows IP); also on
              demand via a workflow_dispatch input, or on every push
              via RUN_NETWORK_TESTS. Tier 2 manual:
              WINDOWS_VALIDATION.md turns §38 into an evidence-based VM
              session script (install/UAC, UxTheme title bar, ES + RTL on
              WebView2, offline preview via real Flight mode, per-user
              install + no-FR_PRIVATE registry proof, reboot persistence,
              external fonts, removed fonts, RC sweep) — items gate their
              milestones (5.x [M6+], 6.x [M7/M8]) and none may be claimed
              done from Linux.
```

