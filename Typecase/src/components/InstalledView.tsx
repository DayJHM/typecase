import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  cacheExternalFont,
  deleteCachedFamily,
  fetchInstalledFonts,
  removeExternalFont,
  type RegisteredFont,
} from "../data/ipc";
import { fmt, useI18n } from "../i18n";
import { forgetLocalFace, refreshCachedFontSources } from "../lib/fonts";

/** One explicit confirmation shape for the two destructive operations on an
    external font: removing it from Windows (§25) and discarding Typecase's own
    copy of it. Focus lands on the safe choice; Escape cancels. `extra` carries
    the §25 step-3 offer, so a copy can be kept *before* the confirmation is
    given rather than after. */
function ConfirmDialog({
  lab,
  title,
  body,
  confirm,
  onConfirm,
  onCancel,
  extra,
}: {
  lab: string;
  title: string;
  body: string;
  confirm: string;
  onConfirm: () => void;
  onCancel: () => void;
  extra?: ReactNode;
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
        aria-label={title}
        className="max-w-[54ch] border border-ink bg-paper p-8"
      >
        <p className="lab text-verm">{lab}</p>
        <p className="disp mt-3 text-[26px] leading-tight">{title}</p>
        <p className="lab mt-3 text-warm">{body}</p>
        {extra}
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
            {confirm}
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
  const [dropping, setDropping] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [saving, setSaving] = useState(false);
  const [removed, setRemoved] = useState(false);
  const [cachedId, setCachedId] = useState<string | null>(font.cachedId);
  const [error, setError] = useState<string | null>(null);

  const remove = async () => {
    setRemoving(true);
    setError(null);
    try {
      await removeExternalFont(font.valueName, font.filePath, font.scope);
      setConfirming(false);
      setRemoved(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRemoving(false);
    }
  };

  /* §25 step 3 / §43 flow 3: keep Typecase's own copy before anything is
     removed. The backend verifies the font is registered and writes the copy
     itself; the row only learns the new cache id. */
  const keepCopy = async () => {
    setSaving(true);
    setError(null);
    try {
      const out = await cacheExternalFont(font.valueName, font.filePath, font.scope);
      setCachedId(out.id);
      // The copy is servable through the same font:// path as any cache entry.
      void refreshCachedFontSources();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  /* §24: deleting the copy is its own explicit operation, and it only ever
     deletes Typecase's copy — the font in Windows is untouched. */
  const discardCopy = async () => {
    if (!cachedId) return;
    setSaving(true);
    setError(null);
    try {
      await deleteCachedFamily(cachedId);
      forgetLocalFace(font.family);
      void refreshCachedFontSources();
      setCachedId(null);
      setDropping(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
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
          <span className="inline-flex shrink-0 items-center gap-1.5">
            {cachedId ? (
              <>
                <span
                  className="lab shrink-0 border border-rule px-1.5 py-0.5 text-warm"
                  title={t.extCacheLab}
                >
                  {t.extCached}
                </span>
                <button
                  onClick={() => setDropping(true)}
                  disabled={saving}
                  title={t.extCacheLab}
                  className="lab border border-ink px-2 py-0.5 text-warm transition-colors duration-150 hover:border-verm hover:text-verm"
                >
                  {t.extCacheDrop}
                </button>
              </>
            ) : (
              <button
                onClick={keepCopy}
                disabled={saving}
                title={t.extCacheOffer}
                className="lab border border-ink px-2 py-0.5 text-warm transition-colors duration-150 hover:border-verm hover:text-verm"
              >
                {saving ? t.extCaching : t.extCache}
              </button>
            )}
            <button
              onClick={() => setConfirming(true)}
              disabled={removing}
              className="lab border border-ink px-2 py-0.5 text-warm transition-colors duration-150 hover:border-verm hover:text-verm"
            >
              {removing ? t.extRemoving : t.extRemove}
            </button>
          </span>
          {error && <span className="lab text-verm">{error}</span>}
        </span>
      )}
      {confirming && (
        <ConfirmDialog
          lab={t.extLab}
          title={fmt(t.extTitle, { F: font.family })}
          body={fmt(t.extBody, { S: font.scope === "user" ? t.scopeUser : t.scopeSystem })}
          confirm={t.extConfirm}
          onConfirm={remove}
          onCancel={() => setConfirming(false)}
          extra={
            <div className="mt-4 border-t border-rule pt-3">
              <p className="lab text-warm">{t.extCacheOffer}</p>
              {cachedId ? (
                <p className="lab mt-2 text-verm">{t.extCached}</p>
              ) : (
                <button
                  onClick={keepCopy}
                  disabled={saving}
                  className="lab mt-2 border border-ink px-3 py-1.5 text-warm transition-colors duration-150 hover:border-verm hover:text-verm"
                >
                  {saving ? t.extCaching : t.extCache}
                </button>
              )}
            </div>
          }
        />
      )}
      {dropping && (
        <ConfirmDialog
          lab={t.extCacheLab}
          title={fmt(t.extCacheTitle, { F: font.family })}
          body={t.extCacheBody}
          confirm={t.extCacheConfirm}
          onConfirm={discardCopy}
          onCancel={() => setDropping(false)}
        />
      )}
    </div>
  );
}

/** M7 Installed view: the real Windows font environment (both registry
    scopes), badged by ownership. Managed rows navigate to their face; external
    rows can be copied into Typecase's cache (§25 step 3) or removed. */
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
