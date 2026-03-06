//! GitHub update checker with caching, conditional requests, and rate-limit awareness.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::app_meta::{APP_VERSION, GITHUB_REPOSITORY};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const CACHE_TTL: Duration = Duration::from_secs(600);
/// Default rate-limit backoff when Retry-After header is missing.
const DEFAULT_RATE_LIMIT_BACKOFF: Duration = Duration::from_secs(60);

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
}

struct CacheEntry {
    result: UpdateResult,
    fetched_at: Instant,
    etag: Option<String>,
    last_modified: Option<String>,
}

static CACHE: std::sync::OnceLock<Mutex<Option<CacheEntry>>> = std::sync::OnceLock::new();
fn cache() -> &'static Mutex<Option<CacheEntry>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

static RATE_LIMIT_UNTIL: std::sync::OnceLock<Mutex<Option<Instant>>> = std::sync::OnceLock::new();
fn rate_limit_until() -> &'static Mutex<Option<Instant>> {
    RATE_LIMIT_UNTIL.get_or_init(|| Mutex::new(None))
}

fn normalize_version(v: &str) -> String {
    v.trim().trim_start_matches('v').trim_start_matches('V').to_string()
}

fn compare_versions(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .filter_map(|s| {
                let num = s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>();
                num.parse().ok()
            })
            .collect()
    };
    let c = parse(current);
    let l = parse(latest);
    l > c
}

/// Build a failure result.
fn failure_result(
    current_version: String,
    message: String,
    error_type: &str,
    status_code: Option<u16>,
) -> UpdateResult {
    UpdateResult {
        success: false,
        current_version,
        latest_version: None,
        update_available: false,
        release_url: None,
        published_at: None,
        message,
        error_type: Some(error_type.into()),
        status_code,
    }
}

/// Store the cache entry (synchronous, no await).
fn store_cache(result: &UpdateResult, etag: Option<String>, last_modified: Option<String>) {
    let mut lock = cache().lock().unwrap();
    *lock = Some(CacheEntry {
        result: result.clone(),
        fetched_at: Instant::now(),
        etag,
        last_modified,
    });
}

/// Touch the cache entry's timestamp to extend its TTL.
fn touch_cache() {
    let mut lock = cache().lock().unwrap();
    if let Some(ref mut entry) = *lock {
        entry.fetched_at = Instant::now();
    }
}

/// Read from cache if still fresh. Returns (result, is_fresh).
/// Also returns the conditional headers for revalidation.
fn read_cache() -> (Option<UpdateResult>, bool, Option<String>, Option<String>) {
    let lock = cache().lock().unwrap();
    match *lock {
        Some(ref entry) => {
            let fresh = entry.fetched_at.elapsed() < CACHE_TTL;
            (
                Some(entry.result.clone()),
                fresh,
                entry.etag.clone(),
                entry.last_modified.clone(),
            )
        }
        None => (None, false, None, None),
    }
}

/// Check if we are currently rate-limited.
fn is_rate_limited() -> bool {
    let lock = rate_limit_until().lock().unwrap();
    match *lock {
        Some(until) => Instant::now() < until,
        None => false,
    }
}

/// Update rate-limit state from response headers.
fn update_rate_limit(headers: &reqwest::header::HeaderMap, status: u16) {
    if status != 403 && status != 429 {
        return;
    }

    let backoff = headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_RATE_LIMIT_BACKOFF);

    let mut lock = rate_limit_until().lock().unwrap();
    *lock = Some(Instant::now() + backoff);
    tracing::warn!("GitHub API rate-limited, backing off for {}s", backoff.as_secs());
}

pub async fn check_github_update(include_prerelease: bool) -> UpdateResult {
    let current_version = normalize_version(APP_VERSION);

    // Check rate limit (no await, guard dropped immediately)
    if is_rate_limited() {
        let (cached_result, _, _, _) = read_cache();
        return cached_result.unwrap_or_else(|| {
            failure_result(
                current_version.clone(),
                "GitHub API 限流中，请稍后再试".into(),
                "rate_limit",
                None,
            )
        });
    }

    // Check cache freshness (no await, guard dropped immediately)
    let (cached_result, is_fresh, cached_etag, cached_last_modified) = read_cache();
    if is_fresh {
        return cached_result.unwrap();
    }

    // Parse repository
    let parts: Vec<&str> = GITHUB_REPOSITORY.split('/').collect();
    if parts.len() != 2 {
        return failure_result(current_version, "无效的仓库格式".into(), "config", None);
    }
    let (owner, repo) = (parts[0], parts[1]);

    // ── Try release API (with conditional headers) ──────────────────
    let release_url = if include_prerelease {
        format!("https://api.github.com/repos/{owner}/{repo}/releases")
    } else {
        format!("https://api.github.com/repos/{owner}/{repo}/releases/latest")
    };

    let mut req = client()
        .get(&release_url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");

    if let Some(ref etag) = cached_etag {
        req = req.header("If-None-Match", etag.as_str());
    }
    if let Some(ref lm) = cached_last_modified {
        req = req.header("If-Modified-Since", lm.as_str());
    }

    // This is the only .await point — all mutex guards are already dropped
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let headers = resp.headers().clone();
            update_rate_limit(&headers, status);

            // 304 Not Modified — cache is still valid
            if status == 304 {
                touch_cache();
                if let Some(r) = cached_result {
                    return r;
                }
                // Shouldn't happen, but fall through to tags
            }
            // Rate-limited
            else if status == 403 || status == 429 {
                let result = failure_result(
                    current_version.clone(),
                    format!("GitHub API 限流: HTTP {status}"),
                    "rate_limit",
                    Some(status),
                );
                if let Some(r) = cached_result {
                    return r; // Prefer cached result over error
                }
                return result;
            }
            // 404 — no releases, try tags fallback below
            else if status == 404 {
                // Fall through to tags API
            }
            // Other error
            else if !resp.status().is_success() {
                let result = failure_result(
                    current_version.clone(),
                    format!("GitHub API 错误: HTTP {status}"),
                    "http",
                    Some(status),
                );
                store_cache(&result, None, None);
                return result;
            }
            // Success
            else {
                let new_etag = headers.get("etag").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
                let new_lm = headers.get("last-modified").and_then(|v| v.to_str().ok()).map(|s| s.to_string());

                let json: JsonValue = resp.json().await.unwrap_or_default();

                let release = if include_prerelease {
                    json.as_array().and_then(|arr| arr.first()).cloned()
                } else {
                    Some(json)
                };

                if let Some(r) = release {
                    let tag = r["tag_name"].as_str().unwrap_or("");
                    let latest = normalize_version(tag);
                    let update_available = compare_versions(&current_version, &latest);
                    let rl_url = r["html_url"].as_str().map(|s| s.to_string());
                    let published_at = r["published_at"].as_str().map(|s| s.to_string());

                    let result = UpdateResult {
                        success: true,
                        current_version: current_version.clone(),
                        latest_version: Some(latest.clone()),
                        update_available,
                        release_url: rl_url,
                        published_at,
                        message: if update_available {
                            format!("发现新版本 v{latest}")
                        } else {
                            "已是最新版本".into()
                        },
                        error_type: None,
                        status_code: None,
                    };

                    store_cache(&result, new_etag, new_lm);
                    return result;
                }
                // No releases in array — fall through to tags
            }
        }
        Err(e) => {
            // Network error — return cached result if available
            if let Some(r) = cached_result {
                return r;
            }
            let result = failure_result(
                current_version.clone(),
                format!("网络错误: {e}"),
                "network",
                None,
            );
            return result;
        }
    }

    // ── Tags API fallback ──────────────────────────────────────────────
    let tags_url = format!("https://api.github.com/repos/{owner}/{repo}/tags?per_page=1");

    match client()
        .get(&tags_url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            update_rate_limit(resp.headers(), status);

            if !resp.status().is_success() {
                let result = failure_result(
                    current_version,
                    format!("GitHub Tags API 错误: HTTP {status}"),
                    "http",
                    Some(status),
                );
                store_cache(&result, None, None);
                return result;
            }

            let json: JsonValue = resp.json().await.unwrap_or_default();
            let result = match json.as_array().and_then(|arr| arr.first()) {
                Some(tag) => {
                    let name = tag["name"].as_str().unwrap_or("");
                    let latest = normalize_version(name);
                    let update_available = compare_versions(&current_version, &latest);

                    UpdateResult {
                        success: true,
                        current_version,
                        latest_version: Some(latest.clone()),
                        update_available,
                        release_url: None,
                        published_at: None,
                        message: if update_available {
                            format!("发现新版本 v{latest} (tag)")
                        } else {
                            "已是最新版本".into()
                        },
                        error_type: None,
                        status_code: None,
                    }
                }
                None => UpdateResult {
                    success: true,
                    current_version,
                    latest_version: None,
                    update_available: false,
                    release_url: None,
                    published_at: None,
                    message: "未找到发布版本".into(),
                    error_type: None,
                    status_code: None,
                },
            };
            store_cache(&result, None, None);
            result
        }
        Err(e) => failure_result(
            current_version,
            format!("网络错误: {e}"),
            "network",
            None,
        ),
    }
}
