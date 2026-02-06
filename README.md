# anifrz

本地番剧媒体库工具，**媒体刮削仍是核心流程**，前端用于展示和播放。

## 当前设计

- 核心后端：`scrape` 刮削流程（扫描 -> LLM 解析 -> BGM 匹配 -> SQLite 落库）
- 缓存能力：
  - 作品信息缓存（名称、简介、标签、评分、剧集、刷新时间）
  - 作品图片缓存（封面下载到本地 `library/covers/`，支持刷新周期）
- 前端能力：
  - 作品墙
  - 作品详情（简介、标签、评分、本地剧集、缺失剧集）
  - 点击剧集文件自动唤起系统播放器
- 日志接口：统一通过 `read_logs` 拉取（前端轮询展示）

## 启动

```bash
# 仅刮削（命令行）
cargo run -- scrape /path/to/media

# 前端应用（Electron）
cd src/ui/frontend
npm install
npm install -D electron

# 终端 1：前端 dev server
npm run dev

# 终端 2：Electron 壳（自动拉起 Rust IPC 后端）
npm run electron:dev
```

`electron:dev` 会自动执行 Rust 后端（`cargo run -- ipc`），无需再手动启动后端进程。

## 配置

优先级：`环境变量 > config.toml > 默认值`

- `BGM_TOKEN` 建议放在环境变量
- `LIBRARY_DIR` 可指定数据库目录（默认 `library`）
- `REFRESH_DAYS` 控制作品信息和封面刷新周期
