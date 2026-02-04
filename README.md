这是一个简单的基于rust的本地番剧媒体库管理工具。

## 功能
- 扫描本地番剧文件夹，自动识别番剧信息
- 支持多种视频格式
- 提供界面进行观看和管理

## 技术
- 采用LLM做文件名解析（支持批处理大量文件）
- 采用BGM API获取番剧元信息

## 使用方法

### 配置

支持两种配置方式，优先级为：**环境变量 > config.toml > 默认值**

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
model = "qwen3:4b"
batch_size = 15  # 每批处理的文件名数量
```

#### 方式 2: 使用环境变量

```bash
export BGM_BASE_URL=https://api.bgm.tv
export BGM_TOKEN=your_token_here
export BGM_LIMIT=20
export OLLAMA_URL=http://127.0.0.1:11434
export OLLAMA_MODEL=qwen3:4b
export LLM_BATCH_SIZE=15
```

### 运行
```bash
# 启动 Ollama 服务
ollama serve

# 生成报告
cargo run -- report [input.txt] [output.json]
```

## 批处理说明

当处理大量文件名时（超过 15 个），程序会自动将它们分批发送给 LLM 处理，以提高准确性：
- 默认每批处理 15 个文件名
- 可通过 `LLM_BATCH_SIZE` 环境变量调整批次大小
- 对于小模型（如 qwen3:4b），建议使用较小的批次（10-15）
- 对于大模型（如 qwen2.5:32b），可以增加到 30-50

这样可以避免小模型在处理过多文件时出现遗漏或错误。