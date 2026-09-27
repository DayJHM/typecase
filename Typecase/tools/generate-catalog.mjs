/* Typecase catalog generator (CONTEXT.md §14).

   Fetches Google Fonts' public family-metadata endpoint (no API key, no auth),
   merges it with the curated editorial layer (tools/curated-notes.json), and
   emits the compact Typecase catalog consumed by the Rust backend (embedded
   snapshot) and by the browser-dev fallback (public/catalog.json).

   The catalog contains METADATA ONLY — never font binaries (§14).

   Usage:
     node tools/generate-catalog.mjs                      # fetch + write
     node tools/generate-catalog.mjs --in /tmp/meta.json  # offline re-run

   Outputs:
     src-tauri/resources/catalog.json   (embedded snapshot for the backend)
     public/catalog.json                (browser-dev fallback only) */

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/* ---- args ---- */
let inFile = null;
const argv = process.argv.slice(2);
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === "--in") inFile = argv[++i];
}

/* ---- category mapping: Google's categories → Typecase's taxonomy ---- */
const CAT = {
  "Sans Serif": "Sans",
  Serif: "Serif",
  Display: "Display",
  Monospace: "Mono",
  Handwriting: "Script",
};

function mapCategory(g) {
  if (CAT[g]) return CAT[g];
  // Never expected with today's five categories, but keep the mapping honest:
  console.warn(`Unknown Google category "${g}" — mapping to Sans.`);
  return "Sans";
}

/* ---- weight derivation ----
   Named-instance keys ("400", "700i", …) cover static families and variable
   families alike (the metadata lists every named instance). Derived weights are
   what the specimen asks the CSS2 API for. */
function deriveWeights(fonts) {
  const set = new Set();
  for (const key of Object.keys(fonts)) {
    const w = parseInt(key, 10);
    if (!Number.isNaN(w)) set.add(w);
  }
  const weights = [...set].sort((a, b) => a - b);
  return weights.length ? weights : [400];
}

const hasItalic = (fonts) => Object.keys(fonts).some((k) => k.endsWith("i"));

/* ---- id slug (stable, backend convention) ---- */
const slug = (family) =>
  family
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");

/* ---- load curated editorial layer ---- */
const curated = new Map();
for (const row of JSON.parse(readFileSync(path.join(root, "tools/curated-notes.json"), "utf8"))) {
  curated.set(row.family, row);
}

/* ---- fetch (or read) the metadata ---- */
let raw;
if (inFile) {
  raw = readFileSync(inFile, "utf8");
} else {
  const res = await fetch("https://fonts.google.com/metadata/fonts");
  if (!res.ok) throw new Error(`metadata fetch failed: ${res.status}`);
  raw = await res.text();
}
// The endpoint prefixes a junk line before the JSON payload.
const json = JSON.parse(raw.startsWith(")]}") ? raw.slice(raw.indexOf("\n") + 1) : raw);

/* ---- merge ---- */
const now = new Date().toISOString().slice(0, 10);
const records = json.familyMetadataList.map((f) => {
  const c = curated.get(f.family);
  const category = mapCategory(f.category);
  return {
    id: slug(f.family),
    family: f.family,
    category,
    designer: c?.designer ?? f.designers.join(", ") ?? "",
    year: c?.year ?? (Number((f.dateAdded || "").slice(0, 4)) || 0),
    styles: Object.keys(f.fonts).length,
    weights: deriveWeights(f.fonts),
    italic: hasItalic(f.fonts),
    note: c?.note ?? "",
    pairsWith: c?.pairsWith ?? "",
    popularity: typeof f.popularity === "number" ? f.popularity : 0,
  };
});

/* Canonical order: code-point order, NOT localeCompare — the Rust backend
   sorts by UTF-8 byte order (String Ord), and the two collations disagree on
   case/punctuation. Code-point order keeps the embedded snapshot and any
   backend re-sort byte-identical. */
records.sort((a, b) => (a.family < b.family ? -1 : a.family > b.family ? 1 : 0));

const catalog = {
  generated: now,
  source: "google-fonts",
  count: records.length,
  records,
};

/* ---- emit ---- */
mkdirSync(path.join(root, "src-tauri/resources"), { recursive: true });
const backendPath = path.join(root, "src-tauri/resources/catalog.json");
const publicPath = path.join(root, "public/catalog.json");
writeFileSync(backendPath, JSON.stringify(catalog));
writeFileSync(publicPath, JSON.stringify(catalog));

const curatedCount = records.filter((r) => r.note).length;
const kb = (p) => `${(readFileSync(p).length / 1024).toFixed(0)} KB`;
console.log(
  `catalog: ${records.length} families (${curatedCount} with curated notes)\n` +
    `  ${path.relative(root, backendPath)} — ${kb(backendPath)}\n` +
    `  ${path.relative(root, publicPath)} — ${kb(publicPath)}`
);
