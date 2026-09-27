/* Extract the curated editorial layer (designer strings, specimen notes,
   pairings) from the prototype catalog into tools/curated-notes.json.

   The prototype's catalog is demo data (CONTEXT.md §8), but its specimen
   notes and pairings are real editorial curation worth carrying into the
   generated catalog for the families they cover. This script runs once;
   the output is committed and consumed by generate-catalog.mjs. */

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const src = path.join(root, "src/data/catalog.ts");

const ts = readFileSync(src, "utf8");
const start = ts.indexOf("export const CATALOG");
const rows = [];
const re = /\{\s*n:\s*"((?:[^"\\]|\\.)*)"\s*,\s*c:\s*"(\w+)"\s*,\s*d:\s*"((?:[^"\\]|\\.)*)"\s*,\s*y:\s*(\d+)\s*,\s*s:\s*(\d+)\s*,\s*w:\s*\[([^\]]*)\]\s*,\s*(?:i:\s*true\s*,\s*)?t:\s*"((?:[^"\\]|\\.)*)"\s*,\s*p:\s*"((?:[^"\\]|\\.)*)"\s*\}/g;

let m;
while ((m = re.exec(ts.slice(start))) !== null) {
  rows.push({
    family: JSON.parse(`"${m[1]}"`),
    designer: JSON.parse(`"${m[3]}"`),
    year: Number(m[4]),
    note: JSON.parse(`"${m[7]}"`),
    pairsWith: JSON.parse(`"${m[8]}"`),
  });
}

if (rows.length < 50) {
  console.error(`Expected the full curated list, parsed only ${rows.length} rows — aborting.`);
  process.exit(1);
}

if (rows.some((r) => r.note === "undefined" || r.pairsWith === "undefined" || !r.note)) {
  console.error("Regex mis-captured a field (found 'undefined' or empty note) — aborting.");
  process.exit(1);
}

const out = path.join(root, "tools/curated-notes.json");
writeFileSync(out, JSON.stringify(rows, null, 2) + "\n");
console.log(`Wrote ${rows.length} curated entries to ${path.relative(root, out)}`);
