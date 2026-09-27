/* Typecase i18n — two first-class languages (EN/ES), no dependencies.

   The dictionary holds every user-facing string: chrome (masthead, tabs,
   labels) and the editorial layer (preset specimen copy). Language-neutral
   data stays in the data modules (catalog categories keep their English
   values as filter keys; display labels live here). */

import { useSyncExternalStore } from "react";
import type { Cat, Phase } from "./data/types";
import type { PresetId } from "./data/presets";

export type Lang = "en" | "es";

const en = {
  /* masthead / shell */
  tagline: "Specimen & retrieval utility · SIL OFL 1.1",
  os: "Windows 10 / 11",
  discover: "Discover",
  library: "Library",
  installed: "Installed",
  capDiscover: "Every face in the case",
  capLibrary: "Cached families",
  capInstalled: "Set on this machine",
  colo1:
    "Colophon — set in Bodoni Moda, IBM Plex Sans and IBM Plex Mono, bundled with the application.",
  colo2:
    "Every family indexed here is released under the SIL Open Font Licence 1.1: free to use, embed and modify, including in commercial work, provided the licence travels with the files.",
  colo3:
    "Typecase · a specimen room and retrieval utility · Windows 10 (1809+) and Windows 11 · no account, no telemetry, no tray agent",
  toDark: "Dark",
  toLight: "Light",
  themeToggleTitle: "Toggle dark mode",

  /* rail */
  indexOfFaces: "Index of faces",
  searchFaces: "Search faces",
  searchPlaceholder: "Search family, designer, use…",
  cats: {
    All: "All",
    Serif: "Serif",
    Sans: "Sans",
    Display: "Display",
    Mono: "Mono",
    Script: "Script",
  } as Record<Cat | "All", string>,
  phases: {
    online: "Available online",
    library: "Cached",
    installed: "Installed",
  } as Record<Phase, string>,
  nothingInCase: "Nothing in the case.",
  noMatches:
    "No face matches “$Q” under $C. Clear the filter to see all $T.",
  libEmptyTitle: "The library is empty.",
  libEmptyHint:
    "Open a face in Discover and choose “Cache this family” to keep a local copy.",
  instEmptyTitle: "Nothing is installed yet.",
  instEmptyHint:
    "Install a cached family from the Library view; installed faces appear here.",

  /* app states */
  errLab: "The type case could not be opened",
  errTitle: "Catalog failed to load",
  retry: "Try again",
  opening: "Opening the case…",
  loadingCatalog: "Loading the catalog",
  noFaceSelected: "No face selected.",

  /* sheet */
  nowSetting: "Now setting",
  styles: "styles",
  statusOkLocal: "◧ set from local file",
  statusOkWeb: "◧ webfont set",
  statusFetching: "◔ fetching…",
  statusOffline: "◨ offline — system fallback",
  size: "Size",
  weight: "Weight",
  tracking: "Tracking",
  leading: "Leading",
  single: "single",
  setFor: "Set for",
  waterfall: "Waterfall — same line, ascending body",
  textBlock: "Text block — $L",
  specimenNote: "Specimen note",
  noNote: "No editorial note for this family yet.",
  setsWellWith: "Sets well with",
  pairsExpl:
    "One family for the display line, its companion for the text block. Two families maximum per document.",
  changeFace: "Change face ↓",
  copyCss: "Copy CSS link",
  copyFamily: "Copy family name",
  copied: "Copied ✓",
  gfPage: "Google Fonts page ↗",
  typeHere: "Type here…",
  capOnline:
    "This face is available online — caching keeps a local, offline copy in your library.",
  capCached:
    "Cached in your library — preview works offline from local files, ready to install.",
  capSet: "Installed on this machine.",

  /* cache / delete actions */
  cache: "⤓ Cache this family",
  caching: "◔ Caching…",
  filesOne: "file",
  filesMany: "files",
  cachedOk: "Cached ✓",
  del: "✕ Delete cached files",
  deleting: "◔ Deleting…",
  keep: "Keep the cache",
  delLab: "Delete cached files",
  delTitle: "Remove $F from the local library?",
  delBody:
    "The downloaded copy ($S style(s)) is deleted from this machine and the face returns to “available online”. If it is ever installed, the installed copy is not touched — and Google may no longer list this family, in which case this cache is the only copy Typecase can re-offer.",
  delConfirm: "Delete cached files",

  /* M6 install / uninstall */
  install: "⤒ Install for this user",
  installSystem: "⤒ Install for everyone (UAC)",
  installing: "◔ Installing…",
  installedOk: "Installed ✓",
  uninstall: "⤓ Uninstall",
  uninstalling: "◔ Uninstalling…",
  uninstalledOk: "Uninstalled ✓",
  instLab: "Install font",
  instTitle: "Install $F for $W?",
  instBody:
    "Typecase registers the family with Windows for the whole system: it appears in every application's font list until it is uninstalled. System-wide installation asks for administrator consent once, for this operation only — Typecase itself never runs elevated.",
  instBodyUser:
    "Typecase registers the family with Windows for your user account: it appears in every application's font list and persists across restarts, without administrator consent.",
  instConfirm: "Install",
  uniLab: "Uninstall font",
  uniTitle: "Uninstall $F?",
  uniBody:
    "The family is removed from Windows for $W; applications will no longer list it. The downloaded cache copy is kept, so it can be installed again without re-downloading.",
  uniConfirm: "Uninstall",
  scopeUser: "this user",
  scopeSystem: "everyone",

  /* M7 installed view / external fonts */
  installedViewCap: "Set on this machine — from the Windows registry",
  ownManaged: "Typecase",
  ownExternal: "External",
  instTypeFace: "typeface",
  instTypeFaces: "typefaces",
  extRemove: "✕ Remove from Windows",
  extRemoving: "◔ Removing…",
  extLab: "Remove external font",
  extTitle: "Remove $F from Windows?",
  extBody:
    "This typeface was installed outside Typecase ($S). Other applications and documents may depend on it — removing it affects the whole machine, not just Typecase. The registry entry and its file will be deleted.",
  extCacheOffer:
    "Typecase cannot offer to cache this font yet; export a copy before removing it if you want a backup.",
  extConfirm: "Remove typeface",
  extRemoved: "Removed ✓",
  extFrom: "from your user account — from this machine (administrator)",

  /* presets — the editorial layer */
  presets: {
    essay: {
      label: "Academic essay",
      use: "Body 11 pt · 62 characters · ragged right",
      display: "A typeface is not a font",
      advice: "Prefer a text-cut serif at 400. Avoid display faces below 16 px.",
      body: [
        "The face is the drawing of the alphabet, in all its weights and widths; the font is the delivery mechanism. Once a wooden tray of foundry sorts, later a film negative, now a file on a disk. When a 1920s specimen book lists forty-eight sizes of Caslon it is listing forty-eight fonts of one face, and the confusion that survives into the desktop era is a confusion between the work and its container.",
        "Set long-form text where the type disappears. Choose a text serif with an optical size axis, keep the measure between 60 and 72 characters, and let the line height follow the x-height rather than the point size. Save the contrast for the chapter opening.",
      ],
    },
    cv: {
      label: "Résumé / CV",
      use: "One page · 10.5 pt · parseable by ATS",
      display: "Marta Kovács — Product Designer",
      advice:
        "One family only. 10.5–11 pt, generous leading, no text below 9 pt, no colour-only meaning.",
      body: [
        "EXPERIENCE — Senior Product Designer, Kestrel Systems, Rotterdam · 2021–present. Led the redesign of the billing console used by 14,000 accounts; cut time-to-invoice from 9 minutes to 80 seconds. Built and documented a 240-component design system in Figma and React.",
        "EDUCATION — MA Information Design, Design Academy Eindhoven, 2016. SKILLS — Figma, React, TypeScript, DirectWrite, WCAG 2.2 AA, design systems, technical writing.",
      ],
    },
    poster: {
      label: "Conference poster",
      use: "A1 · headline 400 pt · read at 3 m",
      display: "TYPE 26",
      advice:
        "Go extreme: 300 pt+ against 24 pt. Three lines maximum in the headline block.",
      body: [
        "SHEFFIELD · 14–16 NOVEMBER — Three days of punchcutting, parametric type and variable fonts. Speakers from Production Type, Bold Monday, Omnibus-Type and the St Bride Library. Punchcutting workshop on the Sunday, twelve places only.",
        "Early bird until 30 September. Tickets and programme at type26.example",
      ],
    },
    ui: {
      label: "Product UI",
      use: "Interface 13–15 px · dense tables · WCAG AA",
      display: "Deployments",
      advice:
        "Sans with tabular figures on by default. Test at 125 % and 150 % Windows scaling.",
      body: [
        "Empty state — No deployments yet. Connect a repository to publish your first build, or read the quickstart. Primary action: Connect repository. Secondary: Read the quickstart.",
        "Table — 1,284 rows · Sorted by updated · filter: status = failed. Column labels in tracked small caps at 11 px; all numerals tabular so the columns align down the page. Focus ring 2 px, 4.5:1 against every surface.",
      ],
    },
    letter: {
      label: "Newsletter",
      use: "Email · deck 32 pt · body 17 px",
      display: "The Set Rule",
      advice:
        "Pair two cuts of one superfamily. Deck tight, body loose, one accent colour for links.",
      body: [
        "ISSUE 41 — Why your columns look cramped, and the one measurement that fixes it. Also this week: five variable fonts that finally ship decent italics, and a short defence of the humble comma.",
        "The deck carries the issue. Set it in the display cut at 32 pt, tight, two lines; then drop hard to the text cut at 17 px with a 1.6 line height. The jump is the whole hierarchy — you should not need a third weight.",
      ],
    },
    deck: {
      label: "Slide deck",
      use: "16:9 · 6 rows of text maximum",
      display: "Ship the small thing",
      advice: "Headline 90–130 pt, one weight, one accent. Never centre body copy.",
      body: [
        "One idea per slide. If a slide needs a paragraph, it is a document wearing a slide's clothes — export it as a PDF instead.",
        "Test the deck at the back of the room: if you cannot read the smallest line from eight metres away, it is caption material and belongs in the speaker notes.",
      ],
    },
  } as Record<PresetId, PresetStrings>,
};

export interface PresetStrings {
  label: string;
  use: string;
  display: string;
  advice: string;
  body: [string, string];
}

type Dict = typeof en;

const es: Dict = {
  tagline: "Utilidad de especímenes y recuperación · SIL OFL 1.1",
  os: "Windows 10 / 11",
  discover: "Descubrir",
  library: "Biblioteca",
  installed: "Instaladas",
  capDiscover: "Cada cara de la caja",
  capLibrary: "Familias en caché",
  capInstalled: "Instaladas en esta máquina",
  colo1:
    "Colofón — compuesto en Bodoni Moda, IBM Plex Sans e IBM Plex Mono, incluidos con la aplicación.",
  colo2:
    "Todas las familias indexadas aquí están bajo la SIL Open Font Licence 1.1: libres para usar, incrustar y modificar, incluso en trabajo comercial, siempre que la licencia acompañe a los archivos.",
  colo3:
    "Typecase · una sala de especímenes y utilidad de recuperación · Windows 10 (1809+) y Windows 11 · sin cuenta, sin telemetría, sin agente en bandeja",
  toDark: "Oscuro",
  toLight: "Claro",
  themeToggleTitle: "Cambiar al modo oscuro",

  indexOfFaces: "Índice de caras",
  searchFaces: "Buscar caras",
  searchPlaceholder: "Busca familia, diseñador, uso…",
  cats: {
    All: "Todas",
    Serif: "Serif",
    Sans: "Sans",
    Display: "Display",
    Mono: "Mono",
    Script: "Script",
  },
  phases: {
    online: "Disponible en línea",
    library: "En caché",
    installed: "Instalada",
  },
  nothingInCase: "Nada en la caja.",
  noMatches:
    "Ninguna cara coincide con «$Q» bajo $C. Limpia el filtro para ver las $T.",
  libEmptyTitle: "La biblioteca está vacía.",
  libEmptyHint:
    "Abre una cara en Descubrir y elige «Guardar esta familia» para conservar una copia local.",
  instEmptyTitle: "Nada está instalado todavía.",
  instEmptyHint:
    "Instala una familia en caché desde la vista Biblioteca; las caras instaladas aparecen aquí.",

  errLab: "No se pudo abrir la caja tipográfica",
  errTitle: "Error al cargar el catálogo",
  retry: "Reintentar",
  opening: "Abriendo la caja…",
  loadingCatalog: "Cargando el catálogo",
  noFaceSelected: "Ninguna cara seleccionada.",

  nowSetting: "Componiendo",
  styles: "estilos",
  statusOkLocal: "◧ compuesto desde archivo local",
  statusOkWeb: "◧ fuente web aplicada",
  statusFetching: "◔ recuperando…",
  statusOffline: "◨ sin conexión — reserva del sistema",
  size: "Cuerpo",
  weight: "Peso",
  tracking: "Espaciado",
  leading: "Interlineado",
  single: "único",
  setFor: "Compuesto para",
  waterfall: "Escalera — misma línea, cuerpo ascendente",
  textBlock: "Bloque de texto — $L",
  specimenNote: "Nota de espécimen",
  noNote: "Aún no hay nota editorial para esta familia.",
  setsWellWith: "Combina bien con",
  pairsExpl:
    "Una familia para la línea de display, su compañera para el bloque de texto. Dos familias como máximo por documento.",
  changeFace: "Cambiar de cara ↓",
  copyCss: "Copiar enlace CSS",
  copyFamily: "Copiar nombre de familia",
  copied: "Copiado ✓",
  gfPage: "Página de Google Fonts ↗",
  typeHere: "Escribe aquí…",
  capOnline:
    "Esta cara está disponible en línea — guardarla en caché conserva una copia local y sin conexión en tu biblioteca.",
  capCached:
    "En caché en tu biblioteca — la vista previa funciona sin conexión desde archivos locales, lista para instalar.",
  capSet: "Instalada en esta máquina.",

  cache: "⤓ Guardar esta familia",
  caching: "◔ Guardando…",
  filesOne: "archivo",
  filesMany: "archivos",
  cachedOk: "En caché ✓",
  del: "✕ Eliminar archivos en caché",
  deleting: "◔ Eliminando…",
  keep: "Conservar la caché",
  delLab: "Eliminar archivos en caché",
  delTitle: "¿Quitar $F de la biblioteca local?",
  delBody:
    "La copia descargada ($S estilo(s)) se elimina de esta máquina y la cara vuelve a estar «disponible en línea». Si alguna vez se instaló, la copia instalada no se toca — y Google ya podría no listar esta familia, en cuyo caso esta caché es la única copia que Typecase puede volver a ofrecer.",
  delConfirm: "Eliminar archivos en caché",

  /* M6 instalación / desinstalación */
  install: "⤒ Instalar para este usuario",
  installSystem: "⤒ Instalar para todos (UAC)",
  installing: "◔ Instalando…",
  installedOk: "Instalada ✓",
  uninstall: "⤓ Desinstalar",
  uninstalling: "◔ Desinstalando…",
  uninstalledOk: "Desinstalada ✓",
  instLab: "Instalar fuente",
  instTitle: "¿Instalar $F para $W?",
  instBody:
    "Typecase registra la familia en Windows para todo el sistema: aparecerá en la lista de fuentes de cada aplicación hasta que se desinstale. La instalación para todo el sistema pide consentimiento de administrador una sola vez, solo para esta operación — Typecase nunca se ejecuta elevado.",
  instBodyUser:
    "Typecase registra la familia en Windows para tu cuenta de usuario: aparecerá en la lista de fuentes de cada aplicación y persiste entre reinicios, sin consentimiento de administrador.",
  instConfirm: "Instalar",
  uniLab: "Desinstalar fuente",
  uniTitle: "¿Desinstalar $F?",
  uniBody:
    "La familia se elimina de Windows para $W; las aplicaciones dejarán de mostrarla. La copia en caché se conserva, así que puede instalarse de nuevo sin volver a descargarla.",
  uniConfirm: "Desinstalar",
  scopeUser: "este usuario",
  scopeSystem: "todos",

  /* M7 vista de instaladas / fuentes externas */
  installedViewCap: "Instaladas en esta máquina — desde el registro de Windows",
  ownManaged: "Typecase",
  ownExternal: "Externa",
  instTypeFace: "tipografía",
  instTypeFaces: "tipografías",
  extRemove: "✕ Quitar de Windows",
  extRemoving: "◔ Quitando…",
  extLab: "Quitar fuente externa",
  extTitle: "¿Quitar $F de Windows?",
  extBody:
    "Esta tipografía se instaló fuera de Typecase ($S). Otras aplicaciones y documentos pueden depender de ella — quitarla afecta a toda la máquina, no solo a Typecase. Se eliminarán la entrada del registro y su archivo.",
  extCacheOffer:
    "Typecase aún no puede ofrecer guardar esta fuente; exporta una copia antes de quitarla si quieres una copia de seguridad.",
  extConfirm: "Quitar tipografía",
  extRemoved: "Quitada ✓",
  extFrom: "de tu cuenta de usuario — de esta máquina (administrador)",

  presets: {
    essay: {
      label: "Ensayo académico",
      use: "Cuerpo 11 pt · 62 caracteres · derecha a bandera",
      display: "Una tipografía no es una fuente",
      advice: "Prefiere una serif de texto a 400. Evita caras display por debajo de 16 px.",
      body: [
        "La tipografía es el dibujo del alfabeto, en todos sus pesos y anchuras; la fuente es el mecanismo de entrega. Antes una bandeja de tipos móviles de fundición, luego una negativa fotográfica, hoy un archivo en disco. Cuando un libro de especímenes de los años veinte lista cuarenta y ocho cuerpos de Caslon, lista cuarenta y ocho fuentes de una sola tipografía, y la confusión que sobrevive a la era de escritorio es una confusión entre la obra y su continente.",
        "Compón textos largos donde la tipografía desaparezca. Elige una serif de texto con eje de tamaño óptico, mantén la medida entre 60 y 72 caracteres y deja que el interlineado siga la altura x más que el cuerpo. Reserva el contraste para la apertura de capítulo.",
      ],
    },
    cv: {
      label: "Currículum / CV",
      use: "Una página · 10,5 pt · legible por ATS",
      display: "Marta Kovács — Diseñadora de producto",
      advice:
        "Una sola familia. 10,5–11 pt, interlineado generoso, nada por debajo de 9 pt, sin significado solo por color.",
      body: [
        "EXPERIENCIA — Diseñadora de producto sénior, Kestrel Systems, Róterdam · 2021–actualidad. Dirigí el rediseño de la consola de facturación usada por 14 000 cuentas; reduje el tiempo de facturación de 9 minutos a 80 segundos. Construí y documenté un sistema de diseño de 240 componentes en Figma y React.",
        "FORMACIÓN — Máster en Diseño de Información, Design Academy Eindhoven, 2016. HABILIDADES — Figma, React, TypeScript, DirectWrite, WCAG 2.2 AA, sistemas de diseño, redacción técnica.",
      ],
    },
    poster: {
      label: "Cartel de congreso",
      use: "A1 · titular 400 pt · legible a 3 m",
      display: "TYPE 26",
      advice:
        "Ve al extremo: 300 pt+ contra 24 pt. Tres líneas como máximo en el bloque del titular.",
      body: [
        "SHEFFIELD · 14–16 DE NOVIEMBRE — Tres días de punzonado, tipografía paramétrica y fuentes variables. Ponentes de Production Type, Bold Monday, Omnibus-Type y la St Bride Library. Taller de punzonado el domingo, doce plazas únicamente.",
        "Precio anticipado hasta el 30 de septiembre. Entradas y programa en type26.example",
      ],
    },
    ui: {
      label: "UI de producto",
      use: "Interfaz 13–15 px · tablas densas · WCAG AA",
      display: "Despliegues",
      advice:
        "Sans con cifras tabulares por defecto. Prueba al 125 % y 150 % de escalado de Windows.",
      body: [
        "Estado vacío — Aún no hay despliegues. Conecta un repositorio para publicar tu primera compilación, o lee la guía rápida. Acción principal: Conectar repositorio. Secundaria: Leer la guía rápida.",
        "Tabla — 1 284 filas · Ordenada por actualización · filtro: estado = fallido. Etiquetas de columna en versalitas espaciadas a 11 px; todos los numerales tabulares para que las columnas alineen a lo largo de la página. Anillo de foco 2 px, 4,5:1 sobre cada superficie.",
      ],
    },
    letter: {
      label: "Boletín",
      use: "Correo · entradilla 32 pt · cuerpo 17 px",
      display: "La Pauta",
      advice:
        "Empareja dos cortes de una misma superfamilia. Entradilla cerrada, cuerpo suelto, un color de acento para los enlaces.",
      body: [
        "NÚMERO 41 — Por qué tus columnas se ven apretadas, y la única medida que lo arregla. También esta semana: cinco fuentes variables que por fin estrenan cursivas decentes, y una breve defensa de la humilde coma.",
        "La entradilla carga el número. Compónla en el corte display a 32 pt, cerrada, dos líneas; luego cae en seco al corte de texto a 17 px con 1,6 de interlineado. El salto es toda la jerarquía — no deberías necesitar un tercer peso.",
      ],
    },
    deck: {
      label: "Presentación",
      use: "16:9 · 6 filas de texto como máximo",
      display: "Envía la cosa pequeña",
      advice: "Titular 90–130 pt, un peso, un acento. Nunca centres el cuerpo.",
      body: [
        "Una idea por diapositiva. Si una diapositiva necesita un párrafo, es un documento con ropa de diapositiva — expórtalo como PDF.",
        "Prueba la presentación desde el fondo de la sala: si no puedes leer la línea más pequeña desde ocho metros, es material de subtítulos y pertenece a las notas del orador.",
      ],
    },
  },
};

const DICTS: Record<Lang, Dict> = { en, es };

/* ---- store ---- */

const KEY = "typecase-lang";

function initial(): Lang {
  try {
    const stored = localStorage.getItem(KEY);
    if (stored === "en" || stored === "es") return stored;
  } catch {
    /* storage unavailable */
  }
  return typeof navigator !== "undefined" && navigator.language?.toLowerCase().startsWith("es")
    ? "es"
    : "en";
}

let current: Lang = initial();
const listeners = new Set<() => void>();

function applyLang(): void {
  // <html lang> drives spellcheck, hyphenation and AT pronunciation.
  document.documentElement.lang = current;
}

applyLang();

export function getLang(): Lang {
  return current;
}

export function setLang(lang: Lang): void {
  if (lang === current) return;
  current = lang;
  try {
    localStorage.setItem(KEY, lang);
  } catch {
    /* storage unavailable */
  }
  applyLang();
  for (const fn of listeners) fn();
}

export function subscribeLang(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** The current dictionary — referentially stable per language, so
    useSyncExternalStore never re-renders spuriously. */
export function useI18n(): Dict {
  return useSyncExternalStore(subscribeLang, () => DICTS[current]);
}

/** Tiny $KEY template interpolation for strings that carry values. */
export function fmt(tpl: string, vars: Record<string, string | number>): string {
  let out = tpl;
  for (const [k, v] of Object.entries(vars)) out = out.split(`$${k}`).join(String(v));
  return out;
}

/** Locale-aware number rendering for computed values (tracking, leading):
    es → decimal comma (RAE), en → decimal point. Fixed `digits` decimals. */
export function num(value: number, digits: number): string {
  return new Intl.NumberFormat(current === "es" ? "es-ES" : "en-US", {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

export const LANGS: { id: Lang; label: string }[] = [
  { id: "en", label: "English" },
  { id: "es", label: "Español" },
];
