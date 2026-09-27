import { useEffect, useRef, useState } from "react";
import { fetchInstalledFonts, removeExternalFont, type RegisteredFont } from "../data/ipc";
import { fmt, useI18n } from "../i18n";

/** §25: explicit warning for removing an EXTERNAL font — other apps and
    documents may depend on it. Focus lands on Keep; Escape cancels. */
function ExternalRemoveConfirm({
  font,
  onConfirm,
  onCancel,
}: {
  font: RegisteredFont;
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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-ink/40 p-6" role="presentation">
      <div
        role="dialog"
        aria-modal="true"
        aria-label={fmt(t.extTitle, { F: font.family })}
        className="max-w-[54ch] border border-ink bg-paper p-8"
      >
        <p className="lab text-verm">{t.extLab}</p>
        <p className="disp mt-3 text-[26px] leading-tight">{fmt(t.extTitle, { F: font.family })}</p>
        <p className="lab mt-3 text-warm">{fmt(t.extBody, { S: font.scope === "user" ? t.scopeUser : t.scopeSystem })}</p>
        <p className="lab mt-2 text-warm">{t.extCacheOffer}</p>
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
            {t.extConfirm}
          </button>
        </div>
      </div>
    </div>
  );
}

function FontRow({
  font,
  onOpenManaged,
}: {
  font: RegisteredFont;
  onOpenManaged: (id: string) => void;
}) {
  const t = useI18n();
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [removed, setRemoved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      await removeExternalFont(font.valueName, font.filePath, font.scope);
      setConfirming(false);
      setRemoved(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const managed = font.ownership === "managed";
  return (
    <div className="flex items-baseline gap-3 border-b border-rule py-2.5">
      <span className="disp min-w-0 flex-1 truncate text-[19px] leading-tight">
        {font.family}
        <span className="lab ml-2 text-warm">{font.style}</span>
      </span>
      <span
        className={`lab shrink-0 border px-1.5 py-0.5 ${
          managed ? "border-verm text-verm" : "border-rule text-warm"
        }`}
        title={managed ? undefined : font.filePath}
      >
        {managed ? t.ownManaged : t.ownExternal}
      </span>
      {managed && font.id ? (
        <button
          onClick={() => onOpenManaged(font.id!)}
          className="lab shrink-0 border border-ink px-2 py-0.5 text-warm transition-colors duration-150 hover:border-verm hover:text-verm"
        >
          {t.discover} →
        </button>
      ) : removed ? (
        <span className="lab shrink-0 border border-ink px-2 py-0.5 text-warm">{t.extRemoved}</span>
      ) : (
        <span className="inline-flex shrink-0 flex-col items-end gap-1">
          <button
            onClick={() => setConfirming(true)}
            disabled={busy}
            className="lab border border-ink px-2 py-0.5 text-warm transition-colors duration-150 hover:border-verm hover:text-verm"
          >
            {busy ? t.extRemoving : t.extRemove}
          </button>
          {error && <span className="lab text-verm">{error}</span>}
        </span>
      )}
      {confirming && (
        <ExternalRemoveConfirm font={font} onConfirm={remove} onCancel={() => setConfirming(false)} />
      )}
    </div>
  );
}

/** M7 Installed view: the real Windows font environment (both registry
    scopes), badged by ownership. Managed rows navigate to their face. */
export default function InstalledView({ onOpenManaged }: { onOpenManaged: (id: string) => void }) {
  const t = useI18n();
  const [fonts, setFonts] = useState<RegisteredFont[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchInstalledFonts()
      .then(setFonts)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, []);

  if (error) {
    return <p className="lab pt-10 text-verm">{error}</p>;
  }
  if (!fonts) {
    return <p className="lab pt-10 text-warm">{t.statusFetching}</p>;
  }
  if (fonts.length === 0) {
    return (
      <div className="pt-10">
        <p className="disp text-[28px] leading-tight">{t.instEmptyTitle}</p>
        <p className="lab mt-2 text-warm">{t.instEmptyHint}</p>
      </div>
    );
  }
  const managed = fonts.filter((f) => f.ownership === "managed").length;
  return (
    <div className="pt-2">
      <p className="lab text-warm">
        {t.installedViewCap} · {fonts.length} {fonts.length === 1 ? t.instTypeFace : t.instTypeFaces} ·{" "}
        {managed} {t.ownManaged} · {fonts.length - managed} {t.ownExternal}
      </p>
      <div className="mt-3">
        {fonts.map((f) => (
          <FontRow key={`${f.scope}:${f.valueName}`} font={f} onOpenManaged={onOpenManaged} />
        ))}
      </div>
    </div>
  );
}
