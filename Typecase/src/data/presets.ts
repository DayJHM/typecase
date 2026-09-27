/* The document presets: the actual question is "which face for which job".
   Carried over from the prototype specimen unchanged. Labels and editorial
   copy render through i18n (src/i18n.ts); the numbers and structure are
   language-neutral. */

export type PresetId = "essay" | "cv" | "poster" | "ui" | "letter" | "deck";

export type Preset = {
  id: PresetId;
  label: string;
  use: string;
  size: number;
  weight: number;
  track: number;
  lead: number;
  display: string;
  body: string[];
  advice: string;
};

export const PRESETS: Preset[] = [
  {
    id: "essay",
    label: "Academic essay",
    use: "Body 11 pt · 62 characters · ragged right",
    size: 96,
    weight: 400,
    track: -0.01,
    lead: 1.05,
    display: "A typeface is not a font",
    body: [
      "The face is the drawing of the alphabet, in all its weights and widths; the font is the delivery mechanism. Once a wooden tray of foundry sorts, later a film negative, now a file on a disk. When a 1920s specimen book lists forty-eight sizes of Caslon it is listing forty-eight fonts of one face, and the confusion that survives into the desktop era is a confusion between the work and its container.",
      "Set long-form text where the type disappears. Choose a text serif with an optical size axis, keep the measure between 60 and 72 characters, and let the line height follow the x-height rather than the point size. Save the contrast for the chapter opening.",
    ],
    advice: "Prefer a text-cut serif at 400. Avoid display faces below 16 px.",
  },
  {
    id: "cv",
    label: "Résumé / CV",
    use: "One page · 10.5 pt · parseable by ATS",
    size: 64,
    weight: 600,
    track: -0.02,
    lead: 1.1,
    display: "Marta Kovács — Product Designer",
    body: [
      "EXPERIENCE — Senior Product Designer, Kestrel Systems, Rotterdam · 2021–present. Led the redesign of the billing console used by 14,000 accounts; cut time-to-invoice from 9 minutes to 80 seconds. Built and documented a 240-component design system in Figma and React.",
      "EDUCATION — MA Information Design, Design Academy Eindhoven, 2016. SKILLS — Figma, React, TypeScript, DirectWrite, WCAG 2.2 AA, design systems, technical writing.",
    ],
    advice: "One family only. 10.5–11 pt, generous leading, no text below 9 pt, no colour-only meaning.",
  },
  {
    id: "poster",
    label: "Conference poster",
    use: "A1 · headline 400 pt · read at 3 m",
    size: 190,
    weight: 800,
    track: -0.03,
    lead: 0.92,
    display: "TYPE 26",
    body: [
      "SHEFFIELD · 14–16 NOVEMBER — Three days of punchcutting, parametric type and variable fonts. Speakers from Production Type, Bold Monday, Omnibus-Type and the St Bride Library. Punchcutting workshop on the Sunday, twelve places only.",
      "Early bird until 30 September. Tickets and programme at type26.example",
    ],
    advice: "Go extreme: 300 pt+ against 24 pt. Three lines maximum in the headline block.",
  },
  {
    id: "ui",
    label: "Product UI",
    use: "Interface 13–15 px · dense tables · WCAG AA",
    size: 56,
    weight: 500,
    track: -0.01,
    lead: 1.15,
    display: "Deployments",
    body: [
      "Empty state — No deployments yet. Connect a repository to publish your first build, or read the quickstart. Primary action: Connect repository. Secondary: Read the quickstart.",
      "Table — 1,284 rows · Sorted by updated · filter: status = failed. Column labels in tracked small caps at 11 px; all numerals tabular so the columns align down the page. Focus ring 2 px, 4.5:1 against every surface.",
    ],
    advice: "Sans with tabular figures on by default. Test at 125 % and 150 % Windows scaling.",
  },
  {
    id: "letter",
    label: "Newsletter",
    use: "Email · deck 32 pt · body 17 px",
    size: 84,
    weight: 400,
    track: -0.02,
    lead: 1.02,
    display: "The Set Rule",
    body: [
      "ISSUE 41 — Why your columns look cramped, and the one measurement that fixes it. Also this week: five variable fonts that finally ship decent italics, and a short defence of the humble comma.",
      "The deck carries the issue. Set it in the display cut at 32 pt, tight, two lines; then drop hard to the text cut at 17 px with a 1.6 line height. The jump is the whole hierarchy — you should not need a third weight.",
    ],
    advice: "Pair two cuts of one superfamily. Deck tight, body loose, one accent colour for links.",
  },
  {
    id: "deck",
    label: "Slide deck",
    use: "16:9 · 6 rows of text maximum",
    size: 130,
    weight: 700,
    track: -0.03,
    lead: 0.98,
    display: "Ship the small thing",
    body: [
      "One idea per slide. If a slide needs a paragraph, it is a document wearing a slide's clothes — export it as a PDF instead.",
      "Test the deck at the back of the room: if you cannot read the smallest line from eight metres away, it is caption material and belongs in the speaker notes.",
    ],
    advice: "Headline 90–130 pt, one weight, one accent. Never centre body copy.",
  },
];
