import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Play, RefreshCcw, X } from "lucide-react";
import { useStore } from "../store/useStore";
import { api } from "../lib/api";
import { SeriesDetail, SeriesDetailEpisode } from "../lib/types";
import { resolveLocalFilePath } from "../lib/utils";
import { CoverImage } from "./CoverImage";

const fallbackCover =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='1200' height='680'%3E%3Crect width='100%25' height='100%25' fill='%23161616'/%3E%3Ctext x='50%25' y='50%25' fill='white' font-size='32' text-anchor='middle' dominant-baseline='middle'%3ENo Cover%3C/text%3E%3C/svg%3E";
const INITIAL_EPISODE_BATCH = 16;
const EPISODE_BATCH_STEP = 16;
const INITIAL_FILE_BUTTONS = 3;

function normalizeCodecLabel(raw: string): string {
  const normalized = raw.toUpperCase().replace(/\./g, "");
  if (normalized === "X265") return "HEVC";
  if (normalized === "X264") return "H264";
  return normalized;
}

function compactFileLabel(file: string, index: number): string {
  const quality = file.match(/\b(2160p|1440p|1080p|720p|480p)\b/i)?.[1]?.toUpperCase();
  const codecRaw = file.match(/\b(HEVC|AV1|X265|X264|H\.?265|H\.?264)\b/i)?.[1] ?? "";
  const codec = codecRaw ? normalizeCodecLabel(codecRaw) : "";
  const ext = file.split(".").pop()?.toUpperCase();
  const parts = [`Source ${index + 1}`, quality, codec, ext].filter(Boolean);
  return parts.join(" · ");
}

interface EpisodeCardProps {
  ep: SeriesDetailEpisode;
  root: string;
  onPlay: (path: string) => void;
}

const EpisodeCard = memo(({ ep, root, onPlay }: EpisodeCardProps) => {
  const epTitle = ep.name_cn || ep.name || `Episode ${ep.episode}`;
  const [showAllFiles, setShowAllFiles] = useState(false);
  const fileItems = useMemo(
    () =>
      ep.files.map((file, index) => ({
        key: `${ep.episode}-${file}`,
        label: compactFileLabel(file, index),
        absolutePath: resolveLocalFilePath(root, file),
      })),
    [ep.episode, ep.files, root]
  );
  const visibleFileItems = showAllFiles ? fileItems : fileItems.slice(0, INITIAL_FILE_BUTTONS);

  return (
    <div className="rounded-lg border border-white/10 bg-black/30 p-3">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-sm font-semibold text-white">
            EP {ep.episode} · {epTitle}
          </p>
          <p className="mt-1 text-xs text-zinc-400">{ep.name}</p>
        </div>
        <span
          className={`rounded border px-2 py-0.5 text-[10px] uppercase tracking-wider ${
            ep.files.length > 0
              ? "border-emerald-400/40 text-emerald-300"
              : "border-amber-400/40 text-amber-300"
          }`}
        >
          {ep.files.length > 0 ? "Ready" : "Missing"}
        </span>
      </div>

      <div className="mt-3 flex flex-wrap gap-2">
        {visibleFileItems.map((item) => (
          <button
            key={item.key}
            onClick={() => onPlay(item.absolutePath)}
            className="rounded border border-white/15 px-2.5 py-1.5 text-xs text-zinc-200 transition hover:border-netflix-red hover:text-white"
          >
            Play · {item.label}
          </button>
        ))}
        {!showAllFiles && fileItems.length > INITIAL_FILE_BUTTONS ? (
          <button
            onClick={() => setShowAllFiles(true)}
            className="rounded border border-white/20 px-2.5 py-1.5 text-xs text-zinc-300 transition hover:border-netflix-red hover:text-white"
          >
            还有 {fileItems.length - INITIAL_FILE_BUTTONS} 个源
          </button>
        ) : null}
      </div>
    </div>
  );
});

EpisodeCard.displayName = "EpisodeCard";

export const DetailModal = () => {
  const isDetailOpen = useStore((state) => state.isDetailOpen);
  const selectedSeriesId = useStore((state) => state.selectedSeriesId);
  const closeDetail = useStore((state) => state.closeDetail);
  const rematchSeries = useStore((state) => state.rematchSeries);

  const [detail, setDetail] = useState<SeriesDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [rematching, setRematching] = useState(false);
  const [visibleEpisodeCount, setVisibleEpisodeCount] = useState(0);
  const episodesScrollRef = useRef<HTMLElement | null>(null);
  const episodesLoadSentinelRef = useRef<HTMLDivElement | null>(null);
  const loadMoreLockRef = useRef(false);

  useEffect(() => {
    if (!isDetailOpen || !selectedSeriesId) {
      setDetail(null);
      setVisibleEpisodeCount(0);
      loadMoreLockRef.current = false;
      return;
    }
    let cancelled = false;
    setLoading(true);
    api
      .getSeriesDetail(selectedSeriesId)
      .then((res) => {
        if (cancelled) return;
        setDetail(res);
        const total = res?.episodes.length ?? 0;
        setVisibleEpisodeCount(Math.min(total, INITIAL_EPISODE_BATCH));
        loadMoreLockRef.current = false;
      })
      .catch(console.error)
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isDetailOpen, selectedSeriesId]);

  useEffect(() => {
    if (!isDetailOpen) return;

    const html = document.documentElement;
    const body = document.body;
    const prevHtmlOverflow = html.style.overflow;
    const prevBodyOverflow = body.style.overflow;
    const prevBodyPaddingRight = body.style.paddingRight;

    const scrollbarWidth = window.innerWidth - html.clientWidth;
    html.style.overflow = "hidden";
    body.style.overflow = "hidden";
    if (scrollbarWidth > 0) {
      body.style.paddingRight = `${scrollbarWidth}px`;
    }

    return () => {
      html.style.overflow = prevHtmlOverflow;
      body.style.overflow = prevBodyOverflow;
      body.style.paddingRight = prevBodyPaddingRight;
    };
  }, [isDetailOpen]);

  useEffect(() => {
    if (!isDetailOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeDetail();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [closeDetail, isDetailOpen]);

  const firstPlayable = useMemo(() => {
    if (!detail) return null;
    for (const ep of detail.episodes) {
      if (ep.files[0]) {
        return resolveLocalFilePath(detail.root, ep.files[0]);
      }
    }
    return null;
  }, [detail]);

  const play = useCallback(async (path: string) => {
    try {
      await api.playEpisode(path);
    } catch (error) {
      console.error(error);
    }
  }, []);
  const handlePlay = useCallback(
    (path: string) => {
      void play(path);
    },
    [play]
  );
  const handleRematch = useCallback(async () => {
    if (!selectedSeriesId || rematching) return;
    setRematching(true);
    try {
      await rematchSeries(selectedSeriesId);
      closeDetail();
    } finally {
      setRematching(false);
    }
  }, [closeDetail, rematchSeries, rematching, selectedSeriesId]);
  const visibleEpisodes = useMemo(
    () => (detail ? detail.episodes.slice(0, visibleEpisodeCount) : []),
    [detail, visibleEpisodeCount]
  );
  const hasMoreEpisodes = detail ? visibleEpisodeCount < detail.episodes.length : false;
  const loadMoreEpisodes = useCallback(() => {
    const total = detail?.episodes.length ?? 0;
    if (!total) return;
    setVisibleEpisodeCount((prev) => {
      if (prev >= total) return prev;
      return Math.min(total, prev + EPISODE_BATCH_STEP);
    });
  }, [detail]);
  const requestLoadMoreEpisodes = useCallback(() => {
    if (!hasMoreEpisodes || loadMoreLockRef.current) return;
    loadMoreLockRef.current = true;
    loadMoreEpisodes();
  }, [hasMoreEpisodes, loadMoreEpisodes]);

  useEffect(() => {
    loadMoreLockRef.current = false;
  }, [visibleEpisodeCount]);

  useEffect(() => {
    if (!isDetailOpen || !hasMoreEpisodes) return;
    const root = episodesScrollRef.current;
    const sentinel = episodesLoadSentinelRef.current;
    if (!root || !sentinel) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          requestLoadMoreEpisodes();
        }
      },
      {
        root,
        rootMargin: "0px 0px 360px 0px",
        threshold: 0.01,
      }
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasMoreEpisodes, isDetailOpen, requestLoadMoreEpisodes, visibleEpisodeCount]);

  return (
    <>
      {isDetailOpen ? (
        <div
          className="fixed inset-0 z-[70] flex items-center justify-center p-2 md:p-6"
          style={{ animation: "fadeIn 0.2s ease-out forwards" }}
        >
          <div
            className="absolute inset-0 bg-black/78"
            onClick={closeDetail}
            style={{ animation: "fadeIn 0.2s ease-out forwards" }}
          />

          <section
            className="relative z-10 flex max-h-[92vh] w-full max-w-5xl flex-col overflow-hidden overscroll-y-contain rounded-xl border border-white/10 bg-[#181818] shadow-[0_28px_90px_rgba(0,0,0,0.55)]"
            style={{ animation: "slideUp 0.3s cubic-bezier(0.16, 1, 0.3, 1) forwards" }}
          >
            <button
              onClick={closeDetail}
              className="absolute right-4 top-4 z-20 rounded-full border border-white/20 bg-black/50 p-2 text-zinc-200 transition hover:text-white"
            >
              <X className="h-4 w-4" />
            </button>

            {loading || !detail ? (
              <div className="grid h-80 place-items-center text-zinc-300">Loading...</div>
            ) : (
              <div className="grid min-h-0 flex-1 md:grid-cols-[minmax(280px,360px)_minmax(0,1fr)]">
                <aside className="overflow-y-auto border-b border-white/10 p-5 text-sm text-zinc-300 md:border-b-0 md:border-r md:p-7">
                  <div className="mx-auto w-full max-w-[240px] overflow-hidden rounded-xl border border-white/15 bg-black/40 shadow-lg">
                    <div className="aspect-[2/3] w-full">
                      <CoverImage
                        path={detail.cover_local_path}
                        alt={detail.title}
                        fallback={detail.cover_url || fallbackCover}
                        sourceMode="data-url"
                        className="h-full w-full object-cover"
                      />
                    </div>
                  </div>

                  <div className="mt-5 space-y-4">
                    <div>
                      <h3 className="text-2xl font-black text-white">{detail.title}</h3>
                      {detail.subtitle && detail.subtitle !== detail.title ? (
                        <p className="mt-1 text-sm text-zinc-400">{detail.subtitle}</p>
                      ) : null}
                    </div>

                    <div className="flex flex-wrap items-center gap-2 text-xs text-zinc-200">
                      <span className="rounded border border-white/25 px-2 py-0.5">{detail.air_date || "Unknown Air Date"}</span>
                      <span className="rounded border border-emerald-400/60 bg-emerald-500/20 px-2 py-0.5">
                        Score {detail.rating_score ?? "-"}
                      </span>
                      <span className="rounded border border-zinc-500 px-2 py-0.5">Episodes {detail.episodes.length}</span>
                      <span className="rounded border border-amber-400/60 bg-amber-500/20 px-2 py-0.5">
                        Missing {detail.missing_episodes.length}
                      </span>
                    </div>

                    <button
                      disabled={!firstPlayable}
                      onClick={() => firstPlayable && void play(firstPlayable)}
                      className="inline-flex w-full items-center justify-center gap-2 rounded bg-white px-5 py-2 text-sm font-bold text-black transition enabled:hover:bg-zinc-200 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      <Play className="h-4 w-4 fill-black" />
                      Play First
                    </button>

                    <button
                      disabled={rematching}
                      onClick={() => void handleRematch()}
                      className="inline-flex w-full items-center justify-center gap-2 rounded border border-white/25 bg-white/5 px-5 py-2 text-sm font-semibold text-zinc-100 transition hover:border-netflix-red hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      <RefreshCcw className={`h-4 w-4 ${rematching ? "animate-spin" : ""}`} />
                      重新匹配本作品
                    </button>

                    <p className="text-sm leading-6 text-zinc-300">
                      {detail.summary || "No summary available."}
                    </p>

                    <div>
                      <p className="mb-2 text-xs uppercase tracking-wider text-zinc-500">Tags</p>
                      <div className="flex flex-wrap gap-2">
                        {detail.tags.length > 0 ? (
                          detail.tags.map((tag) => (
                            <span key={tag} className="rounded border border-white/15 bg-white/5 px-2 py-1 text-xs">
                              {tag}
                            </span>
                          ))
                        ) : (
                          <span className="text-zinc-500">No tags</span>
                        )}
                      </div>
                    </div>

                    <div>
                      <p className="mb-2 text-xs uppercase tracking-wider text-zinc-500">Library Root</p>
                      <p className="break-all text-xs text-zinc-400">{detail.root}</p>
                    </div>
                  </div>
                </aside>

                <section
                  ref={episodesScrollRef}
                  className="min-h-0 overflow-y-auto p-5 md:p-7 [content-visibility:auto]"
                >
                  <div className="mb-4 flex items-end justify-between">
                    <h4 className="text-lg font-semibold text-white">Episodes</h4>
                    <p className="text-xs text-zinc-400">
                      {visibleEpisodeCount}/{detail.episodes.length} shown
                    </p>
                  </div>

                  <div className="space-y-2">
                    {visibleEpisodes.map((ep) => (
                      <EpisodeCard key={`${ep.episode}-${ep.name}`} ep={ep} root={detail.root} onPlay={handlePlay} />
                    ))}
                  </div>
                  {hasMoreEpisodes ? (
                    <div className="mt-4 flex justify-center">
                      <button
                        onClick={requestLoadMoreEpisodes}
                        className="rounded border border-white/20 px-3 py-1.5 text-xs text-zinc-200 transition hover:border-netflix-red hover:text-white"
                      >
                        加载更多（剩余 {detail.episodes.length - visibleEpisodeCount} 集）
                      </button>
                    </div>
                  ) : null}
                  <div ref={episodesLoadSentinelRef} className="h-1 w-full" aria-hidden="true" />
                </section>
              </div>
            )}
          </section>
        </div>
      ) : null}
    </>
  );
};
