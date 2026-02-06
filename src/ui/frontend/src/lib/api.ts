import { AppConfig, RuntimeState, SeriesWallItem, SeriesDetail, AppLog } from './types';
import { invokeTauri } from './utils';

export const api = {
  startScrape: async (root: string) => {
    return invokeTauri<void>('start_scrape', { root });
  },
  
  stopBackend: async () => {
    return invokeTauri<void>('stop_backend');
  },

  listWall: async (limit?: number): Promise<SeriesWallItem[]> => {
    return invokeTauri<SeriesWallItem[]>('list_wall', { limit });
  },

  getSeriesDetail: async (id: number): Promise<SeriesDetail | null> => {
    return invokeTauri<SeriesDetail | null>('get_series_detail', { id });
  },

  rematchSeries: async (id: number): Promise<void> => {
    return invokeTauri<void>('rematch_series', { id });
  },

  playEpisode: async (filePath: string) => {
    return invokeTauri<void>('play_episode', { filePath });
  },

  getCoverDataUrl: async (path: string): Promise<string | null> => {
    return invokeTauri<string | null>('get_cover_data_url', { path });
  },

  readLogs: async (afterId?: number, limit?: number): Promise<AppLog[]> => {
    return invokeTauri<AppLog[]>('read_logs', { afterId, limit });
  },

  getRuntimeState: async (): Promise<RuntimeState> => {
    return invokeTauri<RuntimeState>('get_runtime_state');
  },

  getAppConfig: async (): Promise<AppConfig> => {
    return invokeTauri<AppConfig>('get_app_config');
  },

  saveAppConfig: async (config: AppConfig): Promise<void> => {
    return invokeTauri<void>('save_app_config', { config });
  }
};
