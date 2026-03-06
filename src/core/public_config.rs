//! Remote public config fetcher — GitHub-hosted announcements.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;

use crate::app_meta::GITHUB_REPOSITORY;
use crate::config;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
#[allow(dead_code)]
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(120);

static HTTP_CLIENT: std::sync::OnceLock<Client> = std::sync::OnceLock::new();

fn client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .user_agent("VanceSender-PublicConfig")
            .build()
            .unwrap()
    })
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PublicConfigResult {
    pub success: bool,
    pub visible: bool,
    pub source_url: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    #[serde(default)]
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_text: Option<String>,
}

struct CacheEntry {
    result: PublicConfigResult,
    fetched_at: Instant,
}

static CACHE: std::sync::OnceLock<Mutex<Option<CacheEntry>>> = std::sync::OnceLock::new();
fn cache_lock() -> &'static Mutex<Option<CacheEntry>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

fn default_source_url() -> String {
    format!(
        "https://raw.githubusercontent.com/{}/main/public-config.yaml",
        GITHUB_REPOSITORY
    )
}

pub async fn fetch_public_config(force_refresh: bool) -> PublicConfigResult {
    let cfg = config::load_config();
    let source_url = cfg
        .get("public_config")
        .and_then(|pc| pc.get("source_url"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(default_source_url);

    let cache_ttl_secs = cfg
        .get("public_config")
        .and_then(|pc| pc.get("cache_ttl_seconds"))
        .and_then(|v| v.as_f64())
        .unwrap_or(120.0);
    let cache_ttl = Duration::from_secs_f64(cache_ttl_secs);

    // Check cache
    if !force_refresh {
        let lock = cache_lock().lock().unwrap();
        if let Some(ref entry) = *lock {
            if entry.fetched_at.elapsed() < cache_ttl {
                return entry.result.clone();
            }
        }
    }

    let result = match client().get(&source_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let text = resp.text().await.unwrap_or_default();
            parse_remote_payload(&text, &source_url)
        }
        Ok(resp) => PublicConfigResult {
            success: false,
            visible: false,
            source_url: Some(source_url.clone()),
            message: format!("HTTP {}", resp.status()),
            ..Default::default()
        },
        Err(e) => PublicConfigResult {
            success: false,
            visible: false,
            source_url: Some(source_url.clone()),
            message: format!("请求失败: {e}"),
            ..Default::default()
        },
    };

    // Cache
    {
        let mut lock = cache_lock().lock().unwrap();
        *lock = Some(CacheEntry {
            result: result.clone(),
            fetched_at: Instant::now(),
        });
    }

    result
}

fn parse_remote_payload(raw: &str, source_url: &str) -> PublicConfigResult {
    // Try YAML parse first, then JSON
    let data: JsonValue = if let Ok(yaml) = serde_yaml::from_str::<YamlValue>(raw) {
        serde_json::to_value(yaml).unwrap_or_default()
    } else if let Ok(json) = serde_json::from_str::<JsonValue>(raw) {
        json
    } else {
        return PublicConfigResult {
            success: true,
            visible: true,
            source_url: Some(source_url.to_string()),
            title: None,
            content: Some(raw.to_string()),
            message: "text/plain".into(),
            fetched_at: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        };
    };

    let visible = data
        .get("visible")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let title = data.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
    let content = data
        .get("content")
        .map(|v| {
            if let Some(s) = v.as_str() {
                s.to_string()
            } else {
                v.to_string()
            }
        });

    let link_url = data.get("link_url").and_then(|v| v.as_str()).map(|s| s.to_string());
    let link_text = data.get("link_text").and_then(|v| v.as_str()).map(|s| s.to_string());

    PublicConfigResult {
        success: true,
        visible,
        source_url: Some(source_url.to_string()),
        title,
        content,
        message: "ok".into(),
        fetched_at: Some(chrono::Utc::now().to_rfc3339()),
        link_url,
        link_text,
    }
}
