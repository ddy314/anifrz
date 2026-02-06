const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke;

const statusText = document.getElementById("statusText");
const metrics = document.getElementById("metrics");
const eventLog = document.getElementById("eventLog");
const seriesList = document.getElementById("seriesList");
const rootInput = document.getElementById("rootPath");
const startBtn = document.getElementById("startBtn");
const reloadBtn = document.getElementById("reloadBtn");

let metricState = {
  matched: 0,
  processed: 0,
  total: 0,
};

function appendLog(line) {
  const now = new Date().toLocaleTimeString();
  eventLog.textContent = `[${now}] ${line}\n${eventLog.textContent}`.slice(0, 7000);
}

function decodeEvent(evt) {
  if (!evt || typeof evt !== "object") return ["Unknown", {}];
  const entry = Object.entries(evt)[0];
  if (!entry) return ["Unknown", {}];
  return entry;
}

function renderMetrics() {
  metrics.textContent = `processed ${metricState.processed}/${metricState.total} | matched ${metricState.matched}`;
}

async function startScrape() {
  const root = rootInput.value.trim();
  if (!root) {
    appendLog("请输入有效目录");
    return;
  }
  await invoke("start_scrape", { root });
  appendLog(`已启动刮削: ${root}`);
}

async function fetchSeries() {
  const list = await invoke("list_series", { limit: 120 });
  if (!Array.isArray(list) || list.length === 0) {
    seriesList.innerHTML = `<div class="series-card">暂无入库结果</div>`;
    return;
  }
  seriesList.innerHTML = list
    .map((item) => {
      const title = item.name_cn || item.name || `#${item.id}`;
      const episodes = item.local?.episodes?.length ?? 0;
      const missing = item.local?.missing_episodes?.length ?? 0;
      return `
        <article class="series-card">
          <div class="series-title">${title}</div>
          <div class="series-meta">ID ${item.id} | 本地集数 ${episodes} | 缺失 ${missing}</div>
        </article>
      `;
    })
    .join("");
}

async function poll() {
  const payload = await invoke("poll_events");
  if (!payload) return;

  for (const evt of payload.statuses ?? []) {
    const [kind, data] = decodeEvent(evt);
    if (kind === "Matching") {
      metricState.processed = data.current ?? metricState.processed;
      metricState.total = data.total ?? metricState.total;
      statusText.textContent = `匹配中 ${data.current}/${data.total}`;
      renderMetrics();
    } else if (kind === "Persisting") {
      statusText.textContent = `作品写入 ${data.current}/${data.total}`;
    } else if (kind === "Finished") {
      statusText.textContent = "已完成";
      appendLog(`完成: ${JSON.stringify(data.summary)}`);
      fetchSeries();
    } else if (kind === "Error") {
      statusText.textContent = "发生错误";
      appendLog(`错误: ${data.message}`);
    }
  }

  for (const evt of payload.data_events ?? []) {
    const [kind, data] = decodeEvent(evt);
    if (kind === "DatabaseReady") {
      appendLog(`数据库: ${data.path}`);
    } else if (kind === "MatchSaved") {
      metricState.matched = data.matched ?? metricState.matched;
      metricState.processed = data.processed ?? metricState.processed;
      metricState.total = data.total ?? metricState.total;
      renderMetrics();
      appendLog(`入库匹配: ${data.bgm_id} <- ${data.file_path}`);
    } else if (kind === "SeriesSaved") {
      appendLog(`作品已更新: ${data.id}`);
    }
  }
}

async function init() {
  if (!invoke) {
    statusText.textContent = "Tauri API 不可用";
    return;
  }
  startBtn.addEventListener("click", () => {
    startScrape().catch((err) => appendLog(`启动失败: ${String(err)}`));
  });
  reloadBtn.addEventListener("click", () => {
    fetchSeries().catch((err) => appendLog(`刷新失败: ${String(err)}`));
  });

  await fetchSeries();
  renderMetrics();
  setInterval(() => {
    poll().catch((err) => appendLog(`轮询失败: ${String(err)}`));
  }, 1000);
}

init().catch((err) => {
  statusText.textContent = "初始化失败";
  appendLog(String(err));
});
