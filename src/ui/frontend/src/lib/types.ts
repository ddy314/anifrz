export interface SeriesWallItem {
    id: number;
    title: string;
    subtitle: string;
    cover_url: string | null;
    cover_local_path: string | null;
    episode_count: number;
    total_episode_count: number;
    missing_count: number;
    updated_at: number;
}

export interface SeriesDetailEpisode {
    episode: number;
    name: string;
    name_cn: string;
    ep_type: number;
    files: string[];
    missing: boolean;
}

export interface SeriesDetail {
    id: number;
    title: string;
    subtitle: string;
    summary: string;
    tags: string[];
    air_date: string | null;
    rating_score: number | null;
    rating_total: number | null;
    cover_url: string | null;
    cover_local_path: string | null;
    root: string;
    episodes: SeriesDetailEpisode[];
    missing_episodes: number[];
    unmatched_files: string[];
    updated_at: number;
}

export interface AppLog {
    id: number;
    ts: number;
    level: string;
    message: string;
}

export interface RuntimeState {
    scraping: boolean;
}

export interface BackendStatusPayload {
    kind: string;
    message: string;
    scraping: boolean;
}

export interface AppConfig {
    bgm: {
        base_url: string;
        token: string | null;
        limit: number;
        retries: number;
    };
    llm: {
        url: string;
        provider: string;
        remote_url: string;
        remote_token: string;
        model: string;
        batch_size: number;
        match_concurrency: number;
    };
    library: {
        dir: string;
        refresh_days: number;
        media_root: string;
        auto_watch: boolean;
        watch_interval_secs: number;
    };
    media: {
        min_media_size_mb: number;
    };
}
