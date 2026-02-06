import { useEffect, useRef, useState } from "react";
import { Loader2, RefreshCcw, Search } from "lucide-react";
import { cn } from "../lib/utils";
import { useStore } from "../store/useStore";

export type AppTab = "home" | "library" | "settings";

interface NavbarProps {
  activeTab: AppTab;
  onTabChange: (tab: AppTab) => void;
}

const tabs: Array<{ id: AppTab; label: string }> = [
  { id: "home", label: "主页" },
  { id: "library", label: "媒体库" },
  { id: "settings", label: "设置" },
];

export const Navbar = ({ activeTab, onTabChange }: NavbarProps) => {
  const searchQuery = useStore((state) => state.searchQuery);
  const setSearchQuery = useStore((state) => state.setSearchQuery);
  const isScanning = useStore((state) => state.isScanning);
  const refreshNow = useStore((state) => state.refreshNow);
  const [scrolled, setScrolled] = useState(false);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    const onScroll = () => {
      if (rafRef.current !== null) return;
      rafRef.current = requestAnimationFrame(() => {
        rafRef.current = null;
        setScrolled(window.scrollY > 40);
      });
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      window.removeEventListener("scroll", onScroll);
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
    };
  }, []);

  return (
    <header
      className={cn(
        "fixed inset-x-0 top-0 z-50 navbar-glass transition-all duration-300",
        scrolled ? "navbar-solid border-b" : "navbar-transparent border-b border-transparent"
      )}
    >
      <div className="mx-auto flex h-14 max-w-[1700px] items-center justify-between gap-3 px-4 md:h-16 md:px-10">
        <div className="flex items-center gap-5">
          <h1 className="text-xl font-black tracking-tight text-netflix-red md:text-2xl">ANIFRZ</h1>
          <nav className="hidden items-center gap-0.5 md:flex">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                onClick={() => onTabChange(tab.id)}
                className={cn(
                  "relative rounded-md px-3.5 py-1.5 text-sm font-medium transition-all duration-200",
                  activeTab === tab.id
                    ? "text-white"
                    : "text-zinc-400 hover:text-white"
                )}
              >
                {tab.label}
                {activeTab === tab.id && (
                  <span className="absolute inset-x-1.5 -bottom-0.5 h-0.5 rounded-full bg-netflix-red" />
                )}
              </button>
            ))}
          </nav>
        </div>

        <div className="flex items-center gap-2.5">
          {activeTab !== "settings" ? (
            <label className="hidden items-center gap-2 rounded-lg border border-white/10 bg-white/[0.06] px-3 py-1.5 transition-colors focus-within:border-white/25 focus-within:bg-white/10 md:flex">
              <Search className="h-4 w-4 text-zinc-500" />
              <input
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="搜索..."
                className="w-40 bg-transparent text-sm text-white outline-none placeholder:text-zinc-500 xl:w-56"
              />
            </label>
          ) : null}

          <button
            onClick={() => void refreshNow()}
            className="inline-flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.06] px-3 py-1.5 text-xs font-medium text-zinc-200 transition-all hover:bg-white/15 hover:text-white active:scale-95"
          >
            <RefreshCcw className="h-3.5 w-3.5" />
            刷新
          </button>
          {isScanning ? (
            <span className="inline-flex items-center gap-1.5 rounded-lg border border-emerald-400/25 bg-emerald-500/10 px-2.5 py-1.5 text-xs font-medium text-emerald-300">
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              扫描中
            </span>
          ) : null}
        </div>
      </div>

      {/* Mobile tab bar */}
      <div className="flex gap-0.5 border-t border-white/[0.06] bg-black/60 p-1 backdrop-blur-lg md:hidden">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => onTabChange(tab.id)}
            className={cn(
              "flex-1 rounded-md px-3 py-2 text-xs font-medium transition-all",
              activeTab === tab.id
                ? "bg-white/10 text-white"
                : "text-zinc-400 active:bg-white/5"
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>
    </header>
  );
};
