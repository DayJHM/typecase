# M7 Design — External Font Discovery, Ownership, Removal Warning (§19–20, §25)

**Status.** Implemented. Compile-validated on Windows CI; the runtime registry
enumeration is runtime-validated only by the WINDOWS_VALIDATION.md §6 VM
session (§38 discipline as in M6).

## 1. Discovery (§19: the Installed view = the Windows font environment)

On Windows the registry is the machine's font index. One enumeration per
scope (HKCU and HKLM under `Software\Microsoft\Windows NT\CurrentVersion\Fonts`)
returns every registered font:

```text
ExternalFont { valueName, family, style, filePath, scope }
```

- `valueName` is the raw registry name (`"Inter Bold (TrueType)"`).
- `family`/`style` are parsed from it: strip the trailing ` (TrueType)` /
  `(OpenType)` marker, split a trailing style token (Bold/Italic/Bold
  Italic/`<digits>`[ Italic]) off the family. Unparseable names keep the
  whole string as family with style `"Regular"`.
- `filePath` is the registry data (REG_SZ path) when readable.
- Enumeration is tolerant: a malformed value is skipped, never fatal; the
  command never fails the UI because one entry was odd.

## 2. Ownership classification (§20: never infer from visibility)

Three-way classification, decided per family:

```text
managed    — Typecase wrote it: an install record exists (state/installed.json)
external   — registered in Windows, no Typecase install record
unknown    — (future: matched by content hash; reserved in the type)
```

The classification is **record-based, not heuristic**: seeing a font in the
registry never makes it Typecase's (§20). `unknown` exists in the payload type
now (§17 anticipates content-hash matching in a later milestone) but is not
produced yet.

## 3. Payload and views

`get_installed_fonts` (§29) returns every registered font with its ownership:

```text
{ valueName, family, style, filePath, scope: "user"|"system",
  ownership: "managed"|"external"|"unknown", id? }
```

`id` is set for managed fonts (their Typecase face id) so the UI can navigate.
The Library view's Installed tab lists the union: registered fonts from
Windows, Typecase-managed or external, each badged. This changes M2's
placeholder semantics (Installed = library filter) into the real Windows view.

## 4. Removal flow (§25: warn, offer cache, explicit confirm)

Uninstall of an **external** font (M7 scope: fonts registered in Windows with
no Typecase record — the file is typically outside our cache) requires:

1. A warning dialog naming the family and stating applications/documents may
   depend on it.
2. An explicit checkbox/confirm step — no batch path.
3. Registry value deletion + file deletion (recycling the exact value name;
   the file path comes from the registry entry itself).
4. For managed fonts the M6 record-based uninstall remains the path; the §24
   cache-preservation rule is unchanged.

System-scope removal elevates through the same one-shot helper as M6
(`--typecase-elevated-uninstall-external <valueName> <path>`).

§25 step 3 (offer caching/export first) is now **implemented**: the dialog
offers **Keep a copy** before the confirmation is given, backed by
`cache_external_font` — see §6. It replaced the earlier "link the Export
story" stand-in, which was never actionable (export needs a catalog family,
and an external font has no catalog id), so `extCacheOffer` used to admit
that no copy could be made.

## 5. What M7 deliberately does NOT do

- Content-hash matching for `unknown`/dedup (§18 machinery exists; the UI and
  matching policy are a later milestone).
- Non-registry font sources (per-machine GDI Enumeration of
  not-registered fonts), WOFF/TTC (§12).

## 6. External font caching (`cache_external_font`) — landed after M9's RC

**Why.** §25 requires the removal flow to offer a copy first and §43 flow 3
ends with "user may cache it"; without it, removing an external font simply
deletes the only copy on the machine. Recorded as the one open deviation in
M9_DISTRIBUTION_DESIGN.md §6 before this landed.

**Decisions.**

- **Storage reuses the ordinary cache tree**: `fonts/ext-<slug>/<sha8>.<ttf|otf>`
  plus a `metadata.json` that keeps M4's core fields (`id`, `family`, `files[]`
  with a numeric weight and the CSS-valid `normal`/`italic`) and adds
  provenance (`source: "external-windows"`, per-file `valueName`,
  `registryStyle`). Reusing the layout means the §31 path rules, the
  content-addressed filename shape and the M5 `font://` serving path all apply
  unchanged, so a preserved copy is usable rather than a dead file. §28: no
  second storage subsystem.
- **Identity**: `ext-<family-slug>` in the §31 slug charset, so an external
  copy can never be confused with a provider family id; the command refuses if
  the derived id would collide with a catalog family. The *manifest source tag*,
  not the prefix, is what makes an entry external.
- **Only registered fonts are cacheable** (§30/§31): the caller passes the
  value name + path + scope it saw in the Installed view, and the backend
  verifies that exact triple against live discovery
  (`externalfonts::resolve_registered`) before reading anything. Windows paths
  compare case-folded; the value name and scope must match exactly.
- **Same validation as a download**: `downloads::validate_sfnt_bytes` (size
  bounds + sfnt magic) and `sha256_hex`, staged through `staging/` and renamed
  into place, so an interrupted copy never looks like a valid entry.
- **Idempotent and family-aware**: re-caching unchanged bytes rewrites nothing;
  caching a second style merges into the same family manifest; re-caching a
  font the user replaced in Windows updates that registry entry's file and
  deletes the superseded one instead of accumulating orphans.
- **Deletion** stays explicit (§24): `delete_cached_family` accepts an
  external-copy id (it has no library state to check) and the Installed row
  offers **Discard copy** behind a confirmation that says the Windows font is
  untouched.

**Verification.** Logic-level only, on Linux: `cargo test --locked` 81 passed
(+12: id charset, style inversion against `fontmanager::style_name`, manifest
write/merge/replacement, rejection of non-fonts/too-small/managed/missing
files, inventory, triple verification, corrupt-manifest handling) plus
`npm run typecheck` and `npm run build`. Windows registry behaviour is
**not** claimed from Linux (§38) — it needs the WINDOWS_VALIDATION §6.7 item
in a VM session, which must run on a build at or after this change (the
`v0.1.0-rc.1` binaries predate it).
