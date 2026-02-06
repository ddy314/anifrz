import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { SeriesWallItem } from "../lib/types";
import { MediaCard } from "./MediaCard";
import { cn } from "../lib/utils";

interface MediaRowProps {
  title: string;
  subtitle?: string;
  items: SeriesWallItem[];
}

const MediaRowInner = ({ title, subtitle, items }: MediaRowProps) => {
  const rowRef = useRef<HTMLDivElement>(null);
  const scrollRafRef = useRef<number | null>(null);
  const [atStart, setAtStart] = useState(true);
  const [atEnd, setAtEnd] = useState(false);

  const canScroll = items.length > 6;
  const renderedItems = useMemo(() => items.slice(0, 30), [items]);

  const updateBounds = useCallback(() => {
    const el = rowRef.current;
    if (!el) return;
    const epsilon = 4;
    const nextAtStart = el.scrollLeft <= epsilon;
    const nextAtEnd = el.scrollLeft + el.clientWidth >= el.scrollWidth - epsilon;

    setAtStart((prev) => (prev === nextAtStart ? prev : nextAtStart));
    setAtEnd((prev) => (prev === nextAtEnd ? prev : nextAtEnd));
  }, []);

  useEffect(() => {
    updateBounds();
  }, [renderedItems.length, updateBounds]);

  const shift = (direction: "left" | "right") => {
    const el = rowRef.current;
    if (!el) return;
    const amount = Math.max(el.clientWidth * 0.82, 220);
    const left = direction === "left" ? el.scrollLeft - amount : el.scrollLeft + amount;
    el.scrollTo({ left, behavior: "smooth" });
    // Use rAF chain to update bounds after scroll settles
    const checkBounds = () => {
      updateBounds();
      setTimeout(updateBounds, 350);
    };
    requestAnimationFrame(checkBounds);
  };

  const onScroll = () => {
    if (scrollRafRef.current !== null) return;
    scrollRafRef.current = requestAnimationFrame(() => {
      scrollRafRef.current = null;
      updateBounds();
    });
  };

  useEffect(() => {
    // Add passive scroll listener for better performance
    const el = rowRef.current;
    if (!el) return;
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      el.removeEventListener("scroll", onScroll);
      if (scrollRafRef.current !== null) {
        cancelAnimationFrame(scrollRafRef.current);
      }
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (renderedItems.length === 0) return null;

  return (
    <section className="space-y-3 px-4 md:px-10" style={{ contentVisibility: "auto", containIntrinsicSize: "auto 320px" }}>
      <div className="flex items-end justify-between">
        <div>
          <h3 className="text-lg font-bold text-zinc-50 md:text-xl">{title}</h3>
          {subtitle ? <p className="mt-0.5 text-xs text-zinc-500 md:text-sm">{subtitle}</p> : null}
        </div>
        {canScroll && (
          <div className="hidden items-center gap-1 md:flex">
            <button
              type="button"
              aria-label="scroll left"
              onClick={() => shift("left")}
              disabled={atStart}
              className="flex h-8 w-8 items-center justify-center rounded-full bg-white/[0.08] text-zinc-300 transition-all hover:bg-white/15 hover:text-white disabled:opacity-25 disabled:pointer-events-none"
            >
              <ChevronLeft className="h-4 w-4" />
            </button>
            <button
              type="button"
              aria-label="scroll right"
              onClick={() => shift("right")}
              disabled={atEnd}
              className="flex h-8 w-8 items-center justify-center rounded-full bg-white/[0.08] text-zinc-300 transition-all hover:bg-white/15 hover:text-white disabled:opacity-25 disabled:pointer-events-none"
            >
              <ChevronRight className="h-4 w-4" />
            </button>
          </div>
        )}
      </div>

      <div className="group relative">
        {/* Fade edges */}
        <div
          className={cn(
            "pointer-events-none absolute inset-y-0 left-0 z-10 w-12 bg-gradient-to-r from-[#141414] to-transparent transition-opacity duration-300",
            atStart ? "opacity-0" : "opacity-100"
          )}
        />
        <div
          className={cn(
            "pointer-events-none absolute inset-y-0 right-0 z-10 w-12 bg-gradient-to-l from-[#141414] to-transparent transition-opacity duration-300",
            atEnd ? "opacity-0" : "opacity-100"
          )}
        />

        <div
          ref={rowRef}
          className="no-scrollbar smooth-scroll-row flex gap-2.5 overflow-x-auto py-2 md:gap-3"
        >
          {renderedItems.map((series) => (
            <MediaCard key={series.id} series={series} />
          ))}
        </div>
      </div>
    </section>
  );
};

export const MediaRow = memo(MediaRowInner);
