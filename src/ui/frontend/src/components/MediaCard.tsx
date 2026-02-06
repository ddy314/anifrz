import { memo } from "react";
import { Play, Plus } from "lucide-react";
import { SeriesWallItem } from "../lib/types";
import { cn } from "../lib/utils";
import { useStore } from "../store/useStore";
import { CoverImage } from "./CoverImage";

const fallbackCover =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='300' height='450'%3E%3Crect width='100%25' height='100%25' fill='%23181818'/%3E%3Ctext x='50%25' y='50%25' fill='white' font-size='20' text-anchor='middle' dominant-baseline='middle'%3ENo Cover%3C/text%3E%3C/svg%3E";

interface MediaCardProps {
  series: SeriesWallItem;
  className?: string;
}

const MediaCardInner = ({ series, className }: MediaCardProps) => {
  const openDetail = useStore((state) => state.openDetail);

  const progressPercent =
    series.total_episode_count > 0
      ? Math.round((series.episode_count / series.total_episode_count) * 100)
      : 0;

  return (
    <button
      type="button"
      className={cn("media-card aspect-[2/3] w-[154px] shrink-0 text-left md:w-[190px]", className)}
      onClick={() => openDetail(series.id)}
    >
      {/* Cover image */}
      <CoverImage
        path={series.cover_local_path}
        alt={series.title}
        fallback={series.cover_url || fallbackCover}
        className="card-image"
        loading="lazy"
      />

      {/* Gradient always visible */}
      <div className="card-gradient" />

      {/* Title bar - visible when not hovering */}
      <div className="card-title-bar">
        <h4 className="line-clamp-1 text-[13px] font-semibold text-white drop-shadow-md">{series.title}</h4>
        {/* Progress bar */}
        {series.total_episode_count > 0 && (
          <div className="mt-1.5 h-[3px] w-full overflow-hidden rounded-full bg-white/15">
            <div
              className="h-full rounded-full transition-all duration-500"
              style={{
                width: `${progressPercent}%`,
                backgroundColor: progressPercent >= 100 ? "#22c55e" : "#E50914",
              }}
            />
          </div>
        )}
      </div>

      {/* Hover overlay */}
      <div className="card-overlay">
        <h4 className="line-clamp-2 text-[13px] font-bold leading-snug text-white drop-shadow-md">{series.title}</h4>
        <p className="mt-1 text-[11px] text-zinc-300 drop-shadow">
          EP {series.episode_count}/{series.total_episode_count}
          {series.missing_count > 0 && (
            <span className="ml-1 text-amber-400">· 缺 {series.missing_count}</span>
          )}
        </p>
        {/* Progress bar in overlay */}
        {series.total_episode_count > 0 && (
          <div className="mt-1.5 h-[3px] w-full overflow-hidden rounded-full bg-white/20">
            <div
              className="h-full rounded-full"
              style={{
                width: `${progressPercent}%`,
                backgroundColor: progressPercent >= 100 ? "#22c55e" : "#E50914",
              }}
            />
          </div>
        )}
        <div className="mt-2.5 flex items-center gap-2">
          <span className="inline-flex h-7 w-7 items-center justify-center rounded-full bg-white text-black shadow-lg transition-transform hover:scale-110">
            <Play className="h-3.5 w-3.5 fill-black" />
          </span>
          <span className="inline-flex h-7 w-7 items-center justify-center rounded-full border border-zinc-400/60 text-zinc-100 transition-all hover:scale-110 hover:border-white">
            <Plus className="h-3.5 w-3.5" />
          </span>
        </div>
      </div>
    </button>
  );
};

export const MediaCard = memo(MediaCardInner, (prev, next) => {
  if (prev.className !== next.className) return false;
  return (
    prev.series.id === next.series.id &&
    prev.series.updated_at === next.series.updated_at &&
    prev.series.cover_local_path === next.series.cover_local_path &&
    prev.series.cover_url === next.series.cover_url &&
    prev.series.episode_count === next.series.episode_count &&
    prev.series.total_episode_count === next.series.total_episode_count &&
    prev.series.missing_count === next.series.missing_count
  );
});
