/* Face access layer — the single boundary between the UI and the backend.
   In the Tauri runtime this is IPC (get_catalog); in plain-browser dev it
   falls back to the generated snapshot. Faces are identity-stable per id so
   consumers (Sheet effects, memo deps) never re-fire spuriously.

   CONTEXT.md §33: components never touch source specifics — they consume
   normalized Face records from here. */

import { fetchCatalog, type RawFace } from "./ipc";
import { refreshCachedFontSources } from "../lib/fonts";
import { PRESETS, type Preset } from "./presets";
import type { Cat, Face, Phase } from "./types";

export type { Cat, Face, Phase, Preset };
export { PRESETS };

export const CATS: (Cat | "All")[] = ["All", "Serif", "Sans", "Display", "Mono", "Script"];

export const PHASE_LABEL: Record<Phase, string> = {
  online: "Available online",
  library: "Cached",
  installed: "Installed",
};

export const PHASE_ORDER: Record<Phase, number> = { online: 0, library: 1, installed: 2 };

/* ---- normalization: wire payload → Face ---- */

function toFace(raw: RawFace): Face {
  return {
    id: String(raw.id),
    family: String(raw.family),
    category: (raw.category as Cat) ?? "Sans",
    designer: typeof raw.designer === "string" ? raw.designer : "",
    year: Number(raw.year) || 0,
    styles: Number(raw.styles) || 0,
    weights: Array.isArray(raw.weights) && raw.weights.length ? raw.weights.map(Number) : [400],
    italic: !!raw.italic,
    note: typeof raw.note === "string" ? raw.note : "",
    pairsWith: typeof raw.pairsWith === "string" ? raw.pairsWith : "",
    phase: (raw.phase as Phase) ?? "online",
    local: !!raw.local,
  };
}

/* ---- stable-identity store ----
   Snapshot getters MUST return referentially stable values (useSyncExternalStore
   compares with Object.is), so the derived list and meta object are cached and
   only rebuilt inside absorb(). */

let facesById = new Map<string, Face>();
let cachedList: Face[] = [];
let meta = { loaded: false, error: null as string | null };
const listeners = new Set<() => void>();

function absorb(records: RawFace[]) {
  const next = new Map<string, Face>();
  for (const raw of records) {
    const id = String(raw.id);
    // Reuse the existing object when unchanged so identities stay stable.
    const prev = facesById.get(id);
    const face = prev && JSON.stringify(prev) === JSON.stringify(toFace(raw)) ? prev : toFace(raw);
    next.set(id, face);
  }
  facesById = next;
  cachedList = [...next.values()];
  meta = { ...meta, loaded: true };
}

export function subscribe(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function notify() {
  for (const fn of listeners) fn();
}

export async function loadCatalog(): Promise<void> {
  try {
    absorb(await fetchCatalog());
    void refreshCachedFontSources();
  } catch (e) {
    meta = { loaded: meta.loaded, error: e instanceof Error ? e.message : String(e) };
  }
  notify();
}

export function catalogMeta(): { loaded: boolean; error: string | null } {
  return meta;
}

/* ---- queries (synchronous over the loaded snapshot) ---- */

export function allFaces(): Face[] {
  return cachedList;
}

export function faceById(id: string): Face | undefined {
  return facesById.get(id);
}

export function facesInPhase(phase: Phase): Face[] {
  return allFaces().filter((f) => f.phase === phase);
}

/** Re-fetch the catalog after a backend mutation (M4 download) and absorb it
    into the store. Unchanged faces keep their object identity, so specimen
    effects do not re-fire; changed faces (phase moved) swap cleanly. */
export async function refreshFaces(): Promise<void> {
  try {
    absorb(await fetchCatalog());
    void refreshCachedFontSources();
    notify();
  } catch (e) {
    // A failed refresh keeps the current snapshot; the action that triggered
    // it reports its own error to the user.
    console.error("[typecase] refresh failed:", e);
  }
}
