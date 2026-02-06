import { useDeferredValue, useEffect, useMemo, useState } from "react";
import { X } from "lucide-react";
import { DetailModal } from "./components/DetailModal";
import { Hero } from "./components/Hero";
import { LibraryGrid } from "./components/LibraryGrid";
import { MediaRow } from "./components/MediaRow";
import { AppTab, Navbar } from "./components/Navbar";
import { SettingsPanel } from "./components/SettingsPanel";
import { SeriesWallItem } from "./lib/types";
import { useStore } from "./store/useStore";

function byRecent(a: SeriesWallItem, b: SeriesWallItem) {
  return b.updated_at - a.updated_at;
}

function isMovieLike(item: SeriesWallItem) {
  const text = `${item.title} ${item.subtitle}`.toLowerCase();
  return item.total_episode_count <= 1 || /剧场版|movie|映画|ova|oad/.test(text);
}

function App() {
  const initialize = useStore((state) => state.initialize);
  const library = useStore((state) => state.library);
  const isLoading = useStore((state) => state.isLoading);
  const errorMessage = useStore((state) => state.errorMessage);
  const clearError = useStore((state) => state.clearError);
  const searchQuery = useStore((state) => state.searchQuery);
  const deferredSearchQuery = useDeferredValue(searchQuery);
  const [activeTab, setActiveTab] = useState<AppTab>("home");

  useEffect(() => {
    void initialize();
  }, [initialize]);

  const normalizedQuery = deferredSearchQuery.trim().toLowerCase();
  const filtered = useMemo(() => {
    if (!normalizedQuery) return library;
    return library.filter((item) => `${item.title} ${item.subtitle}`.toLowerCase().includes(normalizedQuery));
  }, [library, normalizedQuery]);

  const recentlyUpdated = useMemo(() => [...filtered].sort(byRecent), [filtered]);
  const continuing = useMemo(
    () => [...filtered].filter((item) => item.missing_count > 0 && item.total_episode_count > 1).sort(byRecent),
    [filtered]
  );
  const complete = useMemo(
    () => [...filtered].filter((item) => item.missing_count === 0 && item.episode_count > 0).sort(byRecent),
    [filtered]
  );
  const movies = useMemo(() => [...filtered].filter(isMovieLike).sort(byRecent), [filtered]);

  return (
    <div className="min-h-screen bg-[#141414] text-white selection:bg-netflix-red selection:text-white">
      <Navbar activeTab={activeTab} onTabChange={setActiveTab} />

      <main className="pb-20">
        {isLoading ? (
          <div className="grid h-[70vh] place-items-center pt-24">
            <div className="flex flex-col items-center gap-4">
              <div className="h-10 w-10 animate-spin rounded-full border-2 border-zinc-600 border-t-netflix-red" />
              <p className="text-sm text-zinc-400">加载媒体库...</p>
            </div>
          </div>
        ) : (
          <>
            {errorMessage ? (
              <div className="mx-4 mb-6 mt-20 flex items-start justify-between gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200 backdrop-blur-sm md:mx-10">
                <span>{errorMessage}</span>
                <button onClick={clearError} className="rounded-full p-0.5 text-red-200/80 transition hover:bg-red-500/20 hover:text-red-100" aria-label="dismiss error">
                  <X className="h-4 w-4" />
                </button>
              </div>
            ) : null}

            {activeTab === "home" ? (
              <>
                {/* Hero section - only show when not searching */}
                {!normalizedQuery && library.length > 0 && <Hero />}

                <div className={`relative z-20 space-y-10 ${normalizedQuery ? "mt-24" : "-mt-16 md:-mt-12"}`}>
                  {filtered.length === 0 ? (
                    <div className="mx-4 rounded-xl border border-white/10 bg-black/35 p-8 text-center text-zinc-300 md:mx-10">
                      没有匹配结果。
                    </div>
                  ) : (
                    <>
                      <MediaRow
                        title={normalizedQuery ? "搜索结果" : "最近更新"}
                        subtitle={normalizedQuery ? `${filtered.length} 条匹配` : undefined}
                        items={normalizedQuery ? filtered : recentlyUpdated}
                      />
                      {continuing.length > 0 && (
                        <MediaRow title="连载中" subtitle="本地仍有剧集缺失" items={continuing} />
                      )}
                      {complete.length > 0 && (
                        <MediaRow title="已完整" subtitle="主线剧集本地已齐全" items={complete} />
                      )}
                      {movies.length > 0 && (
                        <MediaRow title="剧场版 / OVA" subtitle="单集或电影向条目" items={movies} />
                      )}
                    </>
                  )}
                </div>
              </>
            ) : null}

            {activeTab === "library" ? (
              <div className="pt-24">
                <LibraryGrid items={filtered} />
              </div>
            ) : null}
            {activeTab === "settings" ? (
              <div className="pt-24">
                <SettingsPanel />
              </div>
            ) : null}
          </>
        )}
      </main>

      <DetailModal />
    </div>
  );
}

export default App;
