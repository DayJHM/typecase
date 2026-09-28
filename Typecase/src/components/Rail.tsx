import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { Face } from "../data/types";
import { fmt, useI18n } from "../i18n";
import { loadFace } from "../lib/fonts";

/** sets a row's name in its own face, loading the webfont only when it nears the viewport.
    `removed` (M8 §16) marks families the source no longer lists — metadata only. */
function FaceName({ face, removed }: { face: Face; removed?: boolean }) {
  const t = useI18n();
  const ref = useRef<HTMLSpanElement>(null);
  const [state, setState] = useState<"idle" | "loading" | "ok" | "error">("idle");

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const io = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        io.disconnect();
        setState("loading");
        loadFace(face).then((r) => setState(r === "ok" ? "ok" : "error"));
      },
      { rootMargin: "300px 0px" }
    );
    io.observe(el);
    return () => io.disconnect();
  }, [face]);

  return (
    <span
      ref={ref}
      className="block truncate text-[25px] leading-[1.1] transition-[filter,opacity] duration-500"
      style={{
        fontFamily: `"${face.family}", "Bodoni Moda", Georgia, serif`,
        fontWeight: 400,
        filter: state === "ok" ? "blur(0)" : state === "loading" ? "blur(3px)" : "none",
        opacity: state === "error" ? 0.4 : 1,
      }}
    >
      {face.family}
      {removed && (
        <span className="lab ml-2 align-super text-[11px] text-verm" title={t.removedBadge}>
          ✝
        </span>
      )}
    </span>
  );
}

const ROW_H = 64; // px — must match the row layout (py-2.5 + two lines)

type Props = {
  faces: Face[];
  total: number;
  selected: Face | null;
  onSelect: (f: Face) => void;
  query: string;
  setQuery: (q: string) => void;
  cat: Face["category"] | "All";
  setCat: (c: Face["category"] | "All") => void;
  /** View-specific zero-state copy (Library/Installed); the default speaks
      to query filters. Strings come pre-rendered from App via i18n. */
  empty?: { title: string; hint: string };
};

export default function Rail({
  faces,
  total,
  selected,
  onSelect,
  query,
  setQuery,
  cat,
  setCat,
  empty,
}: Props) {
  const t = useI18n();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportH, setViewportH] = useState(600);

  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const measure = () => setViewportH(el.clientHeight || 600);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const overscan = 6;
  const first = Math.max(0, Math.floor(scrollTop / ROW_H) - overscan);
  const visibleCount = Math.ceil(viewportH / ROW_H) + overscan * 2;
  const slice = faces.slice(first, first + visibleCount);

  const cats = ["All", "Serif", "Sans", "Display", "Mono", "Script"] as const;

  return (
    <div className="flex h-full flex-col">
      <div className="sticky top-0 z-10 bg-paper pt-5 pb-3">
        <div className="flex items-baseline justify-between border-b border-ink pb-1">
          <h2 className="lab">{t.indexOfFaces}</h2>
          <span className="lab num text-warm">
            {String(faces.length).padStart(2, "0")}/{total}
          </span>
        </div>

        <label className="mt-3 block">
          <span className="sr-only">{t.searchFaces}</span>
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t.searchPlaceholder}
            className="mono w-full border-b border-rule bg-transparent pb-1.5 text-[13px] placeholder:text-warm/70 focus:border-verm focus:outline-none"
          />
        </label>

        <div className="mt-3 flex flex-wrap gap-x-3 gap-y-1">
          {cats.map((c) => (
            <button
              key={c}
              onClick={() => setCat(c)}
              className={`lab transition-colors ${
                cat === c ? "text-verm" : "text-warm hover:text-ink"
              }`}
            >
              {cat === c ? "▍" : ""}
              {c === "All" ? t.cats.All : t.cats[c]}
            </button>
          ))}
        </div>
      </div>

      {/* windowed list: a fixed-row-height spacer sized to the full list, with
          only the visible slice rendered — 1,900+ rows without DOM weight */}
      <div
        ref={scrollRef}
        className="rail min-h-0 flex-1 overflow-y-auto border-b border-rule pb-10 lg:border-b-0"
        onScroll={(e) => setScrollTop((e.target as HTMLDivElement).scrollTop)}
      >
        <div style={{ height: faces.length * ROW_H, position: "relative" }}>
          <div style={{ transform: `translateY(${first * ROW_H}px)` }}>
            {slice.map((f) => {
              const on = !!selected && f.family === selected.family;
              return (
                <div key={f.id} style={{ height: ROW_H }} className="border-b border-rule">
                  <button
                    onClick={() => onSelect(f)}
                    aria-current={on ? "true" : undefined}
                    className="group relative block h-full w-full py-2.5 pr-2 pl-3 text-left transition-[padding] duration-200 hover:pl-5 focus-visible:pl-5 focus-visible:outline-none"
                  >
                    <span
                      className={`absolute top-0 bottom-0 left-0 w-[3px] bg-verm transition-transform duration-200 ${
                        on ? "scale-y-100" : "scale-y-0 group-hover:scale-y-50"
                      }`}
                      style={{ transformOrigin: "top" }}
                    />
                    <FaceName face={f} removed={f.removedFromSource} />
                    <span className="mt-0.5 flex items-baseline justify-between gap-2">
                      <span className="lab truncate text-warm group-hover:text-ink">
                        {f.designer}
                      </span>
                      <span className="lab num shrink-0 text-warm">
                        {on ? <span className="text-verm">SEL </span> : null}
                        {f.year || ""}
                      </span>
                    </span>
                  </button>
                </div>
              );
            })}
          </div>
        </div>

        {faces.length === 0 && (
          <div className="px-3 py-10">
            <p className="disp text-[28px] leading-tight">
              {empty?.title ?? t.nothingInCase}
            </p>
            <p className="lab mt-2 text-warm">
              {empty?.hint ??
                fmt(t.noMatches, {
                  Q: query,
                  C: cat === "All" ? t.cats.All : t.cats[cat],
                  T: total,
                })}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
