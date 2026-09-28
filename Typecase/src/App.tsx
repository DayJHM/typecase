import { useEffect, useMemo, useState } from "react";
import Rail from "./components/Rail";
import Sheet from "./components/Sheet";
import InstalledView from "./components/InstalledView";
import Shell, { type View } from "./components/Shell";
import { useFaces } from "./hooks/useFaces";
import { PHASE_ORDER, anyRemoved } from "./data/face";
import { PRESETS } from "./data/presets";
import type { Face } from "./data/types";
import { applyCatalogRefresh, refreshCatalog, type CatalogDiffInfo } from "./data/ipc";
import { fmt, useI18n } from "./i18n";

export default function App() {
  const { faces, loaded, error, reload } = useFaces();
  const t = useI18n();
  const [view, setView] = useState<View>("discover");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [cat, setCat] = useState<Face["category"] | "All">("All");
  const [presetId, setPresetId] = useState("essay");

  /* M8 §15: manual, user-controlled catalog refresh. The diff parks on the
     backend; the notice below is dismissible — nothing applies without the
     explicit “Update catalog” click. */
  const [refreshing, setRefreshing] = useState(false);
  const [refreshed, setRefreshed] = useState<"ok" | null>(null);
  const [notice, setNotice] = useState<CatalogDiffInfo | null>(null);
  const [noticeClosed, setNoticeClosed] = useState(false);
  const [applyBusy, setApplyBusy] = useState(false);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const removedCount = anyRemoved();

  const preset = PRESETS.find((p) => p.id === presetId) ?? PRESETS[0];

  // default selection: first family once loaded
  useEffect(() => {
    if (loaded && faces.length && !selectedId) setSelectedId(faces[0].id);
  }, [loaded, faces, selectedId]);

  const selected = useMemo(
    () => faces.find((f) => f.id === selectedId) ?? null,
    [faces, selectedId]
  );

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return faces
      .filter((f) => {
        if (view !== "discover" && f.phase !== view) return false;
        if (cat !== "All" && f.category !== cat) return false;
        if (!q) return true;
        return (
          f.family.toLowerCase().includes(q) ||
          f.designer.toLowerCase().includes(q) ||
          f.note.toLowerCase().includes(q) ||
          f.category.toLowerCase().includes(q)
        );
      })
      .sort(
        (a, b) =>
          (PHASE_ORDER[a.phase] - PHASE_ORDER[b.phase]) * (view === "discover" ? 1 : 0) ||
          a.family.localeCompare(b.family)
      );
  }, [faces, view, cat, query]);

  const counts = useMemo(
    () => ({
      online: faces.filter((f) => f.phase === "online").length,
      library: faces.filter((f) => f.phase === "library").length,
      installed: faces.filter((f) => f.phase === "installed").length,
    }),
    [faces]
  );

  const openFace = (f: Face) => {
    setSelectedId(f.id);
    if (view !== "discover" && f.phase !== view) setView(f.phase);
  };

  const runRefresh = async () => {
    setRefreshing(true);
    setRefreshed(null);
    setRefreshError(null);
    setNoticeClosed(false);
    try {
      const diff = await refreshCatalog();
      if (diff.unchanged) {
        setRefreshed("ok");
      } else {
        setNotice(diff);
      }
    } catch (e) {
      setRefreshError(e instanceof Error ? e.message : String(e));
    } finally {
      setRefreshing(false);
    }
  };

  const runApply = async () => {
    setApplyBusy(true);
    try {
      await applyCatalogRefresh();
      setNotice(null);
      await reload();
    } catch (e) {
      setRefreshError(e instanceof Error ? e.message : String(e));
    } finally {
      setApplyBusy(false);
    }
  };

  const emptyState =
    query.trim() === "" && cat === "All"
      ? view === "library"
        ? { title: t.libEmptyTitle, hint: t.libEmptyHint }
        : view === "installed"
          ? { title: t.instEmptyTitle, hint: t.instEmptyHint }
          : undefined
      : undefined;

  if (error) {
    return (
      <div className="grain flex min-h-screen items-center justify-center">
        <div className="max-w-[52ch] border border-ink p-8">
          <p className="lab text-verm">{t.errLab}</p>
          <p className="disp mt-3 text-[26px] leading-tight">{t.errTitle}</p>
          <p className="lab mt-3 text-warm">{error}</p>
          <button
            onClick={() => location.reload()}
            className="lab mt-6 border border-ink px-4 py-2.5 transition-colors duration-150 hover:bg-ink hover:text-paper"
          >
            {t.retry}
          </button>
        </div>
      </div>
    );
  }

  return (
    <Shell view={view} onView={setView} counts={counts}>
      {/* ——— M8 catalog actions: manual refresh (§15) + removed summary ——— */}
      {loaded && (
        <div className="flex flex-wrap items-center gap-x-5 gap-y-1 border-b border-rule py-2">
          {removedCount > 0 && (
            <span className="lab text-verm" title={t.removedNote}>
              ✝ {removedCount} {t.removedBadge}
            </span>
          )}
          <button
            onClick={runRefresh}
            disabled={refreshing}
            className="lab ml-auto text-warm transition-colors duration-150 hover:text-verm"
          >
            {refreshing ? t.refreshing : t.refresh}
          </button>
          {refreshed === "ok" && <span className="lab text-verm">{t.refreshUpToDate}</span>}
          {refreshError && (
            <span className="lab text-verm" title={refreshError}>
              {t.refreshFailed}
            </span>
          )}
        </div>
      )}
      {notice && !noticeClosed && (
        <div className="mt-3 border border-ink p-4">
          <p className="lab text-verm">
            {fmt(t.refreshAvailable, { A: notice.added, C: notice.changed, R: notice.removed })}
          </p>
          {notice.addedNames.length > 0 && (
            <p className="lab mt-1 text-warm">+ {notice.addedNames.join(", ")}</p>
          )}
          {notice.removedNames.length > 0 && (
            <p className="lab mt-1 text-warm">− {notice.removedNames.join(", ")}</p>
          )}
          <div className="mt-3 flex items-center gap-3">
            <button
              onClick={runApply}
              disabled={applyBusy}
              className="lab border border-verm px-4 py-2 text-verm transition-colors duration-150 hover:bg-verm hover:text-paper"
            >
              {applyBusy ? t.refreshing : t.refreshApply}
            </button>
            <button
              onClick={() => setNoticeClosed(true)}
              className="lab border border-ink px-4 py-2 text-warm transition-colors duration-150 hover:bg-ink hover:text-paper"
            >
              {t.refreshDismiss}
            </button>
          </div>
        </div>
      )}
      {!loaded ? (
        <div className="pt-24 text-center">
          <p className="disp text-[34px] leading-tight">{t.opening}</p>
          <p className="lab mt-3 text-warm">{t.loadingCatalog}</p>
        </div>
      ) : (
        view === "installed" ? (
          <InstalledView
            onOpenManaged={(id) => {
              setSelectedId(id);
              setView("discover");
            }}
          />
        ) : (
        <div className="grid grid-cols-1 gap-x-10 pt-5 lg:grid-cols-[320px_minmax(0,1fr)]">
          <aside id="index" className="order-2 min-h-0 border-t border-ink pt-4 lg:order-1 lg:sticky lg:top-0 lg:h-[calc(100vh-140px)] lg:border-t-0 lg:border-r lg:border-rule lg:pr-6">
            <Rail
              faces={visible}
              total={faces.length}
              selected={selected}
              onSelect={openFace}
              query={query}
              setQuery={setQuery}
              cat={cat}
              setCat={setCat}
              empty={emptyState}
            />
          </aside>

          <div className="order-1 min-w-0 lg:order-2">
            {selected ? (
              <Sheet
                face={selected}
                preset={preset}
                onPreset={setPresetId}
                onPair={(family) => {
                  const f = faces.find((x) => x.family === family);
                  if (f) openFace(f);
                }}
              />
            ) : (
              <p className="lab pt-10 text-warm">{t.noFaceSelected}</p>
            )}
          </div>
        </div>
        )
      )}
    </Shell>
  );
}
