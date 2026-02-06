import { create } from "zustand";
import { api } from "../lib/api";
import { AppConfig, BackendStatusPayload, SeriesWallItem } from "../lib/types";
import { listenTauriEvent } from "../lib/utils";

let unlistenBackendStatus: (() => void) | null = null;
let errorTimer: ReturnType<typeof setTimeout> | null = null;
let libraryRefreshTimer: ReturnType<typeof setTimeout> | null = null;
let runtimeStatePoller: ReturnType<typeof setInterval> | null = null;

interface AppState {
  library: SeriesWallItem[];
  featured: SeriesWallItem | null;
  isLoading: boolean;
  isScanning: boolean;
  isSettingsLoading: boolean;
  isSettingsSaving: boolean;
  searchQuery: string;
  errorMessage: string | null;
  config: AppConfig | null;
  lastBackendMessage: string | null;

  isDetailOpen: boolean;
  selectedSeriesId: number | null;

  initialize: () => Promise<void>;
  fetchLibrary: () => Promise<void>;
  refreshNow: () => Promise<void>;
  stopScanning: () => Promise<void>;
  loadConfig: () => Promise<void>;
  saveConfig: (next: AppConfig) => Promise<void>;
  rematchSeries: (id: number) => Promise<void>;
  setSearchQuery: (query: string) => void;
  setError: (message: string | null, autoClearMs?: number) => void;
  clearError: () => void;
  openDetail: (id: number) => void;
  closeDetail: () => void;
}

function chooseFeatured(items: SeriesWallItem[]): SeriesWallItem | null {
  if (items.length === 0) return null;
  const ranked = [...items].sort((a, b) => {
    if (b.updated_at !== a.updated_at) return b.updated_at - a.updated_at;
    return b.total_episode_count - a.total_episode_count;
  });
  return ranked[0] ?? items[0];
}

function clearPendingErrorTimer() {
  if (errorTimer) {
    clearTimeout(errorTimer);
    errorTimer = null;
  }
}

async function ensureBackendStatusListener(
  set: (partial: Partial<AppState>) => void,
  get: () => AppState
) {
  if (unlistenBackendStatus) return;
  try {
    unlistenBackendStatus = await listenTauriEvent<BackendStatusPayload>("backend://status", (payload) => {
      const scanning =
        payload.kind === "finished" || payload.kind === "error" ? false : payload.scraping;
      set({
        isScanning: scanning,
        lastBackendMessage: payload.message,
      });

      if (payload.kind === "error") {
        get().setError(payload.message);
        return;
      }
      if (payload.kind === "series_saved") {
        if (!libraryRefreshTimer) {
          libraryRefreshTimer = setTimeout(() => {
            libraryRefreshTimer = null;
            if (!get().isLoading) {
              void get().fetchLibrary();
            }
          }, get().isScanning ? 4200 : 1200);
        }
      }
      if (payload.kind === "finished") {
        if (libraryRefreshTimer) {
          clearTimeout(libraryRefreshTimer);
          libraryRefreshTimer = null;
        }
        void get().fetchLibrary();
      }
    });
  } catch (error) {
    set({
      lastBackendMessage: `事件通道初始化失败: ${String(error)}`,
    });
  }
}

function ensureRuntimeStatePoller(
  set: (partial: Partial<AppState>) => void,
  get: () => AppState
) {
  if (runtimeStatePoller) return;
  runtimeStatePoller = setInterval(() => {
    if (!get().isScanning) return;
    api
      .getRuntimeState()
      .then((runtime) => {
        if (!runtime.scraping && get().isScanning) {
          set({ isScanning: false });
          if (!get().isLoading) {
            void get().fetchLibrary();
          }
        }
      })
      .catch(() => {
        // ignore runtime polling failures
      });
  }, 2500);
}

export const useStore = create<AppState>((set, get) => ({
  library: [],
  featured: null,
  isLoading: false,
  isScanning: false,
  isSettingsLoading: false,
  isSettingsSaving: false,
  searchQuery: "",
  errorMessage: null,
  config: null,
  lastBackendMessage: null,

  isDetailOpen: false,
  selectedSeriesId: null,

  initialize: async () => {
    await ensureBackendStatusListener(set, get);
    ensureRuntimeStatePoller(set, get);
    await Promise.all([get().loadConfig(), get().fetchLibrary()]);
    try {
      const runtime = await api.getRuntimeState();
      set({ isScanning: runtime.scraping });
    } catch {
      // ignore runtime state read failure
    }
  },

  fetchLibrary: async () => {
    set({ isLoading: true });
    try {
      const library = await api.listWall(400);
      set({
        library,
        featured: chooseFeatured(library),
        isLoading: false,
      });
    } catch (error) {
      set({ isLoading: false });
      get().setError(String(error));
    }
  },

  refreshNow: async () => {
    const root = get().config?.library.media_root?.trim() ?? "";
    if (!root) {
      get().setError("请先在设置中配置媒体目录", 2800);
      return;
    }
    try {
      await api.startScrape(root);
      set({ isScanning: true });
      get().clearError();
    } catch (error) {
      get().setError(String(error));
      set({ isScanning: false });
    }
  },

  stopScanning: async () => {
    try {
      await api.stopBackend();
    } catch {
      // keep local state reset even if backend already stopped
    } finally {
      set({ isScanning: false });
    }
  },

  loadConfig: async () => {
    set({ isSettingsLoading: true });
    try {
      const config = await api.getAppConfig();
      set({ config, isSettingsLoading: false });
      if (config.library.media_root.trim()) {
        get().clearError();
      }
    } catch (error) {
      set({ isSettingsLoading: false });
      get().setError(String(error));
    }
  },

  saveConfig: async (next: AppConfig) => {
    set({ isSettingsSaving: true });
    try {
      await api.saveAppConfig(next);
      set({ config: next, isSettingsSaving: false });
      if (next.library.media_root.trim()) {
        get().clearError();
      }
    } catch (error) {
      set({ isSettingsSaving: false });
      get().setError(String(error));
    }
  },

  rematchSeries: async (id: number) => {
    if (!id) return;
    try {
      await api.rematchSeries(id);
      set({ isScanning: true });
      get().clearError();
      void get().fetchLibrary();
    } catch (error) {
      get().setError(String(error));
    }
  },

  setSearchQuery: (query: string) => set({ searchQuery: query }),

  setError: (message: string | null, autoClearMs?: number) => {
    clearPendingErrorTimer();
    set({ errorMessage: message });
    if (message && autoClearMs && autoClearMs > 0) {
      errorTimer = setTimeout(() => {
        set({ errorMessage: null });
        errorTimer = null;
      }, autoClearMs);
    }
  },

  clearError: () => {
    clearPendingErrorTimer();
    set({ errorMessage: null });
  },

  openDetail: (id: number) => set({ isDetailOpen: true, selectedSeriesId: id }),
  closeDetail: () => set({ isDetailOpen: false, selectedSeriesId: null }),
}));
