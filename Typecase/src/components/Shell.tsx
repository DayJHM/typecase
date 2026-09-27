import type { Phase } from "../data/types";
import { getLang, LANGS, setLang, useI18n } from "../i18n";
import { useDark, setDark } from "../theme";

export type View = "discover" | Phase;

/** Masthead + view tabs + theme toggle, in the foundry's visual language. */
export default function Shell({
  view,
  onView,
  counts,
  children,
}: {
  view: View;
  onView: (v: View) => void;
  counts: { online: number; library: number; installed: number };
  children: React.ReactNode;
}) {
  const t = useI18n();
  // fresh at render time; a language switch re-renders through useI18n's store
  const lang = getLang();
  const dark = useDark();

  const tabs: { id: View; label: string; n: number }[] = [
    { id: "discover", label: t.discover, n: counts.online + counts.library + counts.installed },
    { id: "library", label: t.library, n: counts.library },
    { id: "installed", label: t.installed, n: counts.installed },
  ];

  return (
    <div className="grain min-h-screen">
      {/* ——— masthead ——— */}
      <header className="mx-auto max-w-[1400px] px-5 pt-6 lg:px-10">
        <div className="flex flex-wrap items-baseline justify-between gap-x-6 gap-y-1 border-b border-ink pb-2">
          <span className="disp text-[26px] leading-none">
            Type<span className="text-verm">case</span>
          </span>
          <span className="lab text-warm">{t.tagline}</span>
          <span className="lab num flex items-center gap-4 text-verm">
            {t.os}
            <button
              onClick={() => setDark(!dark)}
              title={t.themeToggleTitle}
              aria-label={t.themeToggleTitle}
              className="lab border border-ink px-2 py-0.5 text-warm transition-colors duration-150 hover:border-verm hover:text-verm"
            >
              {dark ? `☀ ${t.toLight}` : `☾ ${t.toDark}`}
            </button>
            <span className="flex items-center border border-ink">
              {LANGS.map((l, i) => (
                <button
                  key={l.id}
                  onClick={() => setLang(l.id)}
                  aria-pressed={lang === l.id}
                  className={`lab px-2 py-0.5 transition-colors duration-150 ${
                    lang === l.id ? "bg-verm text-paper" : "text-warm hover:text-verm"
                  } ${i > 0 ? "border-l border-ink" : ""}`}
                >
                  {l.id.toUpperCase()}
                </button>
              ))}
            </span>
          </span>
        </div>

        {/* ——— view tabs ——— */}
        <nav className="flex flex-wrap items-baseline gap-x-8 gap-y-1 border-b border-rule py-3">
          {tabs.map((tab) => {
            const on = view === tab.id;
            return (
              <button
                key={tab.id}
                onClick={() => onView(tab.id)}
                aria-current={on ? "page" : undefined}
                className={`disp text-[22px] leading-tight transition-colors duration-150 ${
                  on ? "text-verm" : "text-ink hover:text-verm"
                }`}
              >
                {tab.label}
                <span className="lab num ml-2 align-super text-[10px] text-warm">{tab.n}</span>
              </button>
            );
          })}
          <span className="lab num ml-auto text-warm">
            {view === "discover" ? t.capDiscover : view === "library" ? t.capLibrary : t.capInstalled}
          </span>
        </nav>
      </header>

      <main className="mx-auto max-w-[1400px] px-5 pb-10 lg:px-10">{children}</main>

      {/* ——— colophon ——— */}
      <footer className="mt-16 border-t border-ink">
        <div className="mx-auto grid max-w-[1400px] gap-6 px-5 py-8 md:grid-cols-3 lg:px-10">
          <p className="lab text-warm">{t.colo1}</p>
          <p className="lab text-warm">{t.colo2}</p>
          <p className="lab text-verm">{t.colo3}</p>
        </div>
      </footer>
    </div>
  );
}
