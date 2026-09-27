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

## 5. What M7 deliberately does NOT do

- Content-hash matching for `unknown`/dedup (§18 machinery exists; the UI and
  matching policy are a later milestone).
- Importing external fonts into the Typecase cache (`cache_external_font`,
  §25 step 3) — the dialog links the existing Export story instead; import
  lands with M8's export/backup work.
- Non-registry font sources (per-machine GDI Enumeration of
  not-registered fonts), WOFF/TTC (§12).
