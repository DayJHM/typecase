# M5 Design — Offline Preview (backend-served font files → @font-face)

**Status.** Implemented; verified live in the Tauri dev window, including an
offline simulation (see the verification record in STAGE2_PLAN §12). This is
the design record for M5: how cached files reach the specimen, and why.

**Implementation:** `serve_font_file` / `FontSource` in
`src-tauri/src/downloads/mod.rs`, `get_font_sources` + scheme registration in
`commands/mod.rs` / `lib.rs`, frontend local-face registration in
`src/data/ipc.ts` + `src/lib/fonts.ts`.

---

## 1. The problem

`fonts/<family-id>/` now holds validated TTF files (M4), but the specimen
still previews every face through the Google CSS2 stylesheet. Cached faces
must render **without any network** (CONTEXT §11, §13): the backend owns the
files, so the backend must serve them to the webview, and the webview must
register them as real fonts — not CSS `@font-face` text, but the FontFace API,
so the specimen keeps using plain `font-family: "<Family>"` with zero
source-specific branches (§33).

## 2. Serving decision: custom `font://` scheme

Options considered:

- **convertFileSrc / asset protocol**: designed for bundled `dist/` assets;
  serving app-data files means loosening asset-scope config to the whole data
  directory — more surface than needed.
- **`invoke` → base64 over JSON**: no new CSP surface, but ~33% size bloat and
  full-file buffering in JS for what the browser should stream itself.
- **custom uri-scheme protocol (chosen)**: `register_uri_scheme_protocol` on
  the Rust side; the handler validates the request, streams the file from
  disk with `Content-Type: font/ttf|otf`, and the browser treats it as a
  first-class font URL. Streaming, no JS buffering, no public asset scope.

WebKitGTK translates custom schemes to `http://font.localhost/...` internally,
which the existing `connect-src http://ipc.localhost` pattern already
anticipates; the CSP below allows both forms explicitly.

## 3. URL shape and strict path validation (§31)

```text
font://cached/<family-id>/<filename>
```

- `<family-id>` must match `[a-z0-9-]+` (the slug charset), `<filename>` must
  match `[0-9a-f]{8}\.(ttf|otf)` (the M4 content-addressed naming).
- The resolved path is canonicalized and must sit inside
  `<data_dir>/fonts/<family-id>/`; anything else is 403/404. No `..`, no
  traversal, no frontend-supplied free paths (§31).
- Unknown id or file → 404. The scheme only ever speaks to the cache tree.

## 4. Manifest-driven font registration

`get_font_sources()` (new command) returns a `FontSource` per face that has
cache files:

```text
{ id, family, files: [{ url, weight, style }] }
```

`url` is the full `font://cached/...` URL built by the backend — the frontend
never constructs filesystem paths. The command reads each `metadata.json`
with `downloads::read_manifest` (validated, existence-checked; corrupt or
missing manifests are skipped, never fabricated — same policy as
`library.json`).

The frontend (`ensureLocalFace`) registers one `FontFace` per entry:

```text
new FontFace(family, "url(<font:// URL>)", { weight: String(w), style })
  → load() → document.fonts.add()
```

- Weights/italics come from the manifest, so the specimen's weight slider
  works on instantiated files (400/700) and variable files alike.
- A `Set<family>` records registered families; `loadFace` only awaits local
  registration when the family isn't registered yet (re-selecting a face is
  instant, no duplicate FontFace objects).
- **Fallback preserved (M4 behavior):** if local registration fails (or the
  scheme were somehow blocked) the resolver still tries the remote stylesheet;
  online, the preview always works; offline, remote fails and the specimen
  shows the honest "offline" state instead of pretending.
- **Dev note:** plain-browser dev has no `font://` scheme (it belongs to the
  Tauri runtime), so `ensureLocalFace` is skipped outside Tauri — the remote
  path serves dev previews as before.

## 5. CSP change (tauri.conf.json)

```text
font-src    'self' https://fonts.gstatic.com font:      (+ font:)
connect-src 'self' ipc: http://ipc.localhost font: http://font.localhost
```

`connect-src` must include the scheme because WebKitGTK routes font fetches
for custom schemes through its network stack (same mechanism as the pre-existing
`http://ipc.localhost` entry). No other CSP surface changes.

## 6. What this deliberately does NOT do

- No install/uninstall (M6), no export (M8), no font binary parsing — the
  manifest's weight/style metadata is sufficient for preview.
- No watching the fonts directory; sources are fetched per `loadCatalog` and
  `refreshFaces`, memoized by manifest mtimes.
- No new dependencies; the scheme handler uses `tauri::http::Response` and
  std fs streaming.

## 7. Verified behavior (summary; details in STAGE2_PLAN §12)

- Online, cached ABeeZee preview resolves through `font://` (2 FontFace
  registrations: 400 normal + 400 italic); re-selecting it re-uses the
  registered faces.
- With the network blocked (dead-proxy env for the app process), the specimen
  still renders the cached family from disk; uncached families show the
  offline state; the app does not appear broken (§13).
