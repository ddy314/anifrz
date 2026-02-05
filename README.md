这是一个简单的基于rust的本地番剧媒体库管理工具。

## 功能
- 扫描本地番剧文件夹，自动识别番剧信息
- 支持多种视频格式
- 后端以 JSON 形式按作品持久化存储
- 父目录名为 `sp`/`sps`/`special`/`cd` 时会直接跳过，不进入 LLM

## 技术
- 采用LLM做文件名解析（支持批处理大量文件）
- 采用BGM API获取番剧元信息

## 使用方法

### 配置

支持两种配置方式，优先级为：**环境变量 > config.toml > 默认值**  
LLM 相关配置仅读取 `config.toml`（不再从环境变量读取）。

#### 方式 1: 使用配置文件（推荐）

复制示例配置文件并编辑：
```bash
cp config.toml.example config.toml
```

编辑 `config.toml`：
```toml
[bgm]
base_url = "https://api.bgm.tv"
token = "your_bgm_token_here"  # 从 https://bgm.tv/dev/app 获取
limit = 20

[llm]
url = "http://127.0.0.1:11434"
provider = "ollama" # ollama / openai
remote_url = "" # OpenAI 兼容地址
remote_token = "" # OpenAI 兼容 Token
model = "qwen3:4b"
batch_size = 15  # 每批处理的文件名数量
match_concurrency = 4 # 匹配阶段并发数

[library]
dir = "library"  # 持久化目录
refresh_days = 7 # 评分与集信息刷新周期

[media]
min_media_size_mb = 30 # 小于该阈值的文件默认不参与匹配
```

#### 方式 2: 使用环境变量

```bash
export BGM_BASE_URL=https://api.bgm.tv
export BGM_TOKEN=your_token_here
export BGM_LIMIT=20
export BGM_TIMEOUT_SECS=20
export BGM_RETRY=0
export BGM_RETRY_DELAY_MS=500
export BGM_DEBUG=0
export BGM_TOLERATE_ERRORS=0
export LIBRARY_DIR=library
export REFRESH_DAYS=7
export MIN_MEDIA_SIZE_MB=30
```

说明：
- `BGM_DEBUG=1` 会输出每个请求的日志（便于定位网络或接口错误）
- `BGM_TOLERATE_ERRORS=1` 会在 BGM 请求失败时继续匹配其它条目
- `BGM_RETRY` / `BGM_RETRY_DELAY_MS` 控制重试次数与间隔（毫秒）
- `BGM_TIMEOUT_SECS` 控制 BGM 请求超时时间

### 运行
```bash
# 启动 Ollama 服务
ollama serve

# 一键刮削（目录内媒体将自动匹配并写入 library/）
cargo run -- scrape /path/to/media

# 生成报告（仍保留原有命令）
cargo run -- report [input.txt] [output.json]
```

## 批处理说明

当处理大量文件名时（超过 15 个），程序会自动将它们分批发送给 LLM 处理，以提高准确性：
- 默认每批处理 15 个文件名
- 可通过 `config.toml` 的 `llm.batch_size` 调整批次大小
- 对于小模型（如 qwen3:4b），建议使用较小的批次（10-15）
- 对于大模型（如 qwen2.5:32b），可以增加到 30-50

这样可以避免小模型在处理过多文件时出现遗漏或错误。
