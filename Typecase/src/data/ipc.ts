/* Typed IPC boundary. In the Tauri runtime everything goes through invoke();
   in plain-browser dev (vite on its own) the catalog falls back to the same
   generated snapshot served from /catalog.json so the UI stays verifiable. */

import { invoke } from "@tauri-apps/api/core";

const isTauri = () => "__TAURI_INTERNALS__" in window;

export type RawFace = Record<string, unknown>;

/** Full catalog with computed library state, from the backend. */
export async function fetchCatalog(): Promise<RawFace[]> {
  if (isTauri()) {
    return invoke<RawFace[]>("get_catalog");
  }
  const res = await fetch("/catalog.json");
  if (!res.ok) throw new Error(`catalog fetch failed: ${res.status}`);
  const json = (await res.json()) as { records: RawFace[] };
  return json.records;
}

/** M4: download a family from the source, validate every file, and cache it
    under fonts/<family-id>/. Returns the written manifest. Requires the
    Tauri runtime (there is no browser fallback for real downloads). */
export interface CachedFileMeta {
  file: string;
  weight: number;
  style: string;
  size: number;
  sha256: string;
}

export interface FontMeta {
  id: string;
  family: string;
  source: string;
  downloadedAt: string;
  files: CachedFileMeta[];
  fileCount: number;
  totalSize: number;
}

export async function downloadFont(id: string): Promise<FontMeta> {
  if (!isTauri()) throw new Error("Downloading requires the Tauri runtime");
  return invoke<FontMeta>("download_font", { id });
}

/** M5: servable cached-font sources. The backend builds the full `font://`
    URLs from the manifests — the frontend never constructs paths. */
export interface CachedFontFile {
  url: string;
  weight: number;
  style: string;
}

export interface FontSourceInfo {
  id: string;
  family: string;
  files: CachedFontFile[];
}

export async function fetchCachedSources(): Promise<FontSourceInfo[]> {
  if (!isTauri()) return [];
  return invoke<FontSourceInfo[]>("get_font_sources");
}

/** §24: the separate explicit cache-deletion operation. Removes the cached
    files and clears the cached flag; installed state is preserved. The UI
    must ask for confirmation before calling this. */
export async function deleteCachedFamily(
  id: string
): Promise<{ id: string; freedBytes: number }> {
  if (!isTauri()) throw new Error("Deleting requires the Tauri runtime");
  return invoke<{ id: string; freedBytes: number }>("delete_cached_family", { id });
}

/** UI: push the theme preference to the backend so the native window chrome
    (title bar on Windows) follows the in-app dark mode. */
export async function applyWindowTheme(dark: boolean): Promise<void> {
  if (!isTauri()) return; // plain-browser dev has no window chrome
  try {
    await invoke("apply_window_theme", { dark });
  } catch (e) {
    console.error("[typecase] apply_window_theme failed:", e);
  }
}

/** M6: install a cached family for a scope. Requires the Tauri runtime;
    on non-Windows the backend returns an honest error. */
export type InstallScope = "user" | "system";

export interface InstallOutcome {
  id: string;
  family: string;
  scope: InstallScope;
  files: number;
}

export async function installFont(id: string, scope: InstallScope): Promise<InstallOutcome> {
  if (!isTauri()) throw new Error("Installing requires the Tauri runtime");
  return invoke<InstallOutcome>("install_font", { id, scope });
}

/** M6: uninstall a family Typecase installed (reverses exactly its recorded
    entries; cache survives per §24). */
export async function uninstallFont(id: string): Promise<InstallOutcome> {
  if (!isTauri()) throw new Error("Uninstalling requires the Tauri runtime");
  return invoke<InstallOutcome>("uninstall_font", { id });
}

/** M7: every font registered in Windows, with record-based ownership. */
export type Ownership = "managed" | "external" | "unknown";

export interface RegisteredFont {
  valueName: string;
  family: string;
  style: string;
  filePath: string;
  scope: "user" | "system";
  ownership: Ownership;
  id: string | null;
}

export async function fetchInstalledFonts(): Promise<RegisteredFont[]> {
  if (!isTauri()) return [];
  return invoke<RegisteredFont[]>("get_installed_fonts");
}

/** M7 §25: remove an EXTERNAL font (never a managed one — the backend
    refuses). The UI must show the removal warning first. */
export async function removeExternalFont(
  valueName: string,
  filePath: string,
  scope: "user" | "system"
): Promise<void> {
  if (!isTauri()) throw new Error("Removing requires the Tauri runtime");
  await invoke("remove_external_font", { valueName, filePath, scope });
}

/** Details for one face (record + live library state) straight from the
    backend. */
export async function fetchFontDetails(id: string): Promise<RawFace | null> {
  if (!isTauri()) throw new Error("Font details require the Tauri runtime");
  return invoke<RawFace | null>("get_font_details", { id });
}

export const ipcAvailable = isTauri;
