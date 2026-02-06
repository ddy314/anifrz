import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Info, Play } from "lucide-react";
import { useStore } from "../store/useStore";
import { CoverImage } from "./CoverImage";
import { SeriesWallItem } from "../lib/types";

const fallbackCover =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='1280' height='720'%3E%3Crect width='100%25' height='100%25' fill='%23141414'/%3E%3Ctext x='50%25' y='50%25' fill='white' font-size='34' text-anchor='middle' dominant-baseline='middle'%3EANIFRZ%3C/text%3E%3C/svg%3E";

const HERO_INTERVAL = 8000; // 8 seconds per slide
const MAX_HERO_ITEMS = 6;

interface HeroSlideProps {
  item: SeriesWallItem;
  isActive: boolean;
  onAction: (id: number) => void;
}

const HeroSlide = ({ item, isActive, onAction }: HeroSlideProps) => {
  const [isPortrait, setIsPortrait] = useState(false);

  return (
    <div
      className={`absolute inset-0 transition-opacity duration-700 ease-in-out ${
        isActive ? "opacity-100 z-[2]" : "opacity-0 z-[1]"
      }`}
    >
      {/* Background blur layer */}
      <div className="absolute inset-0 overflow-hidden">
        <CoverImage
          path={item.cover_local_path}
          alt=""
          fallback={fallbackCover}
          className="absolute inset-0 h-full w-full scale-110 object-cover opacity-30 blur-2xl saturate-125"
        />
      </div>

      {/* Main cover image */}
      <div className="absolute inset-0 overflow-hidden">
        <CoverImage
          path={item.cover_local_path}
          alt={item.title}
          fallback={fallbackCover}
          onImageLoad={(meta) => setIsPortrait(meta.isPortrait)}
          className={`absolute inset-0 h-full w-full ${
            isActive ? "hero-image-anim" : ""
          } ${
            isPortrait
              ? "object-contain object-right pr-[5%] md:object-[85%_center]"
              : "object-cover object-[50%_20%] saturate-[1.1]"
          }`}
        />
      </div>

      {/* Gradient overlays */}
      <div className="absolute inset-0 bg-gradient-to-r from-[#141414] via-[#141414]/70 to-transparent" />
      <div className="absolute inset-0 bg-gradient-to-t from-[#141414] via-transparent to-[#141414]/30" />
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_80%_20%,rgba(229,9,20,0.12),transparent_50%)]" />

      {/* Content */}
      <div className="relative mx-auto flex h-full max-w-[1700px] items-end px-4 pb-28 pt-24 md:px-10 md:pb-32">
        <div className={`max-w-xl space-y-4 ${isActive ? "hero-fade-in" : ""}`}>
          {/* Badge */}
          <div className="flex items-center gap-2">
            <span className="inline-flex items-center gap-1.5 rounded bg-netflix-red/90 px-2.5 py-1 text-[11px] font-bold uppercase tracking-wider text-white shadow-lg">
              <span className="h-1.5 w-1.5 rounded-full bg-white animate-pulse" />
              推荐
            </span>
            {item.missing_count === 0 && item.episode_count > 0 && (
              <span className="rounded bg-emerald-500/20 border border-emerald-400/40 px-2 py-0.5 text-[11px] font-medium text-emerald-300">
                已完整
              </span>
            )}
          </div>

          {/* Title */}
          <h2 className="text-3xl font-black leading-[1.1] tracking-tight text-white drop-shadow-xl md:text-5xl lg:text-6xl">
            {item.title}
          </h2>

          {/* Subtitle */}
          {item.subtitle && item.subtitle !== item.title && (
            <p className="max-w-md text-sm leading-relaxed text-zinc-300/90 md:text-base">
              {item.subtitle}
            </p>
          )}

          {/* Metadata */}
          <div className="flex flex-wrap items-center gap-2 text-xs">
            <span className="rounded-md bg-white/10 px-2.5 py-1 text-zinc-200 backdrop-blur-sm">
              共 {item.total_episode_count} 集
            </span>
            <span className="rounded-md bg-white/10 px-2.5 py-1 text-zinc-200 backdrop-blur-sm">
              已收录 {item.episode_count} 集
            </span>
            {item.missing_count > 0 && (
              <span className="rounded-md bg-amber-500/15 border border-amber-400/30 px-2.5 py-1 text-amber-300">
                缺失 {item.missing_count} 集
              </span>
            )}
          </div>

          {/* Actions */}
          <div className="flex flex-wrap items-center gap-3 pt-1">
            <button
              onClick={() => onAction(item.id)}
              className="inline-flex items-center gap-2.5 rounded-md bg-white px-7 py-3 text-sm font-bold text-black shadow-xl transition-all hover:bg-zinc-100 hover:shadow-2xl hover:scale-[1.02] active:scale-[0.98]"
            >
              <Play className="h-5 w-5 fill-black" />
              播放
            </button>
            <button
              onClick={() => onAction(item.id)}
              className="inline-flex items-center gap-2.5 rounded-md bg-white/15 px-7 py-3 text-sm font-semibold text-white shadow-lg backdrop-blur-sm transition-all hover:bg-white/25 hover:scale-[1.02] active:scale-[0.98]"
            >
              <Info className="h-5 w-5" />
              详情
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export const Hero = () => {
  const library = useStore((state) => state.library);
  const openDetail = useStore((state) => state.openDetail);
  const isDetailOpen = useStore((state) => state.isDetailOpen);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [isPaused, setIsPaused] = useState(false);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const progressKeyRef = useRef(0);

  // Pick top items for hero carousel
  const heroItems = useMemo(() => {
    if (library.length === 0) return [];
    const sorted = [...library].sort((a, b) => {
      // Prefer items with covers and more episodes
      const aScore = (a.cover_local_path ? 10 : 0) + a.total_episode_count;
      const bScore = (b.cover_local_path ? 10 : 0) + b.total_episode_count;
      if (bScore !== aScore) return bScore - aScore;
      return b.updated_at - a.updated_at;
    });
    return sorted.slice(0, MAX_HERO_ITEMS);
  }, [library]);

  const startTimer = useCallback(() => {
    if (timerRef.current) clearInterval(timerRef.current);
    timerRef.current = setInterval(() => {
      setCurrentIndex((prev) => (prev + 1) % heroItems.length);
      progressKeyRef.current += 1;
    }, HERO_INTERVAL);
  }, [heroItems.length]);

  useEffect(() => {
    if (heroItems.length <= 1 || isPaused || isDetailOpen) {
      if (timerRef.current) clearInterval(timerRef.current);
      return;
    }
    startTimer();
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [heroItems.length, isPaused, isDetailOpen, startTimer]);

  const goTo = useCallback(
    (index: number) => {
      setCurrentIndex(index);
      progressKeyRef.current += 1;
      startTimer();
    },
    [startTimer]
  );

  const goNext = useCallback(() => {
    goTo((currentIndex + 1) % heroItems.length);
  }, [currentIndex, goTo, heroItems.length]);

  const goPrev = useCallback(() => {
    goTo((currentIndex - 1 + heroItems.length) % heroItems.length);
  }, [currentIndex, goTo, heroItems.length]);

  if (heroItems.length === 0) return null;

  return (
    <section
      className="relative h-[75vh] min-h-[500px] max-h-[800px] w-full overflow-hidden gpu-accelerated"
      onMouseEnter={() => setIsPaused(true)}
      onMouseLeave={() => setIsPaused(false)}
    >
      {/* Slides */}
      {heroItems.map((item, i) => (
        <HeroSlide
          key={item.id}
          item={item}
          isActive={i === currentIndex}
          onAction={openDetail}
        />
      ))}

      {/* Navigation arrows */}
      {heroItems.length > 1 && (
        <>
          <button
            onClick={goPrev}
            className="absolute left-3 top-1/2 z-10 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full bg-black/40 text-white/70 backdrop-blur-sm transition-all hover:bg-black/60 hover:text-white hover:scale-110 md:left-6"
            aria-label="Previous"
          >
            <ChevronLeft className="h-5 w-5" />
          </button>
          <button
            onClick={goNext}
            className="absolute right-3 top-1/2 z-10 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full bg-black/40 text-white/70 backdrop-blur-sm transition-all hover:bg-black/60 hover:text-white hover:scale-110 md:right-6"
            aria-label="Next"
          >
            <ChevronRight className="h-5 w-5" />
          </button>
        </>
      )}

      {/* Bottom dots + progress */}
      {heroItems.length > 1 && (
        <div className="absolute bottom-6 left-1/2 z-10 -translate-x-1/2 flex items-center gap-2 md:bottom-8">
          {heroItems.map((_, i) => (
            <button
              key={i}
              onClick={() => goTo(i)}
              className="group relative h-1.5 overflow-hidden rounded-full transition-all duration-300"
              style={{ width: i === currentIndex ? 32 : 12 }}
              aria-label={`Go to slide ${i + 1}`}
            >
              <div className="absolute inset-0 rounded-full bg-white/30" />
              {i === currentIndex && (
                <div
                  key={progressKeyRef.current}
                  className="hero-progress-bar absolute inset-y-0 left-0 rounded-full bg-white"
                  style={{ "--hero-duration": `${HERO_INTERVAL}ms` } as React.CSSProperties}
                />
              )}
            </button>
          ))}
        </div>
      )}
    </section>
  );
};
