/* Dark mode — the .dark class on <html> flips the semantic palette variables
   in index.css; everything themed through bg-paper/text-ink/… follows.
   Preference persists; with NO stored preference the app follows the OS
   scheme live while running (prefers-color-scheme change listener), and an
   explicit toggle always wins over the system (WINDOWS_VALIDATION 2.4). */

import { useSyncExternalStore } from "react";
import { applyWindowTheme } from "./data/ipc";

type Mode = "light" | "dark";
const KEY = "typecase-theme";

function stored(): Mode | null {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : null;
  } catch {
    return null;
  }
}

function initial(): boolean {
  const s = stored();
  if (s) return s === "dark";
  return typeof matchMedia !== "undefined" && matchMedia("(prefers-color-scheme: dark)").matches;
}

let dark = initial();
const listeners = new Set<() => void>();

function apply(): void {
  document.documentElement.classList.toggle("dark", dark);
  // native window chrome follows the app theme (title bar on Windows)
  void applyWindowTheme(dark);
}

apply();

/* Follow OS scheme changes live — only while no preference is stored. The
   stored value is re-read at event time: a preference set (or cleared) later
   in the session changes the outcome of subsequent system flips. The native
   window chrome follows along, since apply() pushes to the backend. */
const systemScheme =
  typeof matchMedia !== "undefined" ? matchMedia("(prefers-color-scheme: dark)") : null;

systemScheme?.addEventListener("change", (e) => {
  if (stored()) return; // explicit preference wins over the system
  if (e.matches === dark) return; // no-op flip, don't notify
  dark = e.matches;
  apply();
  for (const fn of listeners) fn();
});

export function isDark(): boolean {
  return dark;
}

export function setDark(next: boolean): void {
  if (next === dark) return;
  dark = next;
  try {
    localStorage.setItem(KEY, next ? "dark" : "light");
  } catch {
    /* storage unavailable */
  }
  apply();
  for (const fn of listeners) fn();
}

export function subscribeDark(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function useDark(): boolean {
  return useSyncExternalStore(subscribeDark, isDark);
}
