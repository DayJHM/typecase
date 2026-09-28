# Windows Validation — §38 Checklist (Manual, VM or Physical Machine)

**Purpose.** CONTEXT.md §38 draws a hard line: Linux testing can verify logic,
Windows-only behavior must be validated on real Windows and never claimed from
Linux. This checklist turns §38 into a concrete, evidence-based session. Every
item ends with something you can see, capture, or file.

**Scope note.** Items marked **[M6+]** gate functionality that does not exist
yet (font install/uninstall, external-font discovery). Run those sections when
their milestone lands; everything else is validatable against the current
build (through M5 + i18n/dark mode).

**Setup.** Windows 10 (1809+) or Windows 11 VM or machine. Install the
`typecase-windows-installers` artifact from CI (NSIS `.exe` or MSI), or build
locally: `npm run tauri build -- --bundles nsis,msi` inside `Typecase/`.
Create a screenshot folder `%USERPROFILE%\Desktop\typecase-evidence\` and
number captures to match the item IDs below.

Legend: **[automated]** already exercised by CI · **[interactive]** needs
human eyes/hands · **[M6+]** needs the install milestone.

---

## 1. Install & launch

- [ ] **1.1** NSIS installer runs to completion; per-user install needs **no
  UAC prompt** (CONTEXT §23: current-user by default). MSI installs silently:
  `msiexec /i typecase.msi /qn` from an ordinary shell.
- [ ] **1.2** App launches from the Start menu shortcut. **[automated by CI
  build; interactive first run]**
- [ ] **1.3** Data root exists at `%LOCALAPPDATA%\Typecase\` with
  `state\library.json` created on first write — never anywhere else (§32,
  STAGE2_PLAN §4). Capture: `dir /s /b %LOCALAPPDATA%\Typecase`.
- [ ] **1.4** Uninstall removes the app but **preserves** `%LOCALAPPDATA%\Typecase\`
  (user data outlives the app; decide deliberately whether that changes later).

## 2. UxTheme title bar (the A1 addendum feature)

- [ ] **2.1** Launch → toggle ☾ **DARK**: the native **title bar** switches to
  dark chrome in sync with the specimen area. Capture before/after.
- [ ] **2.2** Toggle ☀ **LIGHT**: title bar returns to light chrome. Capture.
- [ ] **2.3** Quit, set Windows Settings → Personalization → Colors to *Dark*,
  delete `HKCU\Software\Typecase` (or clear localStorage via devtools), relaunch:
  the app **and** title bar come up dark (system-scheme default).
- [ ] **2.4** With a stored preference (toggle once, quit), change the Windows
  color mode: the app stays on its stored theme (preference wins over system).
- [ ] **2.6** With **no** stored preference (clear localStorage), change the
  Windows color mode **while the app runs**: the app — and title bar — follow
  the scheme live, no restart (prefers-color-scheme listener). Then toggle
  once and change the OS mode again: the app now stays on its stored theme.
  Capture both.
- [ ] **2.5** High-contrast mode (Settings → Accessibility): the app remains
  legible; no unreadable text-on-text. Record any breakage with a capture.

## 3. Language toggle (ES)

- [ ] **3.1** EN→ES flips every visible string; specimen presets render the
  Spanish editorial copy; tracking/leading show comma decimals (`-0,010em`,
  `1,05`). Capture the composing stick in ES.
- [ ] **3.2** Restart: the language and theme choices both persist (localStorage
  under the WebView2 profile; confirm where it lands for the installer build —
  `%LOCALAPPDATA%\dev.typecase.app\` in dev, the packaged equivalent in prod).
- [ ] **3.3** With ES active, paste Arabic (`نصٌّ عربيٌّ 123`) into the specimen
  line: RTL shaping, caret right, digits at visual left (A2 smoke, now on
  WebView2 — the rendering engine actually shipped to Windows users).

## 4. Catalog, download, offline preview (M4/M5 behavior)

- [ ] **4.1** Catalog loads (Discover count = 1946). Search "grotesk" filters
  instantly.
- [ ] **4.2** Cache any family: success shows "Cached ✓ N files"; the face
  moves to Library; `fonts\<id>\` appears with `<sha8>.ttf` files + manifest.
  Capture `dir %LOCALAPPDATA%\Typecase\fonts\<id>`.
- [ ] **4.3** **Offline preview**: enable *Flight mode* (real network cut, not
  a dev hack), relaunch the app, open the cached family → the specimen sets
  from local files ("◧ set from local file"). Uncached faces show the offline
  state without the app appearing broken (§13). Capture.
- [ ] **4.4** Disable Flight mode; delete the cached family via the Library
  view; confirm the dialog copy, then verify `fonts\<id>\` is gone and
  `library.json` shows `cached: false`.
- [ ] **4.5** Hand-corrupt `state\library.json` (type junk into it), relaunch:
  the app starts from defaults and logs the corruption warning — never
  fabricated cached/installed entries. Restore the file.

## 5. Windows font behavior [M6+] — the §38 core

These gate the FontManager milestone; none may be claimed done from Linux.

- [ ] **5.1 [M6+] Per-user install**: install a cached family for the current
  user. Verify **without Typecase**: fonts are visible in `explorer
  shell:fonts`, and a *fresh* Notepad/M Word instance lists and renders the
  family (§22: visibility to other applications).
- [ ] **5.2 [M6+] No FR_PRIVATE**: files land in `%LOCALAPPDATA%\Microsoft\Windows\Fonts\`
  with the per-user registry entry under
  `HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts` — not a private
  process-scoped load. Capture reg query output.
- [ ] **5.3 [M6+] Persistence after Typecase exits**: close Typecase; the font
  still renders in other apps.
- [ ] **5.4 [M6+] Persistence after reboot**: restart Windows; the family
  survives (logon session re-registers per-user fonts). Capture the font list
  before/after restart.
- [ ] **5.5 [M6+] System-wide install (optional scope)**: "Everyone on this
  PC" triggers exactly one UAC elevation for the operation (never a permanently
  elevated app, §23); files land in `C:\Windows\Fonts` with an HKLM entry.
- [ ] **5.6 [M6+] Uninstall**: Typecase-managed font uninstalls cleanly;
  cached copy remains (§24); other apps lose the family only after restart.
- [ ] **5.7 [M6+] Duplicate handling**: install a family that already exists
  (same family, different version) — no ghost entries; behavior documented.
- [ ] **5.8 [M6+] WM_FONTCHANGE broadcast**: running apps notice the new font
  without restart (validate; if unreliable on modern Windows, document the
  actual behavior).

## 6. External fonts & removed fonts [M7/M8]

- [ ] **6.1 [M7]** Pre-install a font manually (drag into `shell:fonts`).
  Typecase's Installed view lists it as **external** — badged
  "External", never claimed as its own (§19–20). Capture the row.
- [ ] **6.2 [M7]** External-font removal flow warns (dialog states
  cross-application dependence), requires explicit confirmation, then
  deletes the registry value and its file. Verify the entry is gone from
  `shell:fonts` after other apps restart. Capture the dialog.
- [ ] **6.2b [M7]** A Typecase-managed family shows the "Typecase" badge and
  no Remove button in the Installed view (the managed uninstall lives on
  its specimen row); attempting the external-removal IPC for it is refused
  by the backend.
- [ ] **6.3 [M8]** Simulate removal-from-source: with a family cached, refresh
  the catalog against a payload missing that family → face marked "Removed
  from Google Fonts", cache and install untouched, preview/install/export
  still work (§16).
- [ ] **6.4 [M8]** Export: open a cached family → **⇩ Export ZIP** →
  `%LOCALAPPDATA%\Typecase\exports\<id>-typecase-export.zip` contains the
  TTFs plus a README manifest; open the archive in Explorer and extract one
  font (double-click → font viewer renders it). Capture the folder listing.
- [ ] **6.5 [M8]** Export after §24: delete a family's cache while it stays
  installed → export still succeeds (falls back to the installed copy) and
  the README/outcome says "installed copy".
- [ ] **6.6 [M8]** Catalog refresh (real network): **⟳ Check for catalog
  updates** → an update notice lists N/M/R; **Not now** dismisses without
  applying; refresh again → **Update catalog** → counts move and the app
  serves the refreshed catalog. Offline: the button reports failure
  honestly and the app stays fully usable (§13).

## 7. Release-candidate sweep (per version)

- [ ] **7.1** Both installers (NSIS + MSI) from the CI artifact install and
  launch on a **clean** VM snapshot (no dev tooling installed).
- [ ] **7.2** `Get-Item ...\typecase.exe | Get-AuthenticodeSignature` — signing
  status recorded (unsigned for now; note it).
- [ ] **7.3** Windows Defender/SmartScreen reaction to the unsigned binary is
  recorded (expected: Mark-of-the-Web warning; document the wording).
- [ ] **7.4** No telemetry: first-launch outbound connections are only
  fonts.googleapis.com/gstatic (observe in Resource Monitor → Network). §36.
- [ ] **7.5** File the session: evidence folder zipped next to the release,
  checklist ticked, deviations logged in CONTEXT.md §45.

---

**Session log template** (copy into the PR/release notes):

```text
Windows validation <date> — <Windows edition/version> — build <artifact id>
Items passed: 1.1–1.4, 2.1–2.5, 3.1–3.3, 4.1–4.5, 7.1–7.5
Items deferred (feature not yet implemented): 5.x [M6], 6.x [M7/M8]
Deviations: <none or list>
Evidence: typecase-evidence-<date>.zip
```
