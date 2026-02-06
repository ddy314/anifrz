import { useEffect, useMemo, useState } from "react";
import { useInView } from "react-intersection-observer";
import { LayoutGrid } from "lucide-react";
import { SeriesWallItem } from "../lib/types";
import { MediaCard } from "./MediaCard";
import { cn } from "../lib/utils";

interface LibraryGridProps {
  items: SeriesWallItem[];
}

const GRID_BATCH = 96;

type GridSize = "compact" | "normal" | "large";

export const LibraryGrid = ({ items }: LibraryGridProps) => {
  const [visibleCount, setVisibleCount] = useState(GRID_BATCH);
  const [gridSize, setGridSize] = useState<GridSize>("normal");
  const { ref, inView } = useInView({
    rootMargin: "720px 0px",
    threshold: 0,
  });

  useEffect(() => {
    setVisibleCount(Math.min(items.length, GRID_BATCH));
  }, [items.length]);

  useEffect(() => {
    if (!inView) return;
    if (visibleCount >= items.length) return;
    setVisibleCount((prev) => Math.min(items.length, prev + GRID_BATCH));
  }, [inView, items.length, visibleCount]);

  const visibleItems = useMemo(() => items.slice(0, visibleCount), [items, visibleCount]);

  const gridCols = {
    compact: "grid-cols-3 md:grid-cols-5 lg:grid-cols-7 xl:grid-cols-9 2xl:grid-cols-10",
    normal: "grid-cols-2 md:grid-cols-4 lg:grid-cols-6 xl:grid-cols-8",
    large: "grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6",
  };

  if (items.length === 0) {
    return (
      <div className="mx-4 rounded-xl border border-white/10 bg-black/35 p-12 text-center md:mx-10">
        <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-white/5">
          <LayoutGrid className="h-8 w-8 text-zinc-500" />
        </div>
        <p className="text-zinc-300 text-lg font-medium">媒体库为空</p>
        <p className="mt-2 text-sm text-zinc-500">请先在设置中配置目录并执行刷新</p>
      </div>
    );
  }

  return (
    <section className="mx-4 pb-12 md:mx-10">
      {/* Toolbar */}
      <div className="mb-5 flex items-center justify-between">
        <p className="text-sm text-zinc-400">
          共 <span className="font-medium text-zinc-200">{items.length}</span> 个条目
          {visibleItems.length < items.length && (
            <span className="ml-1">· 已加载 {visibleItems.length}</span>
          )}
        </p>
        <div className="flex items-center gap-1 rounded-lg bg-white/[0.06] p-1">
          {(["compact", "normal", "large"] as GridSize[]).map((size) => (
            <button
              key={size}
              onClick={() => setGridSize(size)}
              className={cn(
                "rounded-md px-2.5 py-1.5 text-[11px] transition-all",
                gridSize === size
                  ? "bg-white/15 text-white shadow-sm"
                  : "text-zinc-400 hover:text-zinc-200"
              )}
            >
              {size === "compact" ? "紧凑" : size === "normal" ? "标准" : "大图"}
            </button>
          ))}
        </div>
      </div>

      {/* Grid */}
      <div className={cn("grid gap-3 md:gap-4", gridCols[gridSize])}>
        {visibleItems.map((series) => (
          <MediaCard key={series.id} series={series} className="w-full md:w-full" />
        ))}
      </div>

      {/* Load more sentinel */}
      {visibleItems.length < items.length ? (
        <div ref={ref} className="mt-8 flex flex-col items-center gap-2">
          <div className="h-6 w-6 animate-spin rounded-full border-2 border-zinc-600 border-t-netflix-red" />
          <span className="text-xs text-zinc-500">正在加载更多...</span>
        </div>
      ) : items.length > GRID_BATCH ? (
        <p className="mt-6 text-center text-xs text-zinc-600">已加载全部 {items.length} 个条目</p>
      ) : null}
    </section>
  );
};
