/* Typecase font loading — source-agnostic.

   A face is resolved by what it IS (face.local), never by a source branch in
   the specimen: cached faces load from backend-served files (M5 `font://`
   scheme + FontFace registration), uncached faces load from the source's
   stylesheet endpoint. Local resolution falls back to remote when it fails,
   so online previews always work; offline, cached faces still render and
   everything else shows the honest offline state (CONTEXT §13, §33).

   Google-specific URL building lives here and will migrate behind Tauri IPC
   in later stages without touching the specimen component. */

import { fetchCachedSources, ipcAvailable, type FontSourceInfo } from "../data/ipc";

const slug = (f: string) => encodeURIComponent(f).replace(/%20/g, "+");

const remoteCache = new Map<string, Promise<"ok" | "error">>();

function mount(url: string, fallback?: string): Promise<"ok" | "error"> {
  return new Promise((resolve) => {
    const link = document.createElement("link");
    link.rel = "stylesheet";
    link.href = url;
    let settled = false;
    const done = (r: "ok" | "error") => {
      if (settled) return;
      settled = true;
      resolve(r);
    };
    const timer = window.setTimeout(() => done("error"), 7000);
    link.onload = () => {
      window.clearTimeout(timer);
      done("ok");
    };
    link.onerror = () => {
      window.clearTimeout(timer);
      link.remove();
      if (fallback) {
        mount(fallback).then(done);
      } else {
        done("error");
      }
    };
    document.head.appendChild(link);
  });
}

/** Load a remote family stylesheet with a three-stage fallback so a family
    never fails to set: 1. weights+italics  2. weights only  3. whatever the
    source serves by default. */
export function loadRemoteFace(family: string, weights: number[], italic = false): Promise<"ok" | "error"> {
  const hit = remoteCache.get(family);
  if (hit) return hit;

  const base = `https://fonts.googleapis.com/css2?family=${slug(family)}&display=swap`;
  const w = [...new Set(weights)].sort((a, b) => a - b).join(";");
  const wOnly = `https://fonts.googleapis.com/css2?family=${slug(family)}:wght@${w}&display=swap`;
  const full = italic
    ? `https://fonts.googleapis.com/css2?family=${slug(family)}:ital,wght@0,${w};1,${Math.min(400, w ? Number(w.split(";")[0]) : 400)}&display=swap`
    : wOnly;

  const p = mount(full, wOnly === full ? base : wOnly).catch(() => "error" as const);
  remoteCache.set(family, p);
  return p;
}

/** ---- M5: local (cached) faces ----
    The backend serves cached files over the `font://` scheme; each manifest
    entry becomes a FontFace with real weight/style so the specimen's weight
    slider works on instantiated files. Sources are memoized per family and
    refreshed whenever the catalog re-absorbs (face.ts). */

let sourcesPromise: Promise<Map<string, FontSourceInfo>> | null = null;
const localRegistered = new Set<string>();
const localFaces = new Map<string, FontFace[]>();

/** Re-fetch the servable-source map (call after downloads / catalog loads). */
export function refreshCachedFontSources(): Promise<Map<string, FontSourceInfo>> {
  sourcesPromise = fetchCachedSources().then((list) => {
    const map = new Map<string, FontSourceInfo>();
    for (const s of list) map.set(s.family, s);
    return map;
  });
  return sourcesPromise;
}

function cachedSources(): Promise<Map<string, FontSourceInfo>> {
  return (sourcesPromise ?? refreshCachedFontSources());
}

/** Register every manifest file of a family as a FontFace. Per-file failures
    are non-fatal (other files still register); success = at least one face
    loaded. Idempotent per family via `localRegistered`. */
async function ensureLocalFace(source: FontSourceInfo): Promise<boolean> {
  if (localRegistered.has(source.family)) return true;
  let any = false;
  const added: FontFace[] = [];
  for (const f of source.files) {
    try {
      const face = new FontFace(source.family, `url(${f.url})`, {
        weight: String(f.weight),
        style: f.style,
      });
      await face.load();
      document.fonts.add(face);
      added.push(face);
      any = true;
    } catch {
      /* try the remaining files; the resolver falls back to remote */
    }
  }
  if (any) localFaces.set(source.family, added);
  localRegistered.add(source.family);
  return any;
}

/** Drop a family's registered FontFaces (§24 cache deletion): the files are
    gone, so the specimen must fall back to the remote stylesheet instead of
    holding stale faces. A later re-download re-registers fresh. */
export function forgetLocalFace(family: string): void {
  for (const face of localFaces.get(family) ?? []) {
    try {
      document.fonts.delete(face);
    } catch {
      /* FontFaceSet API unavailable — nothing to undo */
    }
  }
  localFaces.delete(family);
  localRegistered.delete(family);
}

/** Resolve any Typecase face for the specimen: cached faces from backend-
    served files (falling back to the source's stylesheet when registration
    fails — online previews always work), uncached faces from the stylesheet. */
export async function loadFace(face: {
  family: string;
  weights: number[];
  italic?: boolean;
  local?: boolean;
}): Promise<"ok" | "error"> {
  if (face.local && ipcAvailable()) {
    const source = (await cachedSources()).get(face.family);
    if (source && (await ensureLocalFace(source))) return "ok";
  }
  return loadRemoteFace(face.family, face.weights, !!face.italic);
}

export const specimenUrl = (family: string) =>
  `https://fonts.google.com/specimen/${slug(family)}`;

/** Keep the CSS snippet around for the current Google source; it moves behind
    IPC together with the download logic in a later stage. */
export function cssSnippet(family: string, weights: number[]) {
  const w = [...new Set(weights)].sort((a, b) => a - b);
  const list = w.join(";");
  const varName = `font-${slug(family).toLowerCase().replace(/\+/g, "-")}`;
  return `<!-- ${family} — SIL Open Font Licence 1.1 -->
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet"
  href="https://fonts.googleapis.com/css2?family=${slug(family)}:wght@${list}&display=swap">

<style>
  :root { --${varName}: "${family}", Georgia, serif; }
  body { font-family: var(--${varName}); font-weight: 400; }
</style>`;
}

export async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand("copy");
    } catch {
      ok = false;
    }
    ta.remove();
    return ok;
  }
}
