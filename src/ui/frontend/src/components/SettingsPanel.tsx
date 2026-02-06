import { useEffect, useState } from "react";
import { AppConfig } from "../lib/types";
import { useStore } from "../store/useStore";

function cloneConfig(config: AppConfig): AppConfig {
  return JSON.parse(JSON.stringify(config)) as AppConfig;
}

export const SettingsPanel = () => {
  const config = useStore((state) => state.config);
  const isSettingsLoading = useStore((state) => state.isSettingsLoading);
  const isSettingsSaving = useStore((state) => state.isSettingsSaving);
  const saveConfig = useStore((state) => state.saveConfig);
  const loadConfig = useStore((state) => state.loadConfig);
  const refreshNow = useStore((state) => state.refreshNow);
  const stopScanning = useStore((state) => state.stopScanning);
  const [draft, setDraft] = useState<AppConfig | null>(null);

  useEffect(() => {
    if (!config) return;
    setDraft(cloneConfig(config));
  }, [config]);

  if (isSettingsLoading) {
    return <div className="mx-4 rounded-xl border border-white/10 bg-black/35 p-8 text-zinc-300 md:mx-10">加载设置中...</div>;
  }
  if (!draft) {
    return (
      <div className="mx-4 rounded-xl border border-white/10 bg-black/35 p-8 text-zinc-300 md:mx-10">
        <p>设置加载失败。</p>
        <button
          onClick={() => void loadConfig()}
          className="mt-3 rounded-md border border-white/20 px-4 py-2 text-sm text-zinc-100 transition hover:border-netflix-red"
        >
          重试加载
        </button>
      </div>
    );
  }

  const save = async () => {
    await saveConfig(draft);
  };

  return (
    <section className="mx-4 space-y-5 pb-12 md:mx-10">
      <div className="rounded-xl border border-white/10 bg-black/30 p-4">
        <h3 className="text-lg font-semibold text-white">媒体与监控</h3>
        <div className="mt-3 grid gap-3 md:grid-cols-2">
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">媒体目录（动态监控源）</p>
            <input
              value={draft.library.media_root}
              onChange={(e) =>
                setDraft((prev) => (prev ? { ...prev, library: { ...prev.library, media_root: e.target.value } } : prev))
              }
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">数据库目录</p>
            <input
              value={draft.library.dir}
              onChange={(e) =>
                setDraft((prev) => (prev ? { ...prev, library: { ...prev.library, dir: e.target.value } } : prev))
              }
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">自动监控间隔（秒）</p>
            <input
              type="number"
              min={2}
              value={draft.library.watch_interval_secs}
              onChange={(e) =>
                setDraft((prev) =>
                  prev
                    ? { ...prev, library: { ...prev.library, watch_interval_secs: Math.max(2, Number(e.target.value) || 2) } }
                    : prev
                )
              }
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="flex items-center gap-2 pt-6">
            <input
              type="checkbox"
              checked={draft.library.auto_watch}
              onChange={(e) =>
                setDraft((prev) => (prev ? { ...prev, library: { ...prev.library, auto_watch: e.target.checked } } : prev))
              }
            />
            <span className="text-sm text-zinc-200">开启自动监控（notify 文件事件触发更新）</span>
          </label>
        </div>
      </div>

      <div className="rounded-xl border border-white/10 bg-black/30 p-4">
        <h3 className="text-lg font-semibold text-white">抓取参数</h3>
        <div className="mt-3 grid gap-3 md:grid-cols-3">
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">信息刷新周期（天）</p>
            <input
              type="number"
              min={1}
              value={draft.library.refresh_days}
              onChange={(e) =>
                setDraft((prev) =>
                  prev ? { ...prev, library: { ...prev.library, refresh_days: Math.max(1, Number(e.target.value) || 1) } } : prev
                )
              }
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">最小媒体体积（MB）</p>
            <input
              type="number"
              min={1}
              value={draft.media.min_media_size_mb}
              onChange={(e) =>
                setDraft((prev) =>
                  prev
                    ? { ...prev, media: { ...prev.media, min_media_size_mb: Math.max(1, Number(e.target.value) || 1) } }
                    : prev
                )
              }
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">BGM 搜索结果数</p>
            <input
              type="number"
              min={1}
              value={draft.bgm.limit}
              onChange={(e) =>
                setDraft((prev) => (prev ? { ...prev, bgm: { ...prev.bgm, limit: Math.max(1, Number(e.target.value) || 1) } } : prev))
              }
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
        </div>
      </div>

      <div className="rounded-xl border border-white/10 bg-black/30 p-4">
        <h3 className="text-lg font-semibold text-white">LLM / BGM</h3>
        <div className="mt-3 grid gap-3 md:grid-cols-2">
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">LLM Provider</p>
            <input
              value={draft.llm.provider}
              onChange={(e) => setDraft((prev) => (prev ? { ...prev, llm: { ...prev.llm, provider: e.target.value } } : prev))}
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">LLM Model</p>
            <input
              value={draft.llm.model}
              onChange={(e) => setDraft((prev) => (prev ? { ...prev, llm: { ...prev.llm, model: e.target.value } } : prev))}
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">LLM Local URL</p>
            <input
              value={draft.llm.url}
              onChange={(e) => setDraft((prev) => (prev ? { ...prev, llm: { ...prev.llm, url: e.target.value } } : prev))}
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">LLM Remote URL</p>
            <input
              value={draft.llm.remote_url}
              onChange={(e) => setDraft((prev) => (prev ? { ...prev, llm: { ...prev.llm, remote_url: e.target.value } } : prev))}
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">LLM Remote Token</p>
            <input
              value={draft.llm.remote_token}
              onChange={(e) => setDraft((prev) => (prev ? { ...prev, llm: { ...prev.llm, remote_token: e.target.value } } : prev))}
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">LLM Batch Size</p>
            <input
              type="number"
              min={1}
              value={draft.llm.batch_size}
              onChange={(e) =>
                setDraft((prev) =>
                  prev ? { ...prev, llm: { ...prev.llm, batch_size: Math.max(1, Number(e.target.value) || 1) } } : prev
                )
              }
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">LLM Match Concurrency</p>
            <input
              type="number"
              min={1}
              value={draft.llm.match_concurrency}
              onChange={(e) =>
                setDraft((prev) =>
                  prev
                    ? { ...prev, llm: { ...prev.llm, match_concurrency: Math.max(1, Number(e.target.value) || 1) } }
                    : prev
                )
              }
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">BGM Base URL</p>
            <input
              value={draft.bgm.base_url}
              onChange={(e) => setDraft((prev) => (prev ? { ...prev, bgm: { ...prev.bgm, base_url: e.target.value } } : prev))}
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">BGM Token</p>
            <input
              value={draft.bgm.token ?? ""}
              onChange={(e) =>
                setDraft((prev) =>
                  prev ? { ...prev, bgm: { ...prev.bgm, token: e.target.value.trim() ? e.target.value : null } } : prev
                )
              }
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
          <label className="space-y-1">
            <p className="text-xs text-zinc-400">BGM Retry</p>
            <input
              type="number"
              min={0}
              value={draft.bgm.retries}
              onChange={(e) =>
                setDraft((prev) => (prev ? { ...prev, bgm: { ...prev.bgm, retries: Math.max(0, Number(e.target.value) || 0) } } : prev))
              }
              className="w-full rounded-md border border-white/15 bg-[#151515] px-3 py-2 text-sm outline-none focus:border-netflix-red"
            />
          </label>
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <button
          onClick={() => void save()}
          disabled={isSettingsSaving}
          className="rounded-md bg-netflix-red px-4 py-2 text-sm font-semibold text-white transition enabled:hover:bg-red-600 disabled:opacity-60"
        >
          {isSettingsSaving ? "保存中..." : "保存设置"}
        </button>
        <button
          onClick={() => void refreshNow()}
          className="rounded-md border border-white/20 px-4 py-2 text-sm text-zinc-100 transition hover:border-netflix-red"
        >
          立即刷新媒体库
        </button>
        <button
          onClick={() => void stopScanning()}
          className="rounded-md border border-white/20 px-4 py-2 text-sm text-zinc-100 transition hover:border-netflix-red"
        >
          停止后台
        </button>
        <button
          onClick={() => void loadConfig()}
          className="rounded-md border border-white/20 px-4 py-2 text-sm text-zinc-100 transition hover:border-netflix-red"
        >
          重新读取配置
        </button>
      </div>
    </section>
  );
};
