use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize, Clone)]
struct Config {
    #[serde(default)]
    bgm: BgmConfig,
    #[serde(default)]
    llm: LlmConfig,
}

#[derive(Debug, Deserialize, Clone)]
struct BgmConfig {
    #[serde(default = "default_bgm_base_url")]
    base_url: String,
    #[serde(default)]
    token: Option<String>,
    #[serde(default = "default_bgm_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize, Clone)]
struct LlmConfig {
    #[serde(default = "default_llm_url")]
    url: String,
    #[serde(default = "default_llm_model")]
    model: String,
    #[serde(default = "default_llm_batch_size")]
    batch_size: usize,
}

impl Default for BgmConfig {
    fn default() -> Self {
        Self {
            base_url: default_bgm_base_url(),
            token: None,
            limit: default_bgm_limit(),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            url: default_llm_url(),
            model: default_llm_model(),
            batch_size: default_llm_batch_size(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bgm: BgmConfig::default(),
            llm: LlmConfig::default(),
        }
    }
}

fn default_bgm_base_url() -> String {
    "https://api.bgm.tv".to_string()
}

fn default_bgm_limit() -> usize {
    20
}

fn default_llm_url() -> String {
    "http://127.0.0.1:11434".to_string()
}

fn default_llm_model() -> String {
    "qwen3:4b".to_string()
}

fn default_llm_batch_size() -> usize {
    15
}

fn load_config() -> Config {
    // 尝试从当前目录加载 config.toml
    let config_path = PathBuf::from("config.toml");
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(config) = toml::from_str::<Config>(&content) {
            return config;
        }
    }
    // 如果没有配置文件或解析失败，使用默认值
    Config::default()
}

fn get_string_config(env_key: &str, config_value: &str, default: &str) -> String {
    env::var(env_key).unwrap_or_else(|_| {
        if !config_value.is_empty() {
            config_value.to_string()
        } else {
            default.to_string()
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config();
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("bgm") => {
            let title = args.next().ok_or("missing title")?;
            let base_url = get_string_config("BGM_BASE_URL", &config.bgm.base_url, "https://api.bgm.tv");
            let token = env::var("BGM_TOKEN")
                .ok()
                .or_else(|| config.bgm.token.clone())
                .ok_or("missing BGM_TOKEN (set in config.toml or environment)")?;
            let result = bgm_search(&base_url, &token, &title, config.bgm.limit).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Some("report") => {
            let input_path = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("test.txt"));
            let output_path = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("report.json"));

            let samples = read_samples(&input_path)?;
            if samples.is_empty() {
                return Err("no samples found".into());
            }

            let base_url = get_string_config("OLLAMA_URL", &config.llm.url, "http://127.0.0.1:11434");
            let model = get_string_config("OLLAMA_MODEL", &config.llm.model, "qwen3:4b");
            let llm_items = llm_parse_list(&base_url, &model, &samples, config.llm.batch_size).await?;

            let bgm_base = get_string_config("BGM_BASE_URL", &config.bgm.base_url, "https://api.bgm.tv");
            let bgm_token = env::var("BGM_TOKEN")
                .ok()
                .or_else(|| config.bgm.token.clone())
                .ok_or("missing BGM_TOKEN (set in config.toml or environment)")?;

            let report = build_report(&config, &bgm_base, &bgm_token, &samples, &llm_items).await?;
            fs::write(&output_path, serde_json::to_string_pretty(&report)?)?;

            println!("report saved to {}", output_path.display());
        }
        Some("help") | None => {
            println!("Usage:");
            println!("  anifrz bgm <title>");
            println!("  anifrz report [test.txt] [report.json]");
            println!();
            println!("Configuration:");
            println!("  优先级: 环境变量 > config.toml > 默认值");
            println!();
            println!("  创建 config.toml 文件 (参考 config.toml.example):");
            println!("    [bgm]");
            println!("    base_url = \"https://api.bgm.tv\"");
            println!("    token = \"your_token_here\"");
            println!("    limit = 20");
            println!();
            println!("    [llm]");
            println!("    url = \"http://127.0.0.1:11434\"");
            println!("    model = \"qwen3:4b\"");
            println!("    batch_size = 15");
            println!();
            println!("  或使用环境变量:");
            println!("    BGM_BASE_URL, BGM_TOKEN, BGM_LIMIT");
            println!("    OLLAMA_URL, OLLAMA_MODEL, LLM_BATCH_SIZE");
        }
        Some(cmd) => {
            println!("unknown command: {cmd}");
        }
    }
    Ok(())
}

async fn bgm_search(
    base_url: &str,
    token: &str,
    title: &str,
    limit: usize,
) -> Result<Value, Box<dyn std::error::Error>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let url = format!(
        "{}/v0/search/subjects?limit={}",
        base_url.trim_end_matches('/'),
        limit
    );
    // Restrict to anime subjects to avoid matching music/blog/etc.
    let body = json!({
        "keyword": title,
        "filter": { "type": [2] }
    });
    let resp = client
        .post(url)
        .bearer_auth(token)
        .header("User-Agent", "anifrz/0.1")
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    Ok(resp)
}

fn read_samples(path: &PathBuf) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)?;
    Ok(text
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect())
}

async fn llm_parse_list(
    base_url: &str,
    model: &str,
    samples: &[String],
    batch_size: usize,
) -> Result<Vec<LlmItem>, Box<dyn std::error::Error>> {
    // 使用传入的batch_size参数

    // 如果样本数量较少，直接处理
    if samples.len() <= batch_size {
        return llm_parse_batch(base_url, model, samples, 0).await;
    }

    // 分批处理
    println!("📦 将 {} 个文件名分成多批处理（每批最多 {} 个）...", samples.len(), batch_size);
    let mut all_items = Vec::new();
    let mut batch_num = 0;
    
    for chunk in samples.chunks(batch_size) {
        batch_num += 1;
        let start_idx = (batch_num - 1) * batch_size;
        println!("  处理第 {}/{} 批 ({} 个文件)...", 
            batch_num, 
            (samples.len() + batch_size - 1) / batch_size,
            chunk.len()
        );
        
        let items = llm_parse_batch(base_url, model, chunk, start_idx).await?;
        all_items.extend(items);
    }
    
    println!("✅ 完成！共解析 {} 个文件名", all_items.len());
    Ok(all_items)
}

async fn llm_parse_batch(
    base_url: &str,
    model: &str,
    samples: &[String],
    start_index: usize,
) -> Result<Vec<LlmItem>, Box<dyn std::error::Error>> {
    let schema = json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": {
                "title":  { "type": "string" },
                "episode":{ "type": ["string", "integer", "null"] },
                "extra":  { "type": ["string", "null"] }
            },
            "required": ["title", "episode", "extra"],
            "additionalProperties": false
        }
    });

    let prompt = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n\n{} {} {}\n{}",
        "/no_think",
        "你是番剧文件名解析器。",
        "给定文件名列表，输出JSON数组，每项包含字段: title, episode, extra。",
        "只输出JSON，不要解释。不要输出思考过程。",
        format!("‼️重要: 输出的JSON数组必须包含exactly {} 个元素，与输入列表一一对应！", samples.len()),
        "每个文件名都必须对应一个JSON对象，不能遗漏或合并！",
        "文件名列表 (共",
        samples.len(),
        "个):",
        samples
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{} ) {}", start_index + i + 1, s))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let generate_url = format!("{}/api/generate", base_url.trim_end_matches('/'));
    let chat_url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()?;

    let request_body = json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "think": false,
        "format": schema,
        "options": {
            "num_ctx": 2048,
            "temperature": 0.0
        }
    });

    let text = match post_json_string(&client, &generate_url, &request_body, base_url).await {
        Ok(t) if !t.trim().is_empty() => t,
        _ => {
            let chat_body = json!({
                "model": request_body["model"],
                "messages": [{"role": "user", "content": request_body["prompt"]}],
                "stream": false,
                "think": false,
                "format": request_body["format"],
                "options": request_body["options"]
            });
            post_chat_content(&client, &chat_url, &chat_body, base_url).await?
        }
    };

    parse_llm_items(&text, samples.len())
}

async fn post_json_string(
    client: &Client,
    url: &str,
    body: &Value,
    base_url: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let resp = match client.post(url).json(body).send().await {
        Ok(r) => r,
        Err(err) => {
            if base_url.contains("localhost") {
                let fallback_url = url.replace("localhost", "127.0.0.1");
                client.post(&fallback_url).json(body).send().await?
            } else {
                return Err(err.into());
            }
        }
    }
    .error_for_status()?;

    let v: Value = resp.json().await?;
    Ok(v.get("response").and_then(|x| x.as_str()).unwrap_or("").trim().to_string())
}

async fn post_chat_content(
    client: &Client,
    url: &str,
    body: &Value,
    base_url: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let resp = match client.post(url).json(body).send().await {
        Ok(r) => r,
        Err(err) => {
            if base_url.contains("localhost") {
                let fallback_url = url.replace("localhost", "127.0.0.1");
                client.post(&fallback_url).json(body).send().await?
            } else {
                return Err(err.into());
            }
        }
    }
    .error_for_status()?;

    let v: Value = resp.json().await?;
    Ok(v.get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim()
        .to_string())
}

fn parse_llm_items(text: &str, expected_len: usize) -> Result<Vec<LlmItem>, Box<dyn std::error::Error>> {
    let v: Value = serde_json::from_str(text).map_err(|e| {
        format!(
            "Model did not return valid JSON: {e}\n---- RAW START ----\n{text}\n---- RAW END ----"
        )
    })?;
    let arr = v.as_array().ok_or("Expected top-level JSON array")?;
    if arr.len() != expected_len {
        eprintln!("⚠️  警告: LLM 返回数组长度不匹配: 得到 {}, 期望 {}", arr.len(), expected_len);
        eprintln!("这可能是因为模型无法正确解析所有文件名。");
        eprintln!("提示: 可以尝试:");
        eprintln!("  1. 使用更强大的模型 (设置 OLLAMA_MODEL 环境变量)");
        eprintln!("  2. 减少输入文件数量");
        eprintln!("  3. 简化文件名");
        eprintln!();
        eprintln!("LLM 原始响应:");
        eprintln!("{}", serde_json::to_string_pretty(&v)?);
        eprintln!();
        return Err(format!("Array length mismatch: got {}, expected {}. 模型可能无法处理这么多文件名，请尝试使用更强大的模型或减少输入文件数量。", arr.len(), expected_len).into());
    }
    let mut items = Vec::with_capacity(arr.len());
    for item in arr {
        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let episode = item.get("episode").cloned();
        let extra = item.get("extra").and_then(|v| v.as_str()).map(|s| s.to_string());
        items.push(LlmItem { title, episode, extra });
    }
    Ok(items)
}

async fn build_report(
    config: &Config,
    bgm_base: &str,
    bgm_token: &str,
    inputs: &[String],
    llm_items: &[LlmItem],
) -> Result<Report, Box<dyn std::error::Error>> {
    let llm_base = get_string_config("OLLAMA_URL", &config.llm.url, "http://127.0.0.1:11434");
    let llm_model = get_string_config("OLLAMA_MODEL", &config.llm.model, "qwen3:4b");
    let bgm_limit: usize = env::var("BGM_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(config.bgm.limit);

    let mut items = Vec::new();
    let mut final_matches = Vec::new();
    let mut matched = 0usize;
    let mut no_match = 0usize;
    let mut llm_empty = 0usize;
    for (idx, input) in inputs.iter().enumerate() {
        let llm_item = llm_items.get(idx);
        let title = llm_item.map(|i| i.title.clone()).unwrap_or_default();
        let episode = llm_item.and_then(|i| i.episode.clone());
        let extra = llm_item.and_then(|i| i.extra.clone());
        let extra_ref = extra.as_deref();
        let llm_debug = LlmDebug {
            title: title.clone(),
            episode: episode.clone(),
            extra: extra.clone(),
        };
        if title.trim().is_empty() {
            llm_empty += 1;
            let debug = ReportDebug {
                llm: llm_debug,
                search: SearchDebug::default(),
                ranking: RankingDebug::default(),
                selection: SelectionDebug {
                    method: "llm_empty".to_string(),
                    ..SelectionDebug::default()
                },
            };
            items.push(ReportItem {
                input: input.clone(),
                llm_title: title,
                llm_episode: episode,
                llm_extra: extra,
                status: "llm_empty".to_string(),
                bgm: None,
                debug: Some(debug),
            });
            continue;
        }

        let query_info = build_search_queries(&llm_base, &llm_model, input, &title).await;
        let (candidates, bgm_debug) =
            bgm_search_candidates(bgm_base, bgm_token, &query_info.queries, bgm_limit).await?;
        let input_special = merge_special_flags(
            detect_special_flags(&title),
            extra_ref.map(detect_special_flags).unwrap_or_default(),
        );
        let ranked = rank_candidates(input, &title, extra_ref, episode.as_ref(), &candidates);
        let mut tokens = extract_upper_tokens(&title);
        tokens.extend(extract_dotted_acronyms(&title));
        let has_episode = has_episode_value(episode.as_ref());
        let input_season = detect_season(&title).or_else(|| extra_ref.and_then(detect_season));
        let ranked_debug: Vec<ScoredCandidateDebug> = ranked
            .iter()
            .map(|s| {
                let name_full = format!("{} {}", s.candidate.name, s.candidate.name_cn);
                ScoredCandidateDebug {
                    score: s.score,
                    candidate: s.candidate.clone(),
                    candidate_season: detect_season(&name_full),
                    candidate_special: detect_special_flags(&name_full),
                    movie_like: is_movie_like(&name_full),
                }
            })
            .collect();
        let search_debug = SearchDebug {
            base_title: title.clone(),
            expand_used: query_info.expand_used,
            expanded_queries: query_info.expanded.clone(),
            expand_error: query_info.expand_error.clone(),
            queries: query_info.queries.clone(),
            bgm_queries: bgm_debug,
            candidates_deduped: candidates.clone(),
        };
        let ranking_debug = RankingDebug {
            tokens: tokens.clone(),
            token_filter_applied: !tokens.is_empty(),
            has_episode,
            input_season,
            input_special,
            ranked: ranked_debug,
        };
        let mut selection_debug = SelectionDebug {
            method: "no_candidate".to_string(),
            ..SelectionDebug::default()
        };

        let bgm_match = if ranked.is_empty() {
            selection_debug.method = "no_candidate".to_string();
            None
        } else if ranked.len() == 1 {
            selection_debug.method = "single_candidate".to_string();
            selection_debug.final_id = Some(ranked[0].candidate.id);
            Some(candidate_to_match(ranked[0].candidate))
        } else {
            let special_ranked: Vec<&ScoredCandidate> = if has_any_special(&input_special) {
                ranked
                    .iter()
                    .filter(|s| {
                        let name_full = format!("{} {}", s.candidate.name, s.candidate.name_cn);
                        overlaps_special(&input_special, &detect_special_flags(&name_full))
                    })
                    .collect()
            } else {
                Vec::new()
            };

            if special_ranked.len() == 1 {
                selection_debug.method = "special_only".to_string();
                selection_debug.special_filtered = true;
                selection_debug.final_id = Some(special_ranked[0].candidate.id);
                Some(candidate_to_match(special_ranked[0].candidate))
            } else {
                let base_list: Vec<BgmCandidate> = if !special_ranked.is_empty() {
                    selection_debug.special_filtered = true;
                    special_ranked
                        .iter()
                        .take(5)
                        .map(|s| s.candidate.clone())
                        .collect()
                } else {
                    ranked
                        .iter()
                        .take(5)
                        .map(|s| s.candidate.clone())
                        .collect()
                };
                selection_debug.candidates_considered =
                    base_list.iter().map(|c| c.id).collect();

                match llm_select_bgm(
                    &llm_base,
                    &llm_model,
                    input,
                    &title,
                    episode.as_ref(),
                    extra_ref,
                    &base_list,
                )
                .await
                {
                    Ok(Some(id)) => {
                        selection_debug.llm_selected_id = Some(id);
                        if let Some(found) =
                            base_list.iter().find(|c| c.id == id).map(candidate_to_match)
                        {
                            selection_debug.method = "llm_select".to_string();
                            selection_debug.final_id = Some(id);
                            Some(found)
                        } else {
                            selection_debug.method = "llm_select_fallback_top1".to_string();
                            if let Some(top) = ranked.first().map(|s| s.candidate) {
                                selection_debug.final_id = Some(top.id);
                            }
                            ranked.first().map(|s| candidate_to_match(s.candidate))
                        }
                    }
                    Ok(None) => {
                        selection_debug.method = "llm_select_none_fallback_top1".to_string();
                        if let Some(top) = ranked.first().map(|s| s.candidate) {
                            selection_debug.final_id = Some(top.id);
                        }
                        ranked.first().map(|s| candidate_to_match(s.candidate))
                    }
                    Err(err) => {
                        selection_debug.method = "llm_select_error_fallback_top1".to_string();
                        selection_debug.llm_error = Some(err.to_string());
                        if let Some(top) = ranked.first().map(|s| s.candidate) {
                            selection_debug.final_id = Some(top.id);
                        }
                        ranked.first().map(|s| candidate_to_match(s.candidate))
                    }
                }
            }
        };

        let status = if bgm_match.is_some() {
            matched += 1;
            "matched"
        } else {
            no_match += 1;
            "no_match"
        };
        if let Some(ref bgm) = bgm_match {
            final_matches.push(FinalMatch {
                input: input.clone(),
                llm_title: title.clone(),
                llm_episode: episode.clone(),
                bgm: bgm.clone(),
            });
        }

        items.push(ReportItem {
            input: input.clone(),
            llm_title: title,
            llm_episode: episode,
            llm_extra: extra,
            status: status.to_string(),
            bgm: bgm_match,
            debug: Some(ReportDebug {
                llm: llm_debug,
                search: search_debug,
                ranking: ranking_debug,
                selection: selection_debug,
            }),
        });
    }

    Ok(Report {
        summary: ReportSummary {
            total: inputs.len(),
            matched,
            no_match,
            llm_empty,
        },
        final_matches,
        items,
    })
}

#[derive(Debug, Clone)]
struct LlmItem {
    title: String,
    episode: Option<Value>,
    extra: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct BgmCandidate {
    id: i64,
    name: String,
    name_cn: String,
    date: Option<String>,
    query_index: usize,
    rank: usize,
}

#[derive(Debug, Clone, Serialize)]
struct BgmQueryDebug {
    query: String,
    response: Value,
    candidates: Vec<BgmCandidate>,
}

fn is_anime_subject(item: &Value) -> bool {
    match item.get("type") {
        Some(Value::Number(n)) => n.as_i64() == Some(2),
        Some(Value::String(s)) => s.parse::<i64>().ok() == Some(2),
        _ => false,
    }
}

fn sanitize_bgm_response(resp: &Value) -> Value {
    let mut out = resp.clone();
    if let Some(obj) = out.as_object_mut() {
        if let Some(data) = obj.get_mut("data").and_then(|v| v.as_array_mut()) {
            for item in data.iter_mut() {
                if let Some(map) = item.as_object_mut() {
                    for key in [
                        "images",
                        "tags",
                        "staff",
                        "persons",
                        "actors",
                        "producer",
                        "producers",
                        "production",
                        "productions",
                        "relations",
                        "infobox",
                    ] {
                        map.remove(key);
                    }
                }
            }
        }
    }
    out
}

struct SearchQueries {
    queries: Vec<String>,
    expand_used: bool,
    expanded: Vec<String>,
    expand_error: Option<String>,
}

fn deterministic_query_variants(_input: &str, title: &str) -> Vec<String> {
    let mut out = Vec::new();
    let base = title.trim();
    if base.is_empty() {
        return out;
    }

    let normalized = base.replace('_', " ");
    let normalized = collapse_spaces(&normalized);
    if !normalized.is_empty() && !normalized.eq_ignore_ascii_case(base) {
        out.push(normalized);
    }

    if let Some(suffix) = split_title_suffix(base) {
        out.push(suffix);
    }

    for q in expand_acronym_variants(base) {
        out.push(q);
    }

    dedupe_queries(out)
}

fn collapse_spaces(s: &str) -> String {
    let mut out = String::new();
    let mut last_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    out.trim().to_string()
}

fn split_title_suffix(title: &str) -> Option<String> {
    for sep in ['：', ':'] {
        if let Some(idx) = title.find(sep) {
            let suffix = title[idx + sep.len_utf8()..].trim();
            if !suffix.is_empty() {
                return Some(suffix.to_string());
            }
        }
    }

    // Only split on dash when surrounded by spaces to avoid breaking words.
    if let Some(idx) = title.find(" - ") {
        let suffix = title[idx + 3..].trim();
        if !suffix.is_empty() {
            return Some(suffix.to_string());
        }
    }
    None
}

fn expand_acronym_variants(title: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut variants = Vec::new();
    if title.contains("S.A.C.") || title.contains("SAC") {
        variants.push(title.replace("S.A.C.", "SAC"));
        variants.push(title.replace("S.A.C.", "Stand Alone Complex"));
        variants.push(title.replace("SAC", "Stand Alone Complex"));
    }
    if title.contains("S.A.C") {
        variants.push(title.replace("S.A.C", "SAC"));
        variants.push(title.replace("S.A.C", "Stand Alone Complex"));
    }

    // Token-only query (e.g. "SAC 2nd GIG")
    let mut token_query = Vec::new();
    token_query.extend(extract_dotted_acronyms(title));
    token_query.extend(extract_upper_tokens(title));
    token_query.retain(|t| t.len() >= 2);
    if token_query.len() >= 2 {
        variants.push(token_query.join(" "));
    }

    for v in variants {
        let v = collapse_spaces(&v);
        if !v.is_empty() && !out.iter().any(|x: &String| x.eq_ignore_ascii_case(&v)) {
            out.push(v);
        }
    }
    out
}

fn dedupe_queries(mut queries: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for q in queries.drain(..) {
        let q = q.trim().to_string();
        if q.is_empty() {
            continue;
        }
        if !out.iter().any(|v: &String| v.eq_ignore_ascii_case(&q)) {
            out.push(q);
        }
    }
    out
}

async fn build_search_queries(
    llm_base: &str,
    llm_model: &str,
    input: &str,
    title: &str,
) -> SearchQueries {
    let mut queries = Vec::new();
    let mut expanded = Vec::new();
    let mut expand_error = None;
    let base = title.trim();
    if !base.is_empty() {
        queries.push(base.to_string());
    }

    for q in deterministic_query_variants(input, title) {
        if !queries.iter().any(|v| v.eq_ignore_ascii_case(&q)) {
            queries.push(q);
        }
    }

    let expand_used = should_expand_queries(title);
    if expand_used {
        match llm_expand_queries(llm_base, llm_model, input, title).await {
            Ok(extra) => {
                expanded = extra;
                for q in expanded.iter() {
                    if q.trim().is_empty() {
                        continue;
                    }
                    let q_trim = q.trim().to_string();
                    if !queries.iter().any(|v| v.eq_ignore_ascii_case(&q_trim)) {
                        queries.push(q_trim);
                    }
                }
            }
            Err(err) => {
                expand_error = Some(err.to_string());
            }
        }
    }

    SearchQueries {
        queries,
        expand_used,
        expanded,
        expand_error,
    }
}

fn should_expand_queries(title: &str) -> bool {
    let mut ascii = 0usize;
    let mut non_ascii = 0usize;
    for ch in title.chars() {
        if ch.is_ascii() {
            ascii += 1;
        } else {
            non_ascii += 1;
        }
    }
    if ascii == 0 {
        return false;
    }
    let total = ascii + non_ascii;
    let non_ascii_ratio = non_ascii as f64 / total as f64;
    non_ascii_ratio < 0.2
}

async fn llm_expand_queries(
    base_url: &str,
    model: &str,
    input: &str,
    title: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let schema = json!({
        "type": "array",
        "items": { "type": "string" }
    });

    let prompt = format!(
        "/no_think\n\
你是动画搜索关键词生成器。\n\
给定文件名和解析标题，输出1到3个搜索关键词，优先给出日文原名/中文名/常见译名。\n\
若解析标题主要为英文，请务必至少给出一个中文或日文关键词（可以是翻译或常见译名）。\n\
只输出JSON数组字符串，不要解释，不要输出思考过程。\n\
文件名:\n{input}\n\
解析标题:\n{title}",
        input = input,
        title = title
    );

    let generate_url = format!("{}/api/generate", base_url.trim_end_matches('/'));
    let chat_url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()?;

    let request_body = json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "think": false,
        "format": schema,
        "options": {
            "num_ctx": 2048,
            "temperature": 0.0
        }
    });

    let text = match post_json_string(&client, &generate_url, &request_body, base_url).await {
        Ok(t) if !t.trim().is_empty() => t,
        _ => {
            let chat_body = json!({
                "model": request_body["model"],
                "messages": [{"role": "user", "content": request_body["prompt"]}],
                "stream": false,
                "think": false,
                "format": request_body["format"],
                "options": request_body["options"]
            });
            post_chat_content(&client, &chat_url, &chat_body, base_url).await?
        }
    };

    let v: Value = serde_json::from_str(&text).map_err(|e| {
        format!(
            "Model did not return valid JSON: {e}\n---- RAW START ----\n{text}\n---- RAW END ----"
        )
    })?;
    let arr = v.as_array().ok_or("Expected top-level JSON array")?;
    let mut out = Vec::new();
    for item in arr.iter() {
        if let Some(s) = item.as_str() {
            let s = s.trim();
            if s.is_empty() {
                continue;
            }
            if !out.iter().any(|v: &String| v.eq_ignore_ascii_case(s)) {
                out.push(s.to_string());
            }
        }
        if out.len() >= 3 {
            break;
        }
    }
    Ok(out)
}

async fn bgm_search_candidates(
    bgm_base: &str,
    bgm_token: &str,
    queries: &[String],
    limit: usize,
) -> Result<(Vec<BgmCandidate>, Vec<BgmQueryDebug>), Box<dyn std::error::Error>> {
    let mut by_id: HashMap<i64, BgmCandidate> = HashMap::new();
    let mut debug_queries = Vec::new();

    for (query_index, query) in queries.iter().enumerate() {
        let resp = bgm_search(bgm_base, bgm_token, query, limit).await?;
        let mut per_query_candidates = Vec::new();
        if let Some(arr) = resp.get("data").and_then(|v| v.as_array()) {
            for (rank, item) in arr.iter().enumerate() {
                if !is_anime_subject(item) {
                    continue;
                }
                let id = match item.get("id").and_then(|v| v.as_i64()) {
                    Some(v) => v,
                    None => continue,
                };
                let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let name_cn = item
                    .get("name_cn")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let date = item.get("date").and_then(|v| v.as_str()).map(|s| s.to_string());
                let candidate = BgmCandidate {
                    id,
                    name,
                    name_cn,
                    date,
                    query_index,
                    rank: rank + 1,
                };
                per_query_candidates.push(candidate.clone());

                let replace = match by_id.get(&id) {
                    None => true,
                    Some(existing) => {
                        if candidate.query_index < existing.query_index {
                            true
                        } else if candidate.query_index == existing.query_index {
                            candidate.rank < existing.rank
                        } else {
                            false
                        }
                    }
                };

                if replace {
                    by_id.insert(id, candidate);
                }
            }
        }
        let sanitized = sanitize_bgm_response(&resp);
        debug_queries.push(BgmQueryDebug {
            query: query.clone(),
            response: sanitized,
            candidates: per_query_candidates,
        });
    }

    Ok((by_id.into_values().collect(), debug_queries))
}

struct ScoredCandidate<'a> {
    score: i32,
    candidate: &'a BgmCandidate,
}

fn rank_candidates<'a>(
    input: &str,
    title: &str,
    extra: Option<&str>,
    episode: Option<&Value>,
    candidates: &'a [BgmCandidate],
) -> Vec<ScoredCandidate<'a>> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut pool: Vec<&BgmCandidate> = candidates.iter().collect();

    let has_episode = has_episode_value(episode);
    let episode_num = episode_number(episode);
    let input_season = detect_season(title).or_else(|| extra.and_then(detect_season));
    let input_special = merge_special_flags(
        detect_special_flags(title),
        extra.map(detect_special_flags).unwrap_or_default(),
    );
    let input_movie_like = is_movie_like(&format!("{} {}", title, extra.unwrap_or("")));
    let input_years = extract_years(&format!("{} {}", title, extra.unwrap_or("")));
    let keyword_tokens = extract_keyword_tokens(title);

    let mut strong_tokens = extract_strong_tokens(input);
    strong_tokens.extend(extract_strong_tokens(title));
    let mut strong_required = Vec::new();
    if !strong_tokens.is_empty() {
        for token in strong_tokens {
            if pool.iter().any(|c| candidate_has_token(c, &token)) {
                if !strong_required.contains(&token) {
                    strong_required.push(token);
                }
            }
        }
    }
    if !strong_required.is_empty() {
        let filtered: Vec<&BgmCandidate> = pool
            .iter()
            .copied()
            .filter(|c| strong_required.iter().all(|t| candidate_has_token(c, t)))
            .collect();
        if !filtered.is_empty() {
            pool = filtered;
        }
    }

    let mut acronym_tokens = Vec::new();
    acronym_tokens.extend(extract_upper_tokens(title));
    acronym_tokens.extend(extract_dotted_acronyms(title));
    acronym_tokens.extend(extract_upper_tokens(input));
    acronym_tokens.extend(extract_dotted_acronyms(input));
    acronym_tokens.retain(|t| t.len() >= 3);
    let acronym_stop = [
        "bdrip", "webrip", "web", "hdr", "uhd", "hevc", "x264", "x265", "aac", "flac", "opus",
        "dts", "truehd", "bd", "tv", "ova", "oad", "ass", "srt",
    ];
    acronym_tokens.retain(|t| !acronym_stop.iter().any(|s| s == t));
    let mut acronym_required = Vec::new();
    for token in acronym_tokens {
        if pool.iter().any(|c| candidate_has_token(c, &token)) && !acronym_required.contains(&token)
        {
            acronym_required.push(token);
        }
    }

    let mut scored = Vec::with_capacity(pool.len());

    for candidate in pool {
        let mut score = (candidate.query_index as i32) * 100 + candidate.rank as i32;
        let name_full = format!("{} {}", candidate.name, candidate.name_cn);
        let name_norm = normalize_ascii(&name_full);
        let title_norm = normalize_ascii(title);
        if !title_norm.is_empty() && !name_norm.is_empty() {
            let sim = dice_coefficient(&title_norm, &name_norm);
            score -= (sim * 120.0) as i32;
        }

        if !keyword_tokens.is_empty() && candidate_has_ascii_words(&name_full) {
            let missing = keyword_tokens
                .iter()
                .filter(|t| !name_norm.contains(*t))
                .count();
            if missing == keyword_tokens.len() {
                score += 400;
            } else if missing > 0 {
                score += 160;
            }
        }

        if has_episode && is_movie_like(&name_full) && !input_movie_like {
            score += 600;
        }

        let cand_special = detect_special_flags(&name_full);
        if has_any_special(&input_special) {
            if overlaps_special(&input_special, &cand_special) {
                score -= 120;
            } else if has_any_special(&cand_special) {
                score -= 20;
            } else {
                score += 200;
            }
        } else if has_episode && has_any_special(&cand_special) {
            // Episode files should avoid special/OVA/bonus entries unless input says so.
            score += 200;
        }

        if let Some(ep) = episode_num {
            if let Some(ch) = detect_chapter_number(&name_full) {
                if ch == ep {
                    score -= 600;
                } else {
                    score += 300;
                }
            }
        }

        if !input_years.is_empty() {
            let cand_years = extract_years(&name_full);
            if cand_years.iter().any(|y| !input_years.contains(y)) {
                score += 120;
            }
        } else if !extract_years(&name_full).is_empty() {
            score += 120;
        }

        if !strong_required.is_empty()
            && !strong_required.iter().all(|t| candidate_has_token(candidate, t))
        {
            score += 800;
        }
        if !acronym_required.is_empty() {
            if acronym_required.iter().all(|t| candidate_has_token(candidate, t)) {
                score -= 160;
            } else {
                score += 260;
            }
        }

        let cand_season = detect_season(&name_full);
        if let Some(n) = input_season {
            if cand_season == Some(n) {
                score -= 20;
            } else if cand_season.is_none() {
                score += 20;
            } else {
                score += 50;
            }
        } else if let Some(cs) = cand_season {
            if cs >= 2 {
                score += 30;
            }
        }

        scored.push(ScoredCandidate { score, candidate });
    }

    scored.sort_by_key(|s| s.score);
    scored
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
struct SpecialFlags {
    bonus: bool,
    ova: bool,
    special: bool,
}

fn merge_special_flags(a: SpecialFlags, b: SpecialFlags) -> SpecialFlags {
    SpecialFlags {
        bonus: a.bonus || b.bonus,
        ova: a.ova || b.ova,
        special: a.special || b.special,
    }
}

fn has_any_special(flags: &SpecialFlags) -> bool {
    flags.bonus || flags.ova || flags.special
}

fn overlaps_special(a: &SpecialFlags, b: &SpecialFlags) -> bool {
    (a.bonus && b.bonus) || (a.ova && b.ova) || (a.special && b.special)
}

fn detect_special_flags(s: &str) -> SpecialFlags {
    let lower = s.to_lowercase();
    let mut flags = SpecialFlags::default();

    if lower.contains("bonus stage")
        || lower.contains("bonus-stage")
        || lower.contains("bonusstage")
        || lower.contains("extra session")
        || lower.contains("extra episode")
        || lower.contains("ex session")
        || lower.contains("ex episode")
        || contains_word(&lower, "bonus")
        || lower.contains("ボーナス")
    {
        flags.bonus = true;
    }

    if contains_word(&lower, "ova")
        || contains_word(&lower, "oad")
        || contains_word(&lower, "oav")
        || lower.contains("オリジナルアニメ")
    {
        flags.ova = true;
    }

    if contains_word(&lower, "special")
        || contains_word(&lower, "sp")
        || lower.contains("スペシャル")
        || lower.contains("特別")
        || lower.contains("特典")
        || lower.contains("番外")
        || lower.contains("外伝")
        || lower.contains("外传")
    {
        flags.special = true;
    }

    flags
}

fn contains_word(s: &str, word: &str) -> bool {
    let mut start = 0usize;
    while let Some(pos) = s[start..].find(word) {
        let idx = start + pos;
        let before_ok = idx == 0 || !s.as_bytes()[idx - 1].is_ascii_alphanumeric();
        let after_idx = idx + word.len();
        let after_ok = after_idx >= s.len() || !s.as_bytes()[after_idx].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        start = idx + word.len();
    }
    false
}

fn normalize_ascii(s: &str) -> String {
    s.chars()
        .filter(|ch| ch.is_ascii() && ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn extract_dotted_acronyms(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    let mut expecting_dot = false;
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            current.push(ch);
            expecting_dot = true;
            continue;
        }
        if expecting_dot && (ch == '.' || ch == '·' || ch == '・' || ch == '･') {
            continue;
        }
        if current.len() >= 2 {
            let token: String = current.iter().collect::<String>().to_lowercase();
            if !out.contains(&token) {
                out.push(token);
            }
        }
        current.clear();
        expecting_dot = false;
    }
    if current.len() >= 2 {
        let token: String = current.iter().collect::<String>().to_lowercase();
        if !out.contains(&token) {
            out.push(token);
        }
    }
    out
}

fn extract_upper_tokens(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() && (ch.is_ascii_uppercase() || ch.is_ascii_digit()) {
            current.push(ch);
        } else if !current.is_empty() {
            if current.len() >= 2 && current.chars().any(|c| c.is_ascii_alphabetic()) {
                tokens.push(current.to_lowercase());
            }
            current.clear();
        }
    }
    if current.len() >= 2 && current.chars().any(|c| c.is_ascii_alphabetic()) {
        tokens.push(current.to_lowercase());
    }
    tokens
}

fn extract_strong_tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch);
        } else {
            push_strong_token(&mut out, &current);
            current.clear();
        }
    }
    push_strong_token(&mut out, &current);
    out
}

fn extract_keyword_tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let stop = [
        "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "with", "by", "from",
        "movie", "film", "season", "part", "episode", "series", "theatrical",
    ];
    let mut current = String::new();
    let push = |buf: &mut String, out: &mut Vec<String>| {
        if buf.len() >= 4 && buf.chars().all(|c| c.is_ascii_alphabetic()) {
            let token = buf.to_ascii_lowercase();
            if !stop.iter().any(|s| *s == token) && !out.contains(&token) {
                out.push(token);
            }
        }
        buf.clear();
    };
    for ch in s.chars() {
        if ch.is_ascii_alphabetic() {
            current.push(ch);
        } else {
            push(&mut current, &mut out);
        }
    }
    push(&mut current, &mut out);
    out
}

fn candidate_has_ascii_words(s: &str) -> bool {
    let mut count = 0usize;
    let mut current = 0usize;
    for ch in s.chars() {
        if ch.is_ascii_alphabetic() {
            current += 1;
        } else {
            if current >= 4 {
                count += 1;
            }
            current = 0;
        }
    }
    if current >= 4 {
        count += 1;
    }
    count > 0
}

fn push_strong_token(out: &mut Vec<String>, token: &str) {
    if token.len() < 3 || token.len() > 8 {
        return;
    }
    let lower = token.to_ascii_lowercase();
    let stoplist = [
        "x264", "x265", "h264", "h265", "hevc", "hevc10", "10bit", "8bit", "aac", "flac",
        "opus", "dts", "truehd", "bd", "bdrip", "webrip", "web", "hdr", "uhd",
    ];
    if stoplist.iter().any(|s| *s == lower) {
        return;
    }
    let mut has_alpha = false;
    let mut digit_count = 0usize;
    let mut first_digit_idx: Option<usize> = None;
    for (idx, ch) in lower.chars().enumerate() {
        if ch.is_ascii_digit() {
            digit_count += 1;
            if first_digit_idx.is_none() {
                first_digit_idx = Some(idx);
            }
        } else if ch.is_ascii_alphabetic() {
            has_alpha = true;
        }
    }
    if !has_alpha || digit_count < 2 || digit_count > 4 {
        return;
    }
    if let Some(idx) = first_digit_idx {
        if idx == 0 {
            return;
        }
    } else {
        return;
    }
    if !out.iter().any(|t| t == &lower) {
        out.push(lower);
    }
}

fn extract_years(s: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut buf = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            buf.push(ch);
        } else {
            if buf.len() == 4 {
                if let Ok(n) = buf.parse::<u32>() {
                    if (1900..=2099).contains(&n) {
                        out.push(n);
                    }
                }
            }
            buf.clear();
        }
    }
    if buf.len() == 4 {
        if let Ok(n) = buf.parse::<u32>() {
            if (1900..=2099).contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

fn episode_number(episode: Option<&Value>) -> Option<u32> {
    match episode {
        Some(Value::Number(n)) => n.as_u64().map(|v| v as u32),
        Some(Value::String(s)) => parse_first_number(s),
        _ => None,
    }
}

fn parse_first_number(s: &str) -> Option<u32> {
    let mut buf = String::new();
    let mut started = false;
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            buf.push(ch);
            started = true;
        } else if started {
            break;
        }
    }
    if buf.is_empty() {
        None
    } else {
        buf.parse::<u32>().ok()
    }
}

fn detect_chapter_number(s: &str) -> Option<u32> {
    let chars: Vec<char> = s.chars().collect();
    for i in 0..chars.len() {
        if chars[i] == '第' {
            let mut j = i + 1;
            let mut buf = String::new();
            while j < chars.len() {
                let ch = chars[j];
                if matches!(ch, '章' | '部' | '話' | '话' | '幕' | '篇') {
                    return parse_chinese_number(&buf).or_else(|| buf.parse::<u32>().ok());
                }
                buf.push(ch);
                j += 1;
            }
        }
    }
    if let Some(n) = parse_latin_part(s) {
        return Some(n);
    }
    None
}

fn parse_latin_part(s: &str) -> Option<u32> {
    let lower = s.to_lowercase();
    for key in ["chapter", "part"] {
        if let Some(idx) = lower.find(key) {
            let rest = &lower[idx + key.len()..];
            if let Some(n) = parse_first_number(rest) {
                return Some(n);
            }
        }
    }
    None
}

fn candidate_has_token(candidate: &BgmCandidate, token: &str) -> bool {
    let norm = normalize_ascii(&format!("{} {}", candidate.name, candidate.name_cn));
    norm.contains(token)
}

fn dice_coefficient(a: &str, b: &str) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a.len() == 1 || b.len() == 1 {
        return if a == b { 1.0 } else { 0.0 };
    }
    let mut bigrams_a = std::collections::HashMap::new();
    let a_chars: Vec<char> = a.chars().collect();
    for i in 0..a_chars.len() - 1 {
        let key = (a_chars[i], a_chars[i + 1]);
        *bigrams_a.entry(key).or_insert(0usize) += 1;
    }
    let b_chars: Vec<char> = b.chars().collect();
    let mut matches = 0usize;
    for i in 0..b_chars.len() - 1 {
        let key = (b_chars[i], b_chars[i + 1]);
        if let Some(count) = bigrams_a.get_mut(&key) {
            if *count > 0 {
                *count -= 1;
                matches += 1;
            }
        }
    }
    let total = (a_chars.len() - 1) + (b_chars.len() - 1);
    (2.0 * matches as f32) / (total as f32)
}

fn has_episode_value(episode: Option<&Value>) -> bool {
    match episode {
        None => false,
        Some(Value::Null) => false,
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Number(_)) => true,
        _ => false,
    }
}

fn is_movie_like(name: &str) -> bool {
    let lower = name.to_lowercase();
    let keywords = [
        "劇場版",
        "剧场版",
        "映画",
        "movie",
        "the movie",
        "ova",
        "oad",
        "特典",
        "总集",
        "総集",
        "special",
        "外伝",
        "外传",
        "剧场",
        "theatrical",
    ];
    keywords.iter().any(|k| lower.contains(k))
}

fn candidate_to_match(c: &BgmCandidate) -> BgmMatch {
    BgmMatch {
        id: Some(c.id),
        name: Some(c.name.clone()),
        name_cn: Some(c.name_cn.clone()),
        date: c.date.clone(),
    }
}

async fn llm_select_bgm(
    base_url: &str,
    model: &str,
    input: &str,
    title: &str,
    episode: Option<&Value>,
    extra: Option<&str>,
    candidates: &[BgmCandidate],
) -> Result<Option<i64>, Box<dyn std::error::Error>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": { "type": ["integer", "null"] }
        },
        "required": ["id"],
        "additionalProperties": false
    });

    let episode_text = episode
        .and_then(|v| {
            if v.is_string() {
                v.as_str().map(|s| s.to_string())
            } else if v.is_i64() {
                v.as_i64().map(|n| n.to_string())
            } else if v.is_u64() {
                v.as_u64().map(|n| n.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "".to_string());

    let mut candidate_lines = String::new();
    for (i, c) in candidates.iter().enumerate() {
        let date = c.date.clone().unwrap_or_else(|| "".to_string());
        candidate_lines.push_str(&format!(
            "{} ) id={} | name={} | name_cn={} | date={}\n",
            i + 1,
            c.id,
            c.name,
            c.name_cn,
            date
        ));
    }

    let prompt = format!(
        "/no_think\n\
你是番剧条目匹配器。\n\
根据输入文件名和解析结果，从候选条目中选择最符合的一条。\n\
只输出JSON对象：{{\"id\": <候选id或null>}}。不要解释，不要输出思考过程。\n\
输入文件名:\n{input}\n\
解析标题:\n{title}\n\
集数:\n{episode_text}\n\
额外信息:\n{extra}\n\
候选列表:\n{candidate_lines}",
        input = input,
        title = title,
        episode_text = episode_text,
        extra = extra.unwrap_or(""),
        candidate_lines = candidate_lines
    );

    let generate_url = format!("{}/api/generate", base_url.trim_end_matches('/'));
    let chat_url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()?;

    let request_body = json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "think": false,
        "format": schema,
        "options": {
            "num_ctx": 2048,
            "temperature": 0.0
        }
    });

    let text = match post_json_string(&client, &generate_url, &request_body, base_url).await {
        Ok(t) if !t.trim().is_empty() => t,
        _ => {
            let chat_body = json!({
                "model": request_body["model"],
                "messages": [{"role": "user", "content": request_body["prompt"]}],
                "stream": false,
                "think": false,
                "format": request_body["format"],
                "options": request_body["options"]
            });
            post_chat_content(&client, &chat_url, &chat_body, base_url).await?
        }
    };

    let v: Value = serde_json::from_str(&text).map_err(|e| {
        format!(
            "Model did not return valid JSON: {e}\n---- RAW START ----\n{text}\n---- RAW END ----"
        )
    })?;
    let id_val = v.get("id");
    let id = match id_val {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.parse::<i64>().ok(),
        Some(Value::Null) | None => None,
        _ => None,
    };
    Ok(id)
}

fn detect_season(s: &str) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let lower = s.to_lowercase();

    if let Some(idx) = lower.find("season") {
        if let Some(n) = parse_digits_after(&lower[idx + "season".len()..]) {
            return Some(n);
        }
    }

    // s2 / s02
    let bytes = lower.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        if bytes[i] == b's' {
            let prev_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            if prev_ok {
                if let Some(n) = parse_digits_after_bytes(bytes, i + 1) {
                    return Some(n);
                }
            }
        }
    }

    // 第2期 / 第2季 / 第2部
    if let Some(n) = parse_chinese_season(&lower) {
        return Some(n);
    }

    // 2nd / 3rd / 4th
    if let Some(n) = parse_ordinal(&lower) {
        return Some(n);
    }

    None
}

fn parse_digits_after(s: &str) -> Option<u32> {
    let mut num = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            num.push(ch);
        } else if !num.is_empty() {
            break;
        }
    }
    if num.is_empty() {
        None
    } else {
        num.parse().ok()
    }
}

fn parse_digits_after_bytes(bytes: &[u8], start: usize) -> Option<u32> {
    let mut num: u32 = 0;
    let mut found = false;
    let mut i = start;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_digit() {
            found = true;
            num = num.saturating_mul(10).saturating_add((b - b'0') as u32);
            i += 1;
            continue;
        }
        break;
    }
    if found { Some(num) } else { None }
}

fn parse_chinese_season(s: &str) -> Option<u32> {
    let chars: Vec<char> = s.chars().collect();
    for i in 0..chars.len() {
        if chars[i] == '第' {
            let mut j = i + 1;
            let mut num_buf = String::new();
            while j < chars.len() {
                let ch = chars[j];
                if ch == '期' || ch == '季' || ch == '部' || ch == '章' {
                    return parse_chinese_number(&num_buf).or_else(|| num_buf.parse().ok());
                }
                num_buf.push(ch);
                j += 1;
            }
        }
    }
    None
}

fn parse_chinese_number(s: &str) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let map = |ch| match ch {
        '零' => Some(0),
        '一' => Some(1),
        '二' | '两' => Some(2),
        '三' => Some(3),
        '四' => Some(4),
        '五' => Some(5),
        '六' => Some(6),
        '七' => Some(7),
        '八' => Some(8),
        '九' => Some(9),
        '十' => Some(10),
        _ => None,
    };

    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 1 {
        return map(chars[0]);
    }
    if chars.len() == 2 && chars[0] == '十' {
        return map(chars[1]).map(|v| 10 + v);
    }
    if chars.len() == 2 && chars[1] == '十' {
        return map(chars[0]).map(|v| v * 10);
    }
    if chars.len() == 3 && chars[1] == '十' {
        let tens = map(chars[0]).unwrap_or(0);
        let ones = map(chars[2]).unwrap_or(0);
        return Some(tens * 10 + ones);
    }
    None
}

fn parse_ordinal(s: &str) -> Option<u32> {
    let bytes = s.as_bytes();
    for i in 0..bytes.len().saturating_sub(2) {
        if bytes[i].is_ascii_digit() {
            let mut j = i;
            let mut num: u32 = 0;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                num = num.saturating_mul(10).saturating_add((bytes[j] - b'0') as u32);
                j += 1;
            }
            if j + 1 < bytes.len() {
                let b1 = bytes[j];
                let b2 = bytes[j + 1];
                let is_suffix = (b1 == b's' && b2 == b't')
                    || (b1 == b'n' && b2 == b'd')
                    || (b1 == b'r' && b2 == b'd')
                    || (b1 == b't' && b2 == b'h');
                if is_suffix {
                    return Some(num);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detect_season_variants() {
        assert_eq!(detect_season("Season 2"), Some(2));
        assert_eq!(detect_season("S3"), Some(3));
        assert_eq!(detect_season("第2期"), Some(2));
        assert_eq!(detect_season("第十季"), Some(10));
        assert_eq!(detect_season("2nd Season"), Some(2));
        assert_eq!(detect_season("Sousou no Frieren"), None);
    }

    #[test]
    fn rank_prefers_base_when_no_season() {
        let candidates = vec![
            BgmCandidate {
                id: 1,
                name: "葬送のフリーレン 第2期".to_string(),
                name_cn: "葬送的芙莉莲 第二季".to_string(),
                date: Some("2026-01-16".to_string()),
                query_index: 0,
                rank: 1,
            },
            BgmCandidate {
                id: 2,
                name: "葬送のフリーレン".to_string(),
                name_cn: "葬送的芙莉莲".to_string(),
                date: Some("2023-09-29".to_string()),
                query_index: 0,
                rank: 2,
            },
        ];

        let ranked = rank_candidates("Sousou no Frieren", "Sousou no Frieren", None, None, &candidates);
        assert_eq!(ranked.first().unwrap().candidate.id, 2);
    }

    #[test]
    fn rank_prefers_season_when_input_has_season() {
        let candidates = vec![
            BgmCandidate {
                id: 1,
                name: "葬送のフリーレン 第2期".to_string(),
                name_cn: "葬送的芙莉莲 第二季".to_string(),
                date: Some("2026-01-16".to_string()),
                query_index: 0,
                rank: 1,
            },
            BgmCandidate {
                id: 2,
                name: "葬送のフリーレン".to_string(),
                name_cn: "葬送的芙莉莲".to_string(),
                date: Some("2023-09-29".to_string()),
                query_index: 0,
                rank: 2,
            },
        ];

        let ranked = rank_candidates(
            "Sousou no Frieren Season 2",
            "Sousou no Frieren Season 2",
            None,
            None,
            &candidates,
        );
        assert_eq!(ranked.first().unwrap().candidate.id, 1);
    }

    #[test]
    fn rank_prefers_acronym_match() {
        let candidates = vec![
            BgmCandidate {
                id: 10,
                name: "Welcome to the Show".to_string(),
                name_cn: "幕间休息".to_string(),
                date: Some("2003-07-18".to_string()),
                query_index: 0,
                rank: 1,
            },
            BgmCandidate {
                id: 995,
                name: "N・H・Kにようこそ！".to_string(),
                name_cn: "欢迎加入NHK！".to_string(),
                date: Some("2006-07-09".to_string()),
                query_index: 1,
                rank: 1,
            },
        ];

        let ranked = rank_candidates(
            "Welcome to the NHK",
            "Welcome to the NHK",
            None,
            Some(&json!("13")),
            &candidates,
        );
        assert_eq!(ranked.first().unwrap().candidate.id, 995);
    }

    #[test]
    fn rank_prefers_bonus_stage_when_input_has_bonus() {
        let candidates = vec![
            BgmCandidate {
                id: 1,
                name: "この素晴らしい世界に祝福を！3".to_string(),
                name_cn: "为美好的世界献上祝福！第三季".to_string(),
                date: Some("2024-04-10".to_string()),
                query_index: 0,
                rank: 2,
            },
            BgmCandidate {
                id: 2,
                name: "この素晴らしい世界に祝福を！３ーBONUS STAGEー".to_string(),
                name_cn: "为美好的世界献上祝福！第三季 ーBONUS STAGEー".to_string(),
                date: Some("2025-04-25".to_string()),
                query_index: 0,
                rank: 1,
            },
        ];

        let ranked = rank_candidates(
            "Kono Subarashii Sekai ni Shukufuku o 3 Bonus Stage",
            "Kono Subarashii Sekai ni Shukufuku o 3 Bonus Stage",
            None,
            Some(&json!("E02")),
            &candidates,
        );
        assert_eq!(ranked.first().unwrap().candidate.id, 2);
    }

    #[test]
    fn rank_avoids_extra_session_for_regular_episode() {
        let candidates = vec![
            BgmCandidate {
                id: 1,
                name: "cowboy bebop Extra Session".to_string(),
                name_cn: "".to_string(),
                date: Some("2005-01-28".to_string()),
                query_index: 0,
                rank: 1,
            },
            BgmCandidate {
                id: 2,
                name: "カウボーイビバップ".to_string(),
                name_cn: "星际牛仔".to_string(),
                date: Some("1998-04-03".to_string()),
                query_index: 0,
                rank: 2,
            },
        ];

        let ranked = rank_candidates(
            "Cowboy Bebop",
            "Cowboy Bebop",
            None,
            Some(&json!("11")),
            &candidates,
        );
        assert_eq!(ranked.first().unwrap().candidate.id, 2);
    }

    #[test]
    fn rank_prefers_strong_token_match() {
        let candidates = vec![
            BgmCandidate {
                id: 1,
                name: "アイドルマスター シンデレラガールズ".to_string(),
                name_cn: "偶像大师 灰姑娘女孩".to_string(),
                date: Some("2015-01-08".to_string()),
                query_index: 0,
                rank: 1,
            },
            BgmCandidate {
                id: 2,
                name: "アイドルマスター シンデレラガールズ U149".to_string(),
                name_cn: "偶像大师 灰姑娘女孩 U149".to_string(),
                date: Some("2023-04-05".to_string()),
                query_index: 0,
                rank: 2,
            },
        ];

        let ranked = rank_candidates(
            "[Group] THE IDOLM@STER CINDERELLA GIRLS U149 [02].mkv",
            "THE IDOLM@STER CINDERELLA GIRLS",
            None,
            Some(&json!("02")),
            &candidates,
        );
        assert_eq!(ranked.first().unwrap().candidate.id, 2);
    }

    #[test]
    fn rank_prefers_chapter_match_for_movie_series() {
        let candidates = vec![
            BgmCandidate {
                id: 10,
                name: "劇場版 空の境界 未来福音".to_string(),
                name_cn: "剧场版 空之境界 未来福音".to_string(),
                date: Some("2013-09-28".to_string()),
                query_index: 0,
                rank: 1,
            },
            BgmCandidate {
                id: 11,
                name: "劇場版 空の境界 第一章 俯瞰風景".to_string(),
                name_cn: "剧场版 空之境界 第一章 俯瞰风景".to_string(),
                date: Some("2007-12-01".to_string()),
                query_index: 0,
                rank: 2,
            },
            BgmCandidate {
                id: 12,
                name: "劇場版 空の境界 第二章 殺人考察(前)".to_string(),
                name_cn: "剧场版 空之境界 第二章 杀人考察（前）".to_string(),
                date: Some("2008-02-09".to_string()),
                query_index: 0,
                rank: 3,
            },
        ];

        let ranked = rank_candidates(
            "[Kara_no_Kyoukai][02].mkv",
            "Kara_no_Kyoukai",
            None,
            Some(&json!("02")),
            &candidates,
        );
        assert_eq!(ranked.first().unwrap().candidate.id, 12);
    }
}

#[derive(Serialize)]
struct Report {
    summary: ReportSummary,
    final_matches: Vec<FinalMatch>,
    items: Vec<ReportItem>,
}

#[derive(Serialize)]
struct ReportSummary {
    total: usize,
    matched: usize,
    no_match: usize,
    llm_empty: usize,
}

#[derive(Serialize)]
struct ReportDebug {
    llm: LlmDebug,
    search: SearchDebug,
    ranking: RankingDebug,
    selection: SelectionDebug,
}

#[derive(Serialize)]
struct LlmDebug {
    title: String,
    episode: Option<Value>,
    extra: Option<String>,
}

#[derive(Serialize)]
struct SearchDebug {
    base_title: String,
    expand_used: bool,
    expanded_queries: Vec<String>,
    expand_error: Option<String>,
    queries: Vec<String>,
    bgm_queries: Vec<BgmQueryDebug>,
    candidates_deduped: Vec<BgmCandidate>,
}

impl Default for SearchDebug {
    fn default() -> Self {
        Self {
            base_title: String::new(),
            expand_used: false,
            expanded_queries: Vec::new(),
            expand_error: None,
            queries: Vec::new(),
            bgm_queries: Vec::new(),
            candidates_deduped: Vec::new(),
        }
    }
}

#[derive(Serialize)]
struct RankingDebug {
    tokens: Vec<String>,
    token_filter_applied: bool,
    has_episode: bool,
    input_season: Option<u32>,
    input_special: SpecialFlags,
    ranked: Vec<ScoredCandidateDebug>,
}

impl Default for RankingDebug {
    fn default() -> Self {
        Self {
            tokens: Vec::new(),
            token_filter_applied: false,
            has_episode: false,
            input_season: None,
            input_special: SpecialFlags::default(),
            ranked: Vec::new(),
        }
    }
}

#[derive(Serialize)]
struct ScoredCandidateDebug {
    score: i32,
    candidate: BgmCandidate,
    candidate_season: Option<u32>,
    candidate_special: SpecialFlags,
    movie_like: bool,
}

#[derive(Serialize)]
struct SelectionDebug {
    method: String,
    special_filtered: bool,
    llm_selected_id: Option<i64>,
    llm_error: Option<String>,
    final_id: Option<i64>,
    candidates_considered: Vec<i64>,
}

impl Default for SelectionDebug {
    fn default() -> Self {
        Self {
            method: "unset".to_string(),
            special_filtered: false,
            llm_selected_id: None,
            llm_error: None,
            final_id: None,
            candidates_considered: Vec::new(),
        }
    }
}

#[derive(Serialize)]
struct ReportItem {
    input: String,
    llm_title: String,
    llm_episode: Option<Value>,
    llm_extra: Option<String>,
    status: String,
    bgm: Option<BgmMatch>,
    debug: Option<ReportDebug>,
}

#[derive(Serialize, Clone)]
struct FinalMatch {
    input: String,
    llm_title: String,
    llm_episode: Option<Value>,
    bgm: BgmMatch,
}

#[derive(Serialize, Clone)]
struct BgmMatch {
    id: Option<i64>,
    name: Option<String>,
    name_cn: Option<String>,
    date: Option<String>,
}
