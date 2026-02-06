const { app, BrowserWindow, ipcMain } = require("electron");
const fs = require("fs");
const http = require("http");
const https = require("https");
const path = require("path");
const readline = require("readline");
const { spawn } = require("child_process");

const repoRoot = path.resolve(__dirname, "../../../..");
const devServerUrl = process.env.ANIFRZ_DEV_URL || process.env.VITE_DEV_SERVER_URL;
const openDevtools = process.env.ANIFRZ_OPEN_DEVTOOLS === "1";

let backend = null;
let backendReader = null;
let nextRequestId = 1;
const pending = new Map();

function resolveBackendCommand() {
  if (process.env.ANIFRZ_BACKEND_BIN) {
    return { cmd: process.env.ANIFRZ_BACKEND_BIN, args: ["ipc"] };
  }
  if (process.env.ANIFRZ_BACKEND_MODE === "cargo" || devServerUrl) {
    return { cmd: "cargo", args: ["run", "--", "ipc"] };
  }
  const debugBin =
    process.platform === "win32"
      ? path.join(repoRoot, "target", "debug", "anifrz.exe")
      : path.join(repoRoot, "target", "debug", "anifrz");

  if (fs.existsSync(debugBin)) {
    return { cmd: debugBin, args: ["ipc"] };
  }
  return { cmd: "cargo", args: ["run", "--", "ipc"] };
}

function ensureBackend() {
  if (backend) {
    return;
  }

  const { cmd, args } = resolveBackendCommand();
  backend = spawn(cmd, args, {
    cwd: repoRoot,
    stdio: ["pipe", "pipe", "pipe"],
  });
  const backendErrReader = readline.createInterface({ input: backend.stderr });
  backendErrReader.on("line", (line) => {
    if (line.trim()) {
      console.error(`[anifrz-backend] ${line}`);
    }
  });

  backendReader = readline.createInterface({ input: backend.stdout });
  backendReader.on("line", (line) => {
    let message;
    try {
      message = JSON.parse(line);
    } catch (_) {
      return;
    }

    const job = pending.get(message.id);
    if (!job) {
      return;
    }
    pending.delete(message.id);
    if (message.ok) {
      job.resolve(message.result ?? null);
    } else {
      job.reject(new Error(message.error || "backend invoke failed"));
    }
  });

  backend.on("close", (code, signal) => {
    backend = null;
    backendErrReader.close();
    if (backendReader) {
      backendReader.close();
      backendReader = null;
    }
    const reason = `backend exited (code=${code}, signal=${signal || "none"})`;
    for (const job of pending.values()) {
      job.reject(new Error(reason));
    }
    pending.clear();
  });

  backend.on("error", (err) => {
    backendErrReader.close();
    const reason = `backend spawn failed: ${err.message}`;
    for (const job of pending.values()) {
      job.reject(new Error(reason));
    }
    pending.clear();
  });
}

function invokeBackend(cmd, payload) {
  ensureBackend();
  if (!backend || !backend.stdin.writable) {
    return Promise.reject(new Error("backend not available"));
  }
  const id = nextRequestId++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    backend.stdin.write(`${JSON.stringify({ id, cmd, payload: payload ?? {} })}\n`);
  });
}

function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function pingUrl(url) {
  return new Promise((resolve) => {
    const client = url.startsWith("https:") ? https : http;
    const req = client.get(url, (res) => {
      res.resume();
      resolve(res.statusCode >= 200 && res.statusCode < 500);
    });
    req.on("error", () => resolve(false));
    req.setTimeout(1200, () => {
      req.destroy();
      resolve(false);
    });
  });
}

async function loadDevUrlWithRetry(win, url, maxAttempts = 80, delayMs = 300) {
  let lastError = null;
  for (let i = 0; i < maxAttempts; i += 1) {
    if (win.isDestroyed()) {
      return false;
    }
    const reachable = await pingUrl(url);
    if (!reachable) {
      await sleep(delayMs);
      continue;
    }
    try {
      await win.loadURL(url);
      return true;
    } catch (err) {
      lastError = err;
      await sleep(delayMs);
    }
  }
  if (lastError) {
    console.error(`[electron] failed to load dev server after retries: ${String(lastError)}`);
  }
  return false;
}

async function createMainWindow() {
  const win = new BrowserWindow({
    width: 1366,
    height: 860,
    minWidth: 1000,
    minHeight: 680,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  win.webContents.on("did-fail-load", (_event, code, desc, validatedUrl) => {
    console.error(`[electron] did-fail-load code=${code} url=${validatedUrl} desc=${desc}`);
  });

  if (devServerUrl) {
    const ok = await loadDevUrlWithRetry(win, devServerUrl);
    if (!ok && !win.isDestroyed()) {
      const html = [
        "<html><body style='font-family: sans-serif; padding: 24px; background:#111; color:#eee;'>",
        "<h2>Dev server is not ready</h2>",
        "<p>Run <code>npm run dev</code> in <code>src/ui/frontend</code>, then reload this window.</p>",
        `<p>Expected URL: <code>${devServerUrl}</code></p>`,
        "</body></html>",
      ].join("");
      await win.loadURL(`data:text/html;charset=utf-8,${encodeURIComponent(html)}`);
    }
  } else {
    await win.loadFile(path.join(__dirname, "..", "dist", "index.html"));
  }
  win.show();
  if (openDevtools && !win.webContents.isDevToolsOpened()) {
    win.webContents.openDevTools({ mode: "detach" });
  }
}

ipcMain.handle("anifrz:invoke", async (_event, cmd, payload) => {
  return invokeBackend(cmd, payload);
});

app.whenReady().then(() => {
  ensureBackend();
  createMainWindow().catch((err) => {
    console.error(`[electron] createMainWindow failed: ${String(err)}`);
  });

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createMainWindow();
    }
  });
});

app.on("before-quit", () => {
  if (backend && !backend.killed) {
    backend.kill();
  }
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});
