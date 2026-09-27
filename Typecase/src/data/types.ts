/* Typecase font resource — the normalized, source-agnostic face record
   that the UI consumes. CONTEXT.md §33: font source → normalized resource →
   specimen renderer. The prototype's terse `Face` fields (n/c/d/y/s/w/i/t/p)
   are migrated onto this shape by data/catalog.ts.

   This is a staging model: in stage 3 the backend becomes the authority and
   this file keeps only the TS mirror of the IPC payload. */

export type Cat = "Serif" | "Sans" | "Display" | "Mono" | "Script";

/** Coarse lifecycle phase, derived by the backend from library state
    (cached/installed) — see CONTEXT.md §17 and STAGE2_PLAN §2. */
export type Phase = "online" | "library" | "installed";

export type Face = {
  /** Stable id (stage 3+: assigned by the backend catalog). */
  id: string;
  /** Family name, as the source spells it. */
  family: string;
  category: Cat;
  designer: string;
  /** First release on Google Fonts. */
  year: number;
  /** Number of styles in the family. */
  styles: number;
  /** Weights to request from the source. */
  weights: number[];
  italic: boolean;
  /** Specimen note — what the face is actually good at. */
  note: string;
  /** Suggested companion family. */
  pairsWith: string;
  /** Which shell section this face appears in (stage-1 stand-in). */
  phase: Phase;
  /** True when the face resolves from a local file rather than the network. */
  local: boolean;
};
