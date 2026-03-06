/// Multi-provider AI client — OpenAI-compatible API endpoints.

use std::collections::HashMap;

use reqwest::Client;
use serde_json::{json, Value as JsonValue};

use crate::config::{self, ProviderConfig};
use crate::core::presets::TextLine;
use crate::error::{AppError, AppResult};

// ── Constants ──────────────────────────────────────────────────────────

const MAX_RETRIES: u32 = 2;
const RETRY_BASE_DELAY_MS: u64 = 1000;

static DEFAULT_SYSTEM_PROMPT: &str = concat!(
    "你是 FiveM 角色扮演文本生成助手。用户会描述一个场景，",
    "你需要生成一系列 /me 和 /do 命令来描述该场景。\n\n",
    "定义与边界：\n",
    "- /me 表示角色自身动作（第三人称），如 /me 缓缓推开了房门。\n",
    "- /do 表示环境或旁白描写，如 /do 门轴发出吱呀的响声。\n",
    "- /b 表示OOC对话或出戏描述\n",
    "- /e 表示表情动作，通常搭配游戏内动画\n\n",
    "输出一个JSON数组，每个元素格式 {\"type\":\"me\",\"content\":\"...\"}，",
    "type 可以是 me/do/b/e。\n\n",
    "如果你无法输出JSON，则每行一条命令，以 /me 或 /do 开头，不要编号，不要额外说明。"
);

// ── HTTP Client pool ───────────────────────────────────────────────────

static HTTP_CLIENT: std::sync::OnceLock<Client> = std::sync::OnceLock::new();

fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap()
    })
}

// ── Provider resolution ────────────────────────────────────────────────

fn resolve_provider(provider_id: Option<&str>) -> AppResult<ProviderConfig> {
    let cfg = config::load_config();
    let providers = config::get_providers(&cfg);

    if providers.is_empty() {
        return Err(AppError::BadRequest("未配置任何AI服务商".into()));
    }

    if let Some(id) = provider_id {
        if let Some(p) = providers.iter().find(|p| p.id == id) {
            return Ok(p.clone());
        }
        return Err(AppError::NotFound(format!("AI服务商 '{id}' 不存在")));
    }

    // Use default provider
    let default_id = cfg
        .get("ai")
        .and_then(|ai| ai.get("default_provider"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if !default_id.is_empty() {
        if let Some(p) = providers.iter().find(|p| p.id == default_id) {
            return Ok(p.clone());
        }
    }

    // Fallback to first provider
    Ok(providers.into_iter().next().unwrap())
}

fn get_system_prompt() -> String {
    let cfg = config::load_config();
    let custom = cfg
        .get("ai")
        .and_then(|ai| ai.get("system_prompt"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if custom.is_empty() {
        DEFAULT_SYSTEM_PROMPT.to_string()
    } else {
        custom.to_string()
    }
}

fn get_custom_headers() -> HashMap<String, String> {
    let cfg = config::load_config();
    let mut headers = HashMap::new();
    if let Some(custom) = cfg
        .get("ai")
        .and_then(|ai| ai.get("custom_headers"))
        .and_then(|v| v.as_mapping())
    {
        for (k, v) in custom {
            if let (Some(key), Some(val)) = (k.as_str(), v.as_str()) {
                let val = val.trim();
                if !val.is_empty() {
                    headers.insert(key.to_string(), val.to_string());
                }
            }
        }
    }
    headers
}

// ── Sanitization ───────────────────────────────────────────────────────

fn sanitize_ascii(s: &str) -> String {
    s.chars()
        .map(|c| {
            // Fullwidth ASCII variants → ASCII
            if ('\u{FF01}'..='\u{FF5E}').contains(&c) {
                char::from(c as u8 - 0xFE + 0x20)
            } else if c == '\u{3000}' {
                ' '
            } else {
                c
            }
        })
        .collect()
}

// ── Build messages ─────────────────────────────────────────────────────

fn build_generate_user_prompt(
    scenario: &str,
    count: Option<u32>,
    text_type: &str,
    style: Option<&str>,
) -> String {
    let mut prompt = format!("场景描述：{scenario}\n");
    if let Some(c) = count {
        prompt.push_str(&format!("请生成{c}条文本。\n"));
    }
    match text_type {
        "me_only" => prompt.push_str("只使用/me命令（type全部为me）。\n"),
        "do_only" => prompt.push_str("只使用/do命令（type全部为do）。\n"),
        _ => {} // mixed
    }
    if let Some(s) = style {
        if !s.is_empty() {
            prompt.push_str(&format!("风格要求：{s}\n"));
        }
    }
    prompt.push_str("输出JSON数组，格式：[{\"type\":\"me\",\"content\":\"...\"}, ...]");
    prompt
}

fn build_messages(system: &str, user_prompt: &str) -> Vec<JsonValue> {
    vec![
        json!({"role": "system", "content": system}),
        json!({"role": "user", "content": user_prompt}),
    ]
}

fn estimate_max_tokens(count: Option<u32>) -> u32 {
    match count {
        Some(c) if c > 15 => 3000,
        Some(c) if c > 8 => 2000,
        _ => 1200,
    }
}

// ── Parse output ───────────────────────────────────────────────────────

pub fn parse_generate_output(content: &str) -> Vec<TextLine> {
    // Try JSON array parse first
    let trimmed = content.trim();

    // Extract JSON array from markdown code block if present
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .lines()
            .skip(1)
            .take_while(|l| !l.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        trimmed.to_string()
    };

    if let Ok(items) = serde_json::from_str::<Vec<TextLine>>(&json_str) {
        return postprocess_texts(items);
    }

    // Fallback: parse line by line
    let mut texts = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (tp, content) = if let Some(rest) = line.strip_prefix("/me ") {
            ("me", rest)
        } else if let Some(rest) = line.strip_prefix("/do ") {
            ("do", rest)
        } else if let Some(rest) = line.strip_prefix("/b ") {
            ("b", rest)
        } else if let Some(rest) = line.strip_prefix("/e ") {
            ("e", rest)
        } else {
            ("me", line)
        };

        texts.push(TextLine {
            r#type: tp.to_string(),
            content: content.to_string(),
        });
    }
    postprocess_texts(texts)
}

fn postprocess_texts(mut texts: Vec<TextLine>) -> Vec<TextLine> {
    for t in &mut texts {
        t.content = t.content.trim().to_string();
        // Strip leading /me, /do prefix from content if present
        if let Some(rest) = t.content.strip_prefix("/me ") {
            t.content = rest.to_string();
        } else if let Some(rest) = t.content.strip_prefix("/do ") {
            t.content = rest.to_string();
        }
        // Normalize type
        if !["me", "do", "b", "e"].contains(&t.r#type.as_str()) {
            t.r#type = "me".to_string();
        }
    }
    texts.retain(|t| !t.content.is_empty());
    texts
}

// ── Public API ─────────────────────────────────────────────────────────

/// Generate texts (non-streaming).
pub async fn generate_texts(
    scenario: &str,
    provider_id: Option<&str>,
    count: Option<u32>,
    text_type: &str,
    style: Option<&str>,
    temperature: Option<f64>,
) -> AppResult<(Vec<TextLine>, String)> {
    let provider = resolve_provider(provider_id)?;
    let system = get_system_prompt();
    let user_prompt = build_generate_user_prompt(scenario, count, text_type, style);
    let messages = build_messages(&system, &user_prompt);
    let max_tokens = estimate_max_tokens(count);
    let temp = temperature.unwrap_or(0.8);

    let api_base = sanitize_ascii(&provider.api_base).trim_end_matches('/').to_string();
    let url = format!("{api_base}/chat/completions");

    let mut headers = reqwest::header::HeaderMap::new();
    if !provider.api_key.is_empty() {
        let key = sanitize_ascii(&provider.api_key);
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {key}").parse().unwrap(),
        );
    }
    for (k, v) in get_custom_headers() {
        if let (Ok(name), Ok(val)) =
            (k.parse::<reqwest::header::HeaderName>(), v.parse::<reqwest::header::HeaderValue>())
        {
            headers.insert(name, val);
        }
    }

    let body = json!({
        "model": provider.model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temp,
    });

    let resp = http_client()
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("AI请求失败: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "AI API 错误 ({status}): {text}"
        )));
    }

    let json: JsonValue = resp.json().await
        .map_err(|e| AppError::Internal(format!("AI响应解析失败: {e}")))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");

    let texts = parse_generate_output(content);
    if texts.is_empty() {
        return Err(AppError::Internal("AI未生成有效文本".into()));
    }

    Ok((texts, provider.id.clone()))
}

/// Generate texts with SSE streaming — yields partial content chunks.
pub async fn generate_texts_stream(
    scenario: &str,
    provider_id: Option<&str>,
    count: Option<u32>,
    text_type: &str,
    style: Option<&str>,
    temperature: Option<f64>,
) -> AppResult<(impl futures_core::Stream<Item = Result<String, AppError>>, String)> {
    let provider = resolve_provider(provider_id)?;
    let system = get_system_prompt();
    let user_prompt = build_generate_user_prompt(scenario, count, text_type, style);
    let messages = build_messages(&system, &user_prompt);
    let max_tokens = estimate_max_tokens(count);
    let temp = temperature.unwrap_or(0.8);

    let api_base = sanitize_ascii(&provider.api_base).trim_end_matches('/').to_string();
    let url = format!("{api_base}/chat/completions");

    let mut headers = reqwest::header::HeaderMap::new();
    if !provider.api_key.is_empty() {
        let key = sanitize_ascii(&provider.api_key);
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {key}").parse().unwrap(),
        );
    }
    for (k, v) in get_custom_headers() {
        if let (Ok(name), Ok(val)) =
            (k.parse::<reqwest::header::HeaderName>(), v.parse::<reqwest::header::HeaderValue>())
        {
            headers.insert(name, val);
        }
    }

    let body = json!({
        "model": provider.model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temp,
        "stream": true,
    });

    let resp = http_client()
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("AI请求失败: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "AI API 错误 ({status}): {text}"
        )));
    }

    let provider_id = provider.id.clone();
    let stream = async_stream::stream! {
        use futures_util::StreamExt;
        let mut byte_stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = byte_stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    yield Err(AppError::Internal(format!("流读取错误: {e}")));
                    break;
                }
            };
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE lines
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }
                if let Some(data) = line.strip_prefix("data: ") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(json) = serde_json::from_str::<JsonValue>(data) {
                        if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                            yield Ok(content.to_string());
                        }
                    }
                }
            }
        }
    };

    Ok((stream, provider_id))
}

/// Rewrite existing texts.
pub async fn rewrite_texts(
    texts: &[TextLine],
    provider_id: Option<&str>,
    instruction: Option<&str>,
    style: Option<&str>,
    requirements: Option<&str>,
    text_type: Option<&str>,
    temperature: Option<f64>,
) -> AppResult<(Vec<TextLine>, String)> {
    let provider = resolve_provider(provider_id)?;
    let system = get_system_prompt();

    let texts_json = serde_json::to_string(texts)
        .map_err(|e| AppError::Internal(format!("序列化失败: {e}")))?;

    let mut user_prompt = format!("请重写以下文本：\n{texts_json}\n");
    if let Some(inst) = instruction {
        if !inst.is_empty() {
            user_prompt.push_str(&format!("重写要求：{inst}\n"));
        }
    }
    if let Some(s) = style {
        if !s.is_empty() {
            user_prompt.push_str(&format!("重写风格：{s}\n"));
        }
    }
    if let Some(r) = requirements {
        if !r.is_empty() {
            user_prompt.push_str(&format!("额外要求：{r}\n"));
        }
    }
    if let Some(tt) = text_type {
        match tt {
            "me_only" => user_prompt.push_str("重写后只使用/me命令。\n"),
            "do_only" => user_prompt.push_str("重写后只使用/do命令。\n"),
            _ => {}
        }
    }
    user_prompt.push_str("输出JSON数组，格式同原文。");

    let messages = build_messages(&system, &user_prompt);
    let temp = temperature.unwrap_or(0.7);

    let api_base = sanitize_ascii(&provider.api_base).trim_end_matches('/').to_string();
    let url = format!("{api_base}/chat/completions");

    let mut headers = reqwest::header::HeaderMap::new();
    if !provider.api_key.is_empty() {
        let key = sanitize_ascii(&provider.api_key);
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {key}").parse().unwrap(),
        );
    }

    let body = json!({
        "model": provider.model,
        "messages": messages,
        "max_tokens": 2000,
        "temperature": temp,
    });

    let resp = http_client()
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("AI请求失败: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "AI API 错误 ({status}): {text}"
        )));
    }

    let json: JsonValue = resp.json().await
        .map_err(|e| AppError::Internal(format!("AI响应解析失败: {e}")))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");

    let result = parse_generate_output(content);
    if result.is_empty() {
        return Err(AppError::Internal("AI未生成有效文本".into()));
    }

    Ok((result, provider.id.clone()))
}

/// Test provider connectivity.
pub async fn test_provider(provider_id: &str) -> AppResult<JsonValue> {
    let provider = resolve_provider(Some(provider_id))?;

    let api_base = sanitize_ascii(&provider.api_base).trim_end_matches('/').to_string();
    let url = format!("{api_base}/chat/completions");

    let mut headers = reqwest::header::HeaderMap::new();
    if !provider.api_key.is_empty() {
        let key = sanitize_ascii(&provider.api_key);
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {key}").parse().unwrap(),
        );
    }

    let body = json!({
        "model": provider.model,
        "messages": [{"role": "user", "content": "reply with OK"}],
        "max_tokens": 10,
        "temperature": 0.0,
    });

    let start = std::time::Instant::now();
    let resp = http_client()
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await;

    let latency_ms = start.elapsed().as_millis();

    match resp {
        Ok(r) if r.status().is_success() => Ok(json!({
            "success": true,
            "latency_ms": latency_ms,
            "model": provider.model,
        })),
        Ok(r) => {
            let status = r.status().as_u16();
            let text = r.text().await.unwrap_or_default();
            Ok(json!({
                "success": false,
                "error": format!("HTTP {status}: {text}"),
                "latency_ms": latency_ms,
            }))
        }
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string(),
            "latency_ms": latency_ms,
        })),
    }
}
