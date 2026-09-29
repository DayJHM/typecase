# M9 — Distribution Design (assessment stage 10)

**Goal.** "Portable build + installer via Tauri bundler; Windows test pass for the
§43 success workflows." The installer half has existed since the M5-era CI
(NSIS + MSI on every push and the nightly); M9 completes the distribution
story: a portable artifact, tag-driven GitHub releases, and the §43
acceptance mapping that turns the VM session into the recorded test pass.

## 1. What already exists (not rebuilt)

- `windows.yml` builds NSIS + MSI on every push/PR/nightly and uploads
  `typecase-windows-installers` (+ the validation checklist artifact).
- Per-user install default (NSIS `installMode: currentUser` is Tauri's
  default; §23/§41 fixed decisions), silent MSI path (§1.1 checklist).
- WINDOWS_VALIDATION.md §1/§7 cover install, uninstall, SmartScreen,
  signature status (unsigned for now).

## 2. Portable build

Tauri 2 has no dedicated "portable" bundle target on Windows; the portable
artifact **is the raw release exe** (`target/release/<name>.exe`). Decisions:

- The portable exe is the **same binary** as the installed one: identical
  behavior, same per-user data root (§1.3). It is *not* a USB-stick build
  that keeps data beside the exe — that would need a launcher shim or env
  override and was not requested (§42; record as a possible addendum).
- WebView2 is required and ships in Windows 10 1809+/11 via Evergreen
  runtime, so the single exe runs unmodified on supported targets.
- CI stages it as `Typecase-portable.exe` (deterministic pick:
  `target/release/typecase.exe`, with a largest-root-exe fallback that
  excludes uninstaller-style names) and uploads artifact
  `typecase-windows-portable`. Version lives in the release/tag, not the
  filename, so the staging step never needs a version bump.

## 3. Workflow topology (drift-proofing the build)

The build steps move into a **reusable workflow**; duplication across
windows-ci and release builds would invite drift in the subtle parts
(working-directory defaults, cache paths, network-tier gating):

```text
.github/workflows/build-windows.yml   on: workflow_call (input: network_tests)
                                      the whole current build job + portable staging
.github/workflows/windows.yml         on: push/PR/dispatch/schedule (unchanged triggers)
                                      thin caller; computes network_tests
                                      (schedule or dispatch input) and forwards it
.github/workflows/release.yml         on: push tags v*
                                      job build → job release (ubuntu, contents:write)
                                      downloads same-run artifacts, creates the release
```

- Caller semantics preserved: same workflow name (`windows-ci`), same job id
  (`windows`), same triggers. The network-tier condition moves into the
  callee as `inputs.network_tests == true || vars.RUN_NETWORK_TESTS == 'true'`
  (a callee always sees `event_name == workflow_call`, so scheduled-ness must
  be forwarded by the caller).
- Release job: `gh run download $GITHUB_RUN_ID` (no extra action, no
  upload/download-artifact version pairing to track), then
  `gh release create` with `permissions: contents: write`. Tags containing
  `-rc` publish as prereleases. Release assets are permanent — they outlive
  the 90-day artifact retention, which is where the RC evidence lives.
- Release builds run the **full network tier** (`network_tests: true`):
  tag builds are rare, so the complete gate is worth the minutes.

## 4. Versioning and the RC gate

- Version 0.1.0 stays in `tauri.conf.json` / `Cargo.toml` / `package.json`
  (bump all three together when cutting a final). RC-ness lives in the tag
  (`v0.1.0-rc.1`) + the release's prerelease flag, so no semver ripple.
- Deliberate gate: **no `v*` tag is cut until the VM session passes §7**
  (clean-VM install sweep, SmartScreen wording, no-telemetry observation).
  `v0.1.0-rc.1` is the natural RC build for that session; `v0.1.0` follows
  the recorded pass.

## 5. Out of scope (recorded, not built)

- Code signing (assessment: "signing status recorded; unsigned for now" —
  checklist §7.2/§7.3 captures the real SmartScreen wording).
- Auto-updater (§42: not requested; Tauri updater plugin exists if ever).
- USB-stick data-local portable mode (§2 above).
- Any app code changes — M9 touches CI + docs only.

## 6. §43 acceptance mapping

The three §43 flows are executable entirely from checklist items that now
all exist (M6–M8). The mapping lives in WINDOWS_VALIDATION.md §8 so the VM
session log can cite flow → item directly; no new tooling.

Known gap, recorded honestly in the mapping: §43 flow 3's "user may cache
it" is **not implemented** (`cache_external_font` from §29 was never built;
the removal dialog says so via `extCacheOffer`). §25's export-first offer is
the validated substitute; the session log records the deviation rather than
claiming the step.

## 7. Verification record

- [x] Reusable-workflow refactor: windows-ci green on the M9 push — run
      36461564474 (commit aba8830): full step list passed, including the
      network-tier skip on an ordinary push (gate semantics preserved).
- [x] `typecase-windows-portable` produced alongside installers: 4.86 MB
      staged exe vs 8.42 MB installers artifact.
- [x] Release workflow exercised by the first `v*` tag, `v0.1.0-rc.1`
      (commit 6dcd87d, annotated tag object 04ede79). Green run 36501073516:
      the `build` job ran the **full** gate including the network tier on
      Windows, and the `release` job published
      https://github.com/DayJHM/typecase/releases/tag/v0.1.0-rc.1
      with `prerelease: true`, `draft: false` and all four assets —
      `Typecase_0.1.0_x64-setup.exe` (3,647,545 B),
      `Typecase_0.1.0_x64_en-US.msi` (5,001,216 B),
      `Typecase-portable.exe` (13,304,320 B raw exe) and
      `WINDOWS_VALIDATION.md` (12,013 B). Exactly one release exists for the
      tag; the `-rc` prerelease rule fired as designed.
- Two fixes were needed to get the release job green, both recorded here
      because they are load-bearing, not incidental:
  - **gh needs the repo named.** The artifact-only `release` job skips
    checkout (run 36500195139, green `build`, failed `release`):
    `gh release create` fell back to local git and died with *"fatal: not a
    git repository"*. Every `gh` call now passes `--repo "$GITHUB_REPOSITORY"`
    and `--verify-tag` (commit a8cfe85).
  - **Assets must be files, not directories.** `upload-artifact` preserves
    the matched paths' common ancestor, so the installer artifact unpacks as
    `nsis/` and `msi/` subdirectories; the one-level glob matched those
    directories and gh refused (*"read …/msi: is a directory"*) (run
    36500636890). The job now flattens every artifact tree into one `assets/`
    directory before uploading (commit 6dcd87d), and deletes any prior
    release for the tag first so re-runs and re-pushed tags are idempotent.
- Note for the record: `--generate-notes` produces only the Full Changelog
      link, because this repository is push-based — GitHub's notes are built
      from merged PRs, and there are none. A hand-written note body would
      read better for a tester landing on the release page.
- [ ] The §7 VM pass itself (clean Windows VM, install/uninstall, SmartScreen
      wording, reboot persistence) is still outstanding — it is what gates
      `v0.1.0`, not the RC.
