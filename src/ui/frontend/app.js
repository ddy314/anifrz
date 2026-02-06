const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke;
const convertFileSrc = tauri?.core?.convertFileSrc;

const wallGrid = document.getElementById("wallGrid");
const wallCount = document.getElementById("wallCount");
const detailPane = document.getElementById("detailPane");
const rootInput = document.getElementById("rootPath");
const startBtn = document.getElementById("startBtn");
const reloadBtn = document.getElementById("reloadBtn");
const logView = document.getElementById("logView");
const logState = document.getElementById("logState");

const state = {
  wall: [],
  selectedId: null,
  lastLogId: 0,
};

const COVER_FALLBACK =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 400">
      <defs>
        <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stop-color="#f7deba"/>
          <stop offset="100%" stop-color="#d4ece0"/>
        </linearGradient>
      </defs>
      <rect width="300" height="400" fill="url(#g)"/>
      <text x="150" y="205" text-anchor="middle" font-size="26" fill="#4d6763" font-family="sans-serif">No Cover</text>
    </svg>`
  );

function canUseTauri() {
  return typeof invoke === "function";
}

async function invokeCmd(cmd, payload = {}) {
  if (!canUseTauri()) {
    throw new Error("Tauri API 不可用");
  }
  return invoke(cmd, payload);
}

function coverSrc(path) {
  if (!path) return COVER_FALLBACK;
  if (typeof convertFileSrc === "function") {
    return convertFileSrc(path);
  }
  return path;
}

function renderWall() {
  wallCount.textContent = `${state.wall.length} 部作品`;
  if (state.wall.length === 0) {
    wallGrid.innerHTML = `<div class="wall-sub">暂无作品，请先刮削</div>`;
    return;
  }

  wallGrid.innerHTML = state.wall
    .map((item) => {
      const active = item.id === state.selectedId ? "active" : "";
      return `
      <article class="wall-card ${active}" data-id="${item.id}">
        <img class="cover" src="${coverSrc(item.cover_local_path)}" alt="${escapeHtml(item.title)}" />
        <div class="wall-body">
          <div class="wall-title">${escapeHtml(item.title)}</div>
          <div class="wall-sub">本地 ${item.episode_count} 集 / 缺失 ${item.missing_count}</div>
        </div>
      </article>
    `;
    })
    .join("");
}

function renderDetail(detail) {
  if (!detail) {
    detailPane.className = "detail-pane empty";
    detailPane.textContent = "未找到该作品";
    return;
  }
  detailPane.className = "detail-pane";
  const tags = (detail.tags || [])
    .slice(0, 8)
    .map((t) => `<span class="chip">${escapeHtml(t)}</span>`)
    .join("");

  const episodesHtml = (detail.episodes || [])
    .map((ep) => {
      const epName = ep.name_cn || ep.name || `Episode ${ep.episode}`;
      const files = (ep.files || [])
        .map((file) => {
          const absolute = resolvePlayPath(detail.root, file);
          return `<button class="file-btn" data-play="${escapeHtmlAttr(absolute)}">${escapeHtml(file)}</button>`;
        })
        .join("");
      return `
        <article class="episode-item">
          <div class="ep-title">EP ${ep.episode} · ${escapeHtml(epName)}</div>
          <div class="ep-files">${files || `<div class="wall-sub">无本地文件</div>`}</div>
        </article>
      `;
    })
    .join("");

  detailPane.innerHTML = `
    <div class="detail-main">
      <img class="detail-cover" src="${coverSrc(detail.cover_local_path)}" alt="${escapeHtml(detail.title)}" />
      <div>
        <h3 class="detail-title">${escapeHtml(detail.title)}</h3>
        <div class="detail-sub">${escapeHtml(detail.subtitle || "")}</div>
        <div class="detail-sub">放送日: ${escapeHtml(detail.air_date || "-")} | 评分: ${detail.rating_score ?? "-"} (${detail.rating_total ?? "-"})</div>
        <div class="chips">${tags}</div>
        <div class="detail-summary">${escapeHtml(detail.summary || "暂无简介")}</div>
      </div>
    </div>
    <div class="episodes">${episodesHtml || `<div class="wall-sub">暂无本地剧集</div>`}</div>
  `;
}

async function loadWall(selectFirst = false) {
  const list = await invokeCmd("list_wall", { limit: 180 });
  state.wall = Array.isArray(list) ? list : [];
  if (selectFirst && state.wall.length > 0 && !state.selectedId) {
    state.selectedId = state.wall[0].id;
  }
  renderWall();
  if (state.selectedId) {
    await loadDetail(state.selectedId);
  }
}

async function loadDetail(id) {
  state.selectedId = id;
  renderWall();
  const detail = await invokeCmd("get_series_detail", { id });
  renderDetail(detail);
}

async function startScrape() {
  const root = rootInput.value.trim();
  if (!root) {
    appendClientLog("error", "请输入有效媒体目录");
    return;
  }
  await invokeCmd("start_scrape", { root });
  appendClientLog("info", `已提交刮削任务: ${root}`);
}

async function playPath(path) {
  await invokeCmd("play_episode", { filePath: path });
}

async function pollLogs() {
  const logs = await invokeCmd("read_logs", { afterId: state.lastLogId, limit: 260 });
  if (!Array.isArray(logs) || logs.length === 0) return;
  state.lastLogId = logs[logs.length - 1].id || state.lastLogId;
  const lines = logs
    .map((log) => {
      const time = formatTs(log.ts);
      return `[${time}] ${String(log.level || "info").toUpperCase()} ${log.message || ""}`;
    })
    .join("\n");
  if (lines) {
    logView.textContent = `${lines}\n${logView.textContent}`.slice(0, 10000);
  }
  logState.textContent = `最新日志 ID: ${state.lastLogId}`;
}

function appendClientLog(level, text) {
  const line = `[${new Date().toLocaleTimeString()}] ${String(level).toUpperCase()} ${text}`;
  logView.textContent = `${line}\n${logView.textContent}`.slice(0, 10000);
}

function resolvePlayPath(root, file) {
  if (!file) return "";
  if (file.startsWith("/") || /^[A-Za-z]:[\\/]/.test(file)) return file;
  if (!root) return file;
  const normalizedRoot = root.replace(/[\\/]+$/, "");
  const normalizedFile = file.replace(/^[\\/]+/, "");
  return `${normalizedRoot}/${normalizedFile}`;
}

function escapeHtml(text) {
  return String(text ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function escapeHtmlAttr(text) {
  return escapeHtml(text).replaceAll("`", "&#96;");
}

function formatTs(ts) {
  if (!Number.isFinite(ts)) return "--:--:--";
  const d = new Date(Number(ts) * 1000);
  return d.toLocaleTimeString();
}

function bindEvents() {
  startBtn.addEventListener("click", () => {
    startScrape().catch((err) => appendClientLog("error", String(err)));
  });
  reloadBtn.addEventListener("click", () => {
    loadWall(false).catch((err) => appendClientLog("error", String(err)));
  });
  wallGrid.addEventListener("click", (evt) => {
    const card = evt.target.closest(".wall-card");
    if (!card) return;
    const id = Number(card.dataset.id || 0);
    if (!id) return;
    loadDetail(id).catch((err) => appendClientLog("error", String(err)));
  });
  detailPane.addEventListener("click", (evt) => {
    const btn = evt.target.closest(".file-btn");
    if (!btn) return;
    const path = btn.dataset.play || "";
    if (!path) return;
    playPath(path).catch((err) => appendClientLog("error", String(err)));
  });
}

async function init() {
  bindEvents();
  if (!canUseTauri()) {
    wallCount.textContent = "浏览器模式";
    detailPane.className = "detail-pane empty";
    detailPane.textContent = "请通过 Tauri 启动以启用本地数据和播放器。";
    return;
  }
  await loadWall(true);
  setInterval(() => {
    pollLogs().catch((err) => appendClientLog("error", String(err)));
  }, 1200);
  setInterval(() => {
    loadWall(false).catch(() => {});
  }, 5000);
}

init().catch((err) => appendClientLog("error", String(err)));
