/// GitHub update checker with caching and conditional requests.


use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::app_meta::{APP_VERSION, GITHUB_REPOSITORY};


const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const CACHE_TTL: Duration = Duration::from_secs(600);

static HTTP_CLIENT: std::sync::OnceLock<Client> = std::sync::OnceLock::new();

fn client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent("VanceSender-UpdateChecker")
            .build()
            .unwrap()
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateResult {
    pub success: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub published_at: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,
}

struct CacheEntry {
    result: UpdateResult,
    fetched_at: Instant,
}

static CACHE: std::sync::OnceLock<Mutex<Option<CacheEntry>>> = std::sync::OnceLock::new();
fn cache() -> &'static Mutex<Option<CacheEntry>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

fn normalize_version(v: &str) -> String {
    v.trim().trim_start_matches('v').trim_start_matches('V').to_string()
}

fn compare_versions(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .filter_map(|s| {
                // Strip pre-release suffix for comparison
                let num = s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>();
                num.parse().ok()
            })
            .collect()
    };
    let c = parse(current);
    let l = parse(latest);
    l > c
}

pub async fn check_github_update(include_prerelease: bool) -> UpdateResult {
    // Check cache
    {
        let lock = cache().lock().unwrap();
        if let Some(ref entry) = *lock {
            if entry.fetched_at.elapsed() < CACHE_TTL {
                return entry.result.clone();
            }
        }
    }

    let current_version = normalize_version(APP_VERSION);
    let parts: Vec<&str> = GITHUB_REPOSITORY.split('/').collect();
    if parts.len() != 2 {
        return UpdateResult {
            success: false,
            current_version,
            latest_version: None,
            update_available: false,
            release_url: None,
            published_at: None,
            message: "无效的仓库格式".into(),
            error_type: Some("config".into()),
        };
    }

    let (owner, repo) = (parts[0], parts[1]);

    // Try release API first
    let url = if include_prerelease {
        format!("https://api.github.com/repos/{owner}/{repo}/releases")
    } else {
        format!("https://api.github.com/repos/{owner}/{repo}/releases/latest")
    };

    let result = match client()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let json: JsonValue = resp.json().await.unwrap_or_default();

            let release = if include_prerelease {
                json.as_array().and_then(|arr| arr.first()).cloned()
            } else {
                Some(json)
            };

            match release {
                Some(r) => {
                    let tag = r["tag_name"].as_str().unwrap_or("");
                    let latest = normalize_version(tag);
                    let update_available = compare_versions(&current_version, &latest);
                    let release_url = r["html_url"].as_str().map(|s| s.to_string());
                    let published_at = r["published_at"].as_str().map(|s| s.to_string());

                    UpdateResult {
                        success: true,
                        current_version: current_version.clone(),
                        latest_version: Some(latest.clone()),
                        update_available,
                        release_url,
                        published_at,
                        message: if update_available {
                            format!("发现新版本 v{latest}")
                        } else {
                            "已是最新版本".into()
                        },
                        error_type: None,
                    }
                }
                None => UpdateResult {
                    success: true,
                    current_version: current_version.clone(),
                    latest_version: None,
                    update_available: false,
                    release_url: None,
                    published_at: None,
                    message: "未找到发布版本".into(),
                    error_type: None,
                },
            }
        }
        Ok(resp) => {
            let status = resp.status().as_u16();
            UpdateResult {
                success: false,
                current_version: current_version.clone(),
                latest_version: None,
                update_available: false,
                release_url: None,
                published_at: None,
                message: format!("GitHub API 错误: HTTP {status}"),
                error_type: Some("http".into()),
            }
        }
        Err(e) => UpdateResult {
            success: false,
            current_version: current_version.clone(),
            latest_version: None,
            update_available: false,
            release_url: None,
            published_at: None,
            message: format!("网络错误: {e}"),
            error_type: Some("network".into()),
        },
    };

    // Cache the result
    {
        let mut lock = cache().lock().unwrap();
        *lock = Some(CacheEntry {
            result: result.clone(),
            fetched_at: Instant::now(),
        });
    }

    result
}
