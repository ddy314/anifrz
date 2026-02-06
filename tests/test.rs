use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use reqwest::Client;
use serde_json::{Value, json};

#[tokio::test]
async fn llm_basic_filename_parse_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = env::var("BINDINGS_REPORT").unwrap_or_else(|_| "test.txt".to_string());
    let sample_count: usize = env::var("SAMPLE_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let mut report_full_path = PathBuf::from(&report_path);
    if report_full_path.is_relative() {
        let cwd = env::current_dir()?;
        report_full_path = cwd.join(report_full_path);
    }

    let report_text = fs::read_to_string(&report_full_path)?;
    let samples: Vec<String> = report_text
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .take(sample_count)
        .collect();

    if samples.is_empty() {
        return Err("no samples found in test.txt".into());
    }

    // JSON Schema: array<object{title,episode,extra}> only
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

    // Prompt: keep it short, but hard constraints are enforced by `format: schema`
    let prompt = format!(
        "{}\n{}\n{}\n{}\n\n{}\n{}",
        "/no_think",
        "你是番剧文件名解析器。",
        "给定文件名列表，输出JSON数组，每项包含字段: title, episode, extra。",
        "只输出JSON，不要解释。不要输出思考过程。",
        "文件名列表:",
        samples
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{} ) {}", i + 1, s))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let base_url = env::var("OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3:4b".to_string());

    let generate_url = format!("{}/api/generate", base_url.trim_end_matches('/'));
    let chat_url = format!("{}/api/chat", base_url.trim_end_matches('/'));

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()?;

    // --- /api/generate (primary) ---
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

    println!(
        "---- LLM prompt (raw) ----\n{}\n---------------------------",
        request_body["prompt"]
    );
    println!(
        "---- Request body ----\n{}\n---------------------",
        serde_json::to_string_pretty(&request_body)
            .unwrap_or_else(|_| "<failed to serialize request body>".to_string())
    );

    let text = match post_json_string(&client, &generate_url, &request_body, &base_url).await {
        Ok(t) if !t.trim().is_empty() => t,
        _ => {
            // --- /api/chat (fallback) ---
            let chat_body = json!({
                "model": request_body["model"],
                "messages": [{"role": "user", "content": request_body["prompt"]}],
                "stream": false,
                "think": false,
                "format": request_body["format"],
                "options": request_body["options"]
            });

            println!(
                "---- Fallback chat body ----\n{}\n---------------------",
                serde_json::to_string_pretty(&chat_body)
                    .unwrap_or_else(|_| "<failed to serialize chat body>".to_string())
            );

            post_chat_content(&client, &chat_url, &chat_body, &base_url).await?
        }
    };

    println!("LLM response:\n{}", text);

    // ✅ Hard validation: must be valid JSON + must match expected shape.
    validate_parse_result_json(&text, samples.len())?;

    Ok(())
}

#[tokio::test]
async fn bgm_search_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let token = match env::var("BGM_TOKEN") {
        Ok(v) => v,
        Err(_) => {
            println!("BGM_TOKEN not set, skip");
            return Ok(());
        }
    };
    let base_url = env::var("BGM_BASE_URL").unwrap_or_else(|_| "https://api.bgm.tv".to_string());
    let keyword = env::var("BGM_KEYWORD").unwrap_or_else(|_| "GNOSIA".to_string());

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let url = format!(
        "{}/v0/search/subjects?limit=3",
        base_url.trim_end_matches('/')
    );
    let body = json!({ "keyword": keyword });
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

    println!(
        "BGM search response: {}",
        serde_json::to_string_pretty(&resp)?
    );
    let count = resp
        .get("data")
        .and_then(|v| v.as_array())
        .map(|v| v.len())
        .unwrap_or(0);
    if count == 0 {
        return Err("BGM search returned empty list".into());
    }
    Ok(())
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
    Ok(v.get("response")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string())
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

fn validate_parse_result_json(
    text: &str,
    expected_len: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1) Must be valid JSON
    let v: Value = serde_json::from_str(text).map_err(|e| {
        format!(
            "Model did not return valid JSON: {e}\n---- RAW START ----\n{text}\n---- RAW END ----"
        )
    })?;

    // 2) Must be an array of correct length
    let arr = v.as_array().ok_or("Expected top-level JSON array")?;
    if arr.len() != expected_len {
        return Err(format!(
            "Array length mismatch: got {}, expected {}",
            arr.len(),
            expected_len
        )
        .into());
    }

    // 3) Each item must be object with exactly {title, episode, extra}
    let expected_keys: BTreeSet<&str> = ["title", "episode", "extra"].into_iter().collect();

    for (i, item) in arr.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| format!("Item {i} is not an object"))?;

        let keys: BTreeSet<&str> = obj.keys().map(|k| k.as_str()).collect();
        if keys != expected_keys {
            return Err(format!(
                "Item {i} keys mismatch. got={:?}, expected={:?}. item={}",
                keys, expected_keys, item
            )
            .into());
        }

        // title: string (non-empty recommended)
        let title = obj
            .get("title")
            .and_then(|x| x.as_str())
            .ok_or_else(|| format!("Item {i}.title must be a string"))?;
        if title.trim().is_empty() {
            return Err(format!("Item {i}.title is empty").into());
        }

        // episode: string | int | null
        let episode = obj
            .get("episode")
            .ok_or_else(|| format!("Item {i} missing episode"))?;
        if !(episode.is_string() || episode.is_i64() || episode.is_u64() || episode.is_null()) {
            return Err(format!("Item {i}.episode must be string|int|null, got={episode}").into());
        }

        // extra: string | null
        let extra = obj
            .get("extra")
            .ok_or_else(|| format!("Item {i} missing extra"))?;
        if !(extra.is_string() || extra.is_null()) {
            return Err(format!("Item {i}.extra must be string|null, got={extra}").into());
        }
    }

    Ok(())
}
