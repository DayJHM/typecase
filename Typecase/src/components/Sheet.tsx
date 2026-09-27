import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { Face } from "../data/types";
import { PRESETS, type Preset } from "../data/presets";
import { allFaces, refreshFaces } from "../data/face";
import { deleteCachedFamily, downloadFont, installFont, uninstallFont, type InstallScope } from "../data/ipc";
import { copyText, cssSnippet, forgetLocalFace, loadFace, specimenUrl } from "../lib/fonts";
import { fmt, num, useI18n } from "../i18n";

const WATERFALL = [12, 16, 21, 28, 37, 49, 65, 87];

function Gauge({ size }: { size: number }) {
  const pct = (size - 12) / (200 - 12);
  return (
    <div className="flex items-end gap-[3px]" aria-hidden="true">
      {Array.from({ length: 41 }).map((_, i) => (
        <span
          key={i}
          className={`w-px transition-all duration-150 ${i / 40 <= pct ? "bg-verm" : "bg-rule"}`}
          style={{ height: i % 5 === 0 ? 11 : 5 }}
        />
      ))}
    </div>
  );
}

function CopyBtn({ label, doneLabel, getText }: { label: string; doneLabel: string; getText: () => string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      onClick={async () => {
        const ok = await copyText(getText());
        setDone(ok);
        window.setTimeout(() => setDone(false), 1600);
      }}
      className={`lab border px-3 py-2 transition-colors duration-150 ${
        done
          ? "border-verm bg-verm text-paper"
          : "border-ink hover:bg-ink hover:text-paper"
      }`}
    >
      {done ? doneLabel : label}
    </button>
  );
}

/** §24: explicit confirmation for the destructive cache-deletion operation.
    An overlay dialog in the Typecase visual language; focus lands on Keep so
    Enter keeps the cache and the removal needs a deliberate Tab+Enter. */
function ConfirmDialog({
  face,
  onConfirm,
  onCancel,
}: {
  face: Face;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const t = useI18n();
  const keepRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    keepRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 p-6"
      role="presentation"
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={fmt(t.delTitle, { F: face.family })}
        className="max-w-[52ch] border border-ink bg-paper p-8"
      >
        <p className="lab text-verm">{t.delLab}</p>
        <p className="disp mt-3 text-[26px] leading-tight">
          {fmt(t.delTitle, { F: face.family })}
        </p>
        <p className="lab mt-3 text-warm">{fmt(t.delBody, { S: face.styles })}</p>
        <div className="mt-6 flex items-center gap-3">
          <button
            ref={keepRef}
            onClick={onCancel}
            className="lab border border-ink px-4 py-2.5 transition-colors duration-150 hover:bg-ink hover:text-paper"
          >
            {t.keep}
          </button>
          <button
            onClick={onConfirm}
            className="lab border border-verm px-4 py-2.5 text-verm transition-colors duration-150 hover:bg-verm hover:text-paper"
          >
            {t.delConfirm}
          </button>
        </div>
      </div>
    </div>
  );
}

/** §24 retrieval action for cached faces: delete the local files after an
    explicit confirmation. Plain-browser dev has no deletion path. */
function DeleteButton({ face }: { face: Face }) {
  const t = useI18n();
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const del = async () => {
    setBusy(true);
    setError(null);
    try {
      await deleteCachedFamily(face.id);
      forgetLocalFace(face.family);
      setConfirming(false);
      await refreshFaces();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <span className="inline-flex flex-col gap-1">
        <button
          onClick={() => setConfirming(true)}
          disabled={busy}
          title={t.delLab}
          className={`lab border px-3 py-2 transition-colors duration-150 ${
            busy
              ? "border-warm text-warm"
              : "border-ink text-warm hover:border-verm hover:text-verm"
          }`}
        >
          {busy ? t.deleting : t.del}
        </button>
        {error && <span className="lab text-verm">{error}</span>}
      </span>
      {confirming && (
        <ConfirmDialog face={face} onConfirm={del} onCancel={() => setConfirming(false)} />
      )}
    </>
  );
}

/** M6: explicit confirmation for installing a family into Windows. */
function InstallConfirm({
  face,
  scope,
  onConfirm,
  onCancel,
}: {
  face: Face;
  scope: InstallScope;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const t = useI18n();
  const keepRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    keepRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);
  const where = scope === "user" ? t.scopeUser : t.scopeSystem;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 p-6" role="presentation">
      <div
        role="dialog"
        aria-modal="true"
        aria-label={fmt(t.instTitle, { F: face.family, W: where })}
        className="max-w-[52ch] border border-ink bg-paper p-8"
      >
        <p className="lab text-verm">{t.instLab}</p>
        <p className="disp mt-3 text-[26px] leading-tight">{fmt(t.instTitle, { F: face.family, W: where })}</p>
        <p className="lab mt-3 text-warm">{scope === "user" ? t.instBodyUser : t.instBody}</p>
        <div className="mt-6 flex items-center gap-3">
          <button
            ref={keepRef}
            onClick={onCancel}
            className="lab border border-ink px-4 py-2.5 transition-colors duration-150 hover:bg-ink hover:text-paper"
          >
            {t.keep}
          </button>
          <button
            onClick={onConfirm}
            className="lab border border-verm px-4 py-2.5 text-verm transition-colors duration-150 hover:bg-verm hover:text-paper"
          >
            {t.instConfirm}
          </button>
        </div>
      </div>
    </div>
  );
}

/** M6: install the cached family for a scope, after explicit confirmation.
    Non-Windows platforms return the backend's honest error. */
function InstallButtons({ face }: { face: Face }) {
  const t = useI18n();
  const [confirming, setConfirming] = useState<InstallScope | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const install = async (scope: InstallScope) => {
    setBusy(true);
    setError(null);
    try {
      const out = await installFont(face.id, scope);
      setDone(`${out.files} ${out.files === 1 ? t.filesOne : t.filesMany}`);
      await new Promise((r) => window.setTimeout(r, 900));
      setConfirming(null);
      await refreshFaces();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  if (done) {
    return (
      <span className="lab border border-verm px-3 py-2 text-verm">{t.installedOk} {done}</span>
    );
  }
  return (
    <>
      <span className="inline-flex flex-col gap-1">
        <span className="inline-flex gap-2">
          <button
            onClick={() => setConfirming("user")}
            disabled={busy}
            className="lab border border-verm px-3 py-2 text-verm transition-colors duration-150 hover:bg-verm hover:text-paper"
          >
            {busy ? t.installing : t.install}
          </button>
          <button
            onClick={() => setConfirming("system")}
            disabled={busy}
            className="lab border border-ink px-3 py-2 text-warm transition-colors duration-150 hover:border-verm hover:text-verm"
          >
            {t.installSystem}
          </button>
        </span>
        {error && <span className="lab text-verm">{error}</span>}
      </span>
      {confirming && (
        <InstallConfirm
          face={face}
          scope={confirming}
          onConfirm={() => install(confirming)}
          onCancel={() => setConfirming(null)}
        />
      )}
    </>
  );
}

/** M6: uninstall a family Typecase installed — reverses exactly the recorded
    entries; the cache survives (§24). */
function UninstallButton({ face }: { face: Face }) {
  const t = useI18n();
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const uni = async () => {
    setBusy(true);
    setError(null);
    try {
      await uninstallFont(face.id);
      setConfirming(false);
      setDone(true);
      await refreshFaces();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  if (done) {
    return <span className="lab border border-ink px-3 py-2 text-warm">{t.uninstalledOk}</span>;
  }
  return (
    <>
      <span className="inline-flex flex-col gap-1">
        <button
          onClick={() => setConfirming(true)}
          disabled={busy}
          className="lab border border-ink px-3 py-2 text-warm transition-colors duration-150 hover:border-verm hover:text-verm"
        >
          {busy ? t.uninstalling : t.uninstall}
        </button>
        {error && <span className="lab text-verm">{error}</span>}
      </span>
      {confirming && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 p-6" role="presentation">
          <div
            role="dialog"
            aria-modal="true"
            aria-label={fmt(t.uniTitle, { F: face.family })}
            className="max-w-[52ch] border border-ink bg-paper p-8"
          >
            <p className="lab text-verm">{t.uniLab}</p>
            <p className="disp mt-3 text-[26px] leading-tight">{fmt(t.uniTitle, { F: face.family })}</p>
            <p className="lab mt-3 text-warm">{fmt(t.uniBody, { W: t.scopeUser })}</p>
            <div className="mt-6 flex items-center gap-3">
              <button
                onClick={() => setConfirming(false)}
                className="lab border border-ink px-4 py-2.5 transition-colors duration-150 hover:bg-ink hover:text-paper"
              >
                {t.keep}
              </button>
              <button
                onClick={uni}
                className="lab border border-verm px-4 py-2.5 text-verm transition-colors duration-150 hover:bg-verm hover:text-paper"
              >
                {t.uniConfirm}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

/** M4 retrieval action: download + validate every weight of the family and
    cache it locally. Plain-browser dev has no download path, so this only
    renders in the Tauri runtime. Alt+C triggers it for the selected face. */
function CacheButton({ face }: { face: Face }) {
  const t = useI18n();
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const cache = async () => {
    setBusy(true);
    setError(null);
    try {
      const meta = await downloadFont(face.id);
      setDone(
        `${meta.fileCount} ${meta.fileCount === 1 ? t.filesOne : t.filesMany} · ${(meta.totalSize / 1024).toFixed(0)} KB`
      );
      // Let the confirmation register before the face flips to "library".
      await new Promise((r) => window.setTimeout(r, 900));
      await refreshFaces();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  // Keyboard path (Alt+C from App): keep the handler current without
  // re-subscribing on every render.
  const cacheRef = useRef(cache);
  cacheRef.current = cache;
  useEffect(() => {
    const onRequest = (e: Event) => {
      if ((e as CustomEvent<string>).detail === face.id) void cacheRef.current();
    };
    window.addEventListener("typecase:cache", onRequest);
    return () => window.removeEventListener("typecase:cache", onRequest);
  }, [face.id]);

  if (done) {
    return (
      <span className="lab border border-verm px-3 py-2 text-verm">
        {t.cachedOk} {done}
      </span>
    );
  }
  return (
    <span className="inline-flex flex-col gap-1">
      <button
        onClick={cache}
        disabled={busy}
        title={`${t.cache} (Alt+C)`}
        className={`lab border px-3 py-2 transition-colors duration-150 ${
          busy
            ? "border-warm text-warm"
            : "border-verm text-verm hover:bg-verm hover:text-paper"
        }`}
      >
        {busy ? t.caching : t.cache}
      </button>
      {error && <span className="lab text-verm">{error}</span>}
    </span>
  );
}

export default function Sheet({
  face,
  preset,
  onPreset,
  onPair,
}: {
  face: Face;
  preset: Preset;
  onPreset: (id: string) => void;
  onPair: (family: string) => void;
}) {
  const t = useI18n();
  const ps = t.presets[preset.id];
  const pair = allFaces().find((f) => f.family === face.pairsWith);
  const [size, setSize] = useState(preset.size);
  const [weight, setWeight] = useState(preset.weight);
  const [track, setTrack] = useState(preset.track);
  const [lead, setLead] = useState(preset.lead);
  const [typed, setTyped] = useState("");
  const [revision, setRevision] = useState(0);
  const [status, setStatus] = useState<"setting" | "ok" | "error">("setting");

  const lineRef = useRef<HTMLDivElement>(null);
  const textRef = useRef(ps.display);

  useEffect(() => {
    setSize(preset.size);
    setWeight(preset.weight);
    setTrack(preset.track);
    setLead(preset.lead);
    textRef.current = ps.display;
    setTyped("");
    setRevision((r) => r + 1);
  }, [preset, ps]);

  useEffect(() => {
    const lo = Math.min(...face.weights);
    const hi = Math.max(...face.weights);
    // keep the requested weight inside what this family actually ships
    setWeight((w) => Math.min(Math.max(w, lo), hi));
    setStatus("setting");
    setRevision((r) => r + 1);
    loadFace(face).then((r) => setStatus(r === "ok" ? "ok" : "error"));
  }, [face]);

  useLayoutEffect(() => {
    const el = lineRef.current;
    if (el && el.textContent !== textRef.current) el.textContent = textRef.current;
  }, [revision]);

  const sample = typed.trim() ? typed.trim() : ps.display;
  const wMin = Math.min(...face.weights);
  const wMax = Math.max(...face.weights);

  const statusLabel =
    status === "ok"
      ? face.local
        ? t.statusOkLocal
        : t.statusOkWeb
      : status === "setting"
        ? t.statusFetching
        : t.statusOffline;

  return (
    <section className="min-w-0">
      {/* ————— composing stick ————— */}
      <div className="sticky top-0 z-20 -mx-5 mb-8 bg-paper/95 px-5 pt-5 pb-3 backdrop-blur-[2px] lg:-mx-10 lg:px-10">
        <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 border-b border-ink pb-2">
          <span className="lab text-verm">{t.nowSetting}</span>
          <span className="disp text-[22px] leading-none">{face.family}</span>
          <span className="lab text-warm">
            {face.category} · {face.designer} · {face.year} · {face.styles} {t.styles} · SIL OFL 1.1
          </span>
          <span className="lab ml-auto flex items-center gap-4 text-warm">
            {statusLabel}
            <a href="#index" className="text-verm underline underline-offset-4 lg:hidden">
              {t.changeFace}
            </a>
          </span>
        </div>

        <div className="mt-3 grid grid-cols-2 gap-x-8 gap-y-3 md:grid-cols-4">
          <label className="block">
            <span className="lab flex items-baseline justify-between text-warm">
              {t.size} <b className="num text-verm">{size}px</b>
            </span>
            <input type="range" min={12} max={200} step={1} value={size} onChange={(e) => setSize(+e.target.value)} />
          </label>
          <label className="block">
            <span className="lab flex items-baseline justify-between text-warm">
              {t.weight} <b className="num text-verm">{wMax === wMin ? t.single : weight}</b>
            </span>
            <input
              type="range"
              min={wMin}
              max={wMax}
              step={100}
              value={weight}
              disabled={wMax === wMin}
              onChange={(e) => setWeight(+e.target.value)}
            />
          </label>
          <label className="block">
            <span className="lab flex items-baseline justify-between text-warm">
              {t.tracking} <b className="num text-verm">{num(track, 3)}em</b>
            </span>
            <input type="range" min={-0.05} max={0.2} step={0.005} value={track} onChange={(e) => setTrack(+e.target.value)} />
          </label>
          <label className="block">
            <span className="lab flex items-baseline justify-between text-warm">
              {t.leading} <b className="num text-verm">{num(lead, 2)}</b>
            </span>
            <input type="range" min={0.8} max={2} step={0.01} value={lead} onChange={(e) => setLead(+e.target.value)} />
          </label>
        </div>

        <div className="mt-3 flex flex-wrap items-center gap-x-5 gap-y-1">
          <span className="lab text-warm">{t.setFor}</span>
          {PRESETS.map((p0) => {
            const id = p0.id;
            const on = preset.id === id;
            return (
              <button
                key={id}
                onClick={() => onPreset(id)}
                className={`lab border-b transition-colors duration-150 ${
                  on
                    ? "border-verm text-ink"
                    : "border-transparent text-warm hover:border-rule hover:text-ink"
                }`}
              >
                {t.presets[id].label}
              </button>
            );
          })}
        </div>
        <p className="lab mt-2 text-warm">
          {ps.use} — <span className="text-verm">{ps.advice}</span>
        </p>
      </div>

      {/* ————— the specimen sheet ————— */}
      <div key={revision} className="setline">
        <div
          ref={lineRef}
          contentEditable
          suppressContentEditableWarning
          spellCheck={false}
          role="textbox"
          aria-label={fmt(t.textBlock, { L: ps.label })}
          data-ph={t.typeHere}
          dir="auto"
          onInput={(e) => {
            textRef.current = (e.target as HTMLElement).textContent ?? "";
            setTyped(textRef.current);
          }}
          className="min-h-[0.6em] break-words hyphens-none"
          style={{
            fontFamily: `"${face.family}", "Bodoni Moda", Georgia, serif`,
            fontSize: `${size}px`,
            fontWeight: weight,
            letterSpacing: `${track}em`,
            lineHeight: lead,
          }}
        />
      </div>

      <div className="mt-3 flex items-end justify-between gap-6 border-b border-ink pb-2">
        <Gauge size={size} />
        <span className="lab num shrink-0 text-verm">
          {size} pt · {weight} · {ps.label.toUpperCase()}
        </span>
      </div>

      {/* ————— waterfall ————— */}
      <div key={`wf-${face.id}`} className="mt-8 border-t border-rule pt-4">
        <h3 className="lab text-warm">{t.waterfall}</h3>
        <div className="mt-3 space-y-1">
          {WATERFALL.map((s, i) => (
            <div
              key={s}
              className="setline flex items-baseline gap-4 border-b border-rule pb-1"
              style={{ animationDelay: `${i * 22}ms` }}
            >
              <span className="lab num w-8 shrink-0 text-right text-verm">{s}</span>
              <span
                dir="auto"
                className="min-w-0 flex-1 truncate"
                style={{
                  fontFamily: `"${face.family}", "Bodoni Moda", Georgia, serif`,
                  fontSize: `${s}px`,
                  fontWeight: weight,
                  lineHeight: 1.25,
                }}
              >
                {sample}
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* ————— body block, set to a 62-character measure ————— */}
      <div className="mt-10 border-t border-ink pt-5">
        <h3 className="lab text-warm">{fmt(t.textBlock, { L: ps.label })}</h3>
        <div
          key={`body-${face.id}`}
          className="setline mt-4 gap-10 md:columns-2"
          style={{
            fontFamily: `"${face.family}", "Bodoni Moda", Georgia, serif`,
            fontSize: "17.5px",
            fontWeight: 400,
            lineHeight: 1.6,
          }}
        >
          {ps.body.map((para, i) => (
            <p key={i} className="mb-4 max-w-[62ch] break-inside-avoid">
              {para}
            </p>
          ))}
        </div>
      </div>

      {/* ————— glyph case ————— */}
      <div className="mt-10 overflow-hidden border-y border-rule py-4">
        <span
          className="block whitespace-nowrap"
          style={{
            fontFamily: `"${face.family}", "Bodoni Moda", Georgia, serif`,
            fontSize: "34px",
            fontWeight: 500,
            letterSpacing: "0.02em",
          }}
        >
          ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz 0123456789 &amp;@#$€£%§¶ ffi ft 1lI0O
        </span>
      </div>

      {/* ————— specimen note + pairing ————— */}
      <div className="mt-10 grid gap-8 border-t border-ink pt-5 md:grid-cols-2">
        <div>
          <h3 className="lab text-warm">{t.specimenNote}</h3>
          {face.note ? (
            <p className="disp mt-2 max-w-[38ch] text-[24px] leading-[1.25] italic">{face.note}</p>
          ) : (
            <p className="lab mt-3 text-warm">{t.noNote}</p>
          )}
        </div>
        <div>
          <h3 className="lab text-warm">{t.setsWellWith}</h3>
          {pair ? (
            <button
              onClick={() => onPair(pair.family)}
              className="disp mt-2 block border-b-2 border-verm pb-0.5 text-left text-[24px] leading-tight transition-transform duration-150 hover:translate-x-1"
            >
              {pair.family} →
            </button>
          ) : (
            <p className="disp mt-2 text-[24px] leading-tight">{face.pairsWith || "—"}</p>
          )}
          <p className="lab mt-3 max-w-[34ch] text-warm">{t.pairsExpl}</p>
        </div>
      </div>

      {/* ————— retrieval ————— */}
      <div className="mt-8 flex flex-wrap items-center gap-2">
        {face.phase === "online" && <CacheButton face={face} />}
        {face.phase === "library" && (
          <>
            <InstallButtons face={face} />
            <DeleteButton face={face} />
          </>
        )}
        {face.phase === "installed" && <UninstallButton face={face} />}
        <CopyBtn label={t.copyCss} doneLabel={t.copied} getText={() => cssSnippet(face.family, face.weights)} />
        <CopyBtn label={t.copyFamily} doneLabel={t.copied} getText={() => face.family} />
        <a
          href={specimenUrl(face.family)}
          target="_blank"
          rel="noreferrer"
          className="lab border border-ink px-3 py-2 transition-colors duration-150 hover:bg-ink hover:text-paper"
        >
          {t.gfPage}
        </a>
      </div>
      <p className="lab mt-2 max-w-[62ch] text-warm">
        {face.phase === "online"
          ? t.capOnline
          : face.phase === "library"
            ? t.capCached
            : t.capSet}
      </p>
    </section>
  );
}
