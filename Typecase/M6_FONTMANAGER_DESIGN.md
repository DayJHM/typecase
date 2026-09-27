# M6 Design — FontManager: Windows Install/Uninstall (§21 abstraction)

**Status.** Implemented behind the `fontmanager` module. The Windows
implementation is **compile-validated on Windows** by CI (windows-latest runs
`cargo test` on every push/nightly); its runtime behavior is validated **only**
by the WINDOWS_VALIDATION.md §5 VM session — never claimed from Linux (§38).
Non-Windows builds compile with an unsupported manager whose commands return
honest errors.

## 1. Mechanism (modern per-user, NOT FR_PRIVATE)

Windows 10 1809+ supports true per-user fonts, which is what §22/§23 require:

```text
install (scope User):
  copy file   → %LOCALAPPDATA%\Microsoft\Windows\Fonts\<name>.ttf
  registry    → HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts
                value name: "<Family> (TrueType)" = <full path>
  session     → AddFontResourceW(<path>)   (font usable immediately)
  broadcast   → WM_FONTCHANGE               (running apps notice)
```

- `AddFontResourceW` loads the font for this logon session; the registry value
  is what makes it persist across reboots (Windows re-registers per-user fonts
  at logon). This is the documented 1809+ pairing — **not** the prototype's
  `FR_PRIVATE` hypothesis (§22 forbids it).
- Uninstall reverses the exact three writes: `RemoveFontResourceW`, delete the
  registry value **we wrote** (matched by exact value name from the install
  record), delete the file we copied.

**System scope (Everyone)**: HKLM + `C:\Windows\Fonts` require elevation. The
app must never run elevated (§23), so elevation is scoped to one operation:
a small elevated helper process re-execs Typecase itself with an
elevation-only subcommand that performs the HKLM/file writes and exits
(`--elevated-install-<kinds> --family <fam> --files <paths>` parsed in
`main.rs`, gated `#[cfg(windows)]`). The interactive app stays unelevated.

## 2. Install records (the uninstall contract)

`state/installed.json` records exactly what Typecase wrote per family:

```text
{ id, family, scope: "user"|"system", registeredAt,
  entries: [{ file (installed path), valueName (registry), source (cache path) }] }
```

Uninstall never guesses: it reverses entries from the record (§20 — external
fonts are not touched; the M7 warning flow builds on this). Absent record ⇒
"not installed by Typecase" error, never a heuristic registry scan.

## 3. Source of truth for family/file selection

Files come from the M4 cache manifest (`fonts/<id>/metadata.json`): one entry
per (weight, style). The installed filename is `<family> <style>.ttf`
(style suffix only when not the first regular weight; sanitized: family names
are data, not paths). Display name for the registry value:
`"<Family> <StyleName> (TrueType)"` — StyleName = Regular/Bold/Italic/Bold
Italic/<weight> style, derived from (weight, style) the same way the CSS2
pipeline tags them.

## 4. Abstraction (§21)

```rust
pub trait FontManager { install, uninstall, is_installed }
struct WindowsFontManager;  // cfg(windows): HKCU/HKLM + font APIs + helper
struct UnsupportedFontManager; // cfg(not(windows)): honest Err
commands::install_font(id, scope) / uninstall_font(id)  // §29 intent level
```

State write order on install: files+registry first (Windows), then
`installed.json`, then `library.json` (`installed=true`, scope, managed=true).
A crash between steps leaves at worst an orphaned copy — the record is the
repair source; `install` is idempotent per scope (re-install over an existing
record replaces entries cleanly).

## 5. What is deliberately NOT in M6

- External-font discovery/ownership (M7); the uninstall warning flow lands
  there too — M6 only ever uninstalls fonts Typecase installed (records only).
- Per-file install granularity: scope is the family (M4 cache is per-family).
- Linux/macOS support (§42): the unsupported stub exists so the UI and tests
  compile everywhere, returning `unsupported on this platform`.

## 6. Verification boundary (§38)

- Linux: pure helpers unit-tested (style/value-name derivation, record
  round-trips, idempotency plans, filename sanitization); commands compile;
  the UI flow is exercised against the unsupported manager's honest errors.
- Windows CI: everything compiles and unit tests pass on windows-latest —
  including the `windows`-crate code paths (compile-level validation).
- Real Windows (VM): WINDOWS_VALIDATION.md §5 is the acceptance script —
  Notepad/Word visibility, HKCU/HKLM reg captures, reboot persistence,
  WM_FONTCHANGE behavior, duplicate handling. Nothing here is "done" until
  those captures exist.
