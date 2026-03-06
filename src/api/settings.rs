//! Settings & provider management routes.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use serde_yaml::Value as YamlValue;

use crate::config::{self, ProviderConfig};

use crate::core::public_config;
use crate::core::update_checker;
use crate::error::{AppError, AppResult};
use crate::state::SharedState;

// ── Settings GET ───────────────────────────────────────────────────────

pub async fn get_settings(State(state): State<SharedState>) -> Json<JsonValue> {
    let cfg = config::load_config();

    let host = state.runtime_host.read().clone();
    let port = *state.runtime_port.read();
    let lan_access = *state.runtime_lan_access.read();
    let lan_ips = state.runtime_lan_ips.read().clone();

    let lan_urls: Vec<String> = lan_ips
        .iter()
        .map(|ip| format!("http://{ip}:{port}"))
        .collect();

    // Mask API keys
    let providers: Vec<JsonValue> = config::get_providers(&cfg)
        .iter()
        .map(|p| {
            json!({
                "id": p.id,
                "name": p.name,
                "api_base": p.api_base,
                "api_key_set": !p.api_key.is_empty(),
                "model": p.model,
            })
        })
        .collect();

    Json(json!({
        "sender": config::get_section(&cfg, "sender"),
        "server": {
            "host": host,
            "port": port,
            "lan_access": lan_access,
            "token_set": !config::get_str(&cfg, "server", "token").is_empty(),
            "lan_urls": lan_urls,
        },
        "launch": config::get_section(&cfg, "launch"),
        "ai": {
            "providers": providers,
            "default_provider": config::get_str(&cfg, "ai", "default_provider"),
            "system_prompt": config::get_str(&cfg, "ai", "system_prompt"),
            "custom_headers": config::get_section(&cfg, "ai").get("custom_headers").cloned().unwrap_or(YamlValue::Mapping(Default::default())),
        },
        "quick_overlay": config::get_section(&cfg, "quick_overlay"),
    }))
}

// ── Settings PATCH ─────────────────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct SenderSettingsPatch {
    #[serde(flatten)]
    pub values: JsonValue,
}

pub async fn update_sender_settings(Json(body): Json<JsonValue>) -> AppResult<Json<JsonValue>> {
    let patch = serde_yaml::to_value(json!({"sender": body}))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    config::update_config(&patch)?;
    Ok(Json(json!({"success": true})))
}

pub async fn update_server_settings(Json(body): Json<JsonValue>) -> AppResult<Json<JsonValue>> {
    let patch = serde_yaml::to_value(json!({"server": body}))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    config::update_config(&patch)?;
    Ok(Json(json!({"success": true, "restart_required": true})))
}

pub async fn update_launch_settings(Json(body): Json<JsonValue>) -> AppResult<Json<JsonValue>> {
    let patch = serde_yaml::to_value(json!({"launch": body}))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    config::update_config(&patch)?;
    Ok(Json(json!({"success": true})))
}

pub async fn update_ai_settings(Json(body): Json<JsonValue>) -> AppResult<Json<JsonValue>> {
    let patch = serde_yaml::to_value(json!({"ai": body}))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    config::update_config(&patch)?;
    Ok(Json(json!({"success": true})))
}

pub async fn update_quick_overlay_settings(Json(body): Json<JsonValue>) -> AppResult<Json<JsonValue>> {
    let patch = serde_yaml::to_value(json!({"quick_overlay": body}))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    config::update_config(&patch)?;
    Ok(Json(json!({"success": true})))
}

// ── Special endpoints ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateQuery {
    #[serde(default)]
    pub include_prerelease: bool,
}

pub async fn check_update(Query(q): Query<UpdateQuery>) -> Json<JsonValue> {
    let result = update_checker::check_github_update(q.include_prerelease).await;
    Json(serde_json::to_value(result).unwrap_or_default())
}

pub async fn get_public_config() -> Json<JsonValue> {
    let result = public_config::fetch_public_config(false).await;
    Json(serde_json::to_value(result).unwrap_or_default())
}

pub async fn get_notifications(
    State(state): State<SharedState>,
    Query(q): Query<ClearQuery>,
) -> Json<JsonValue> {
    if q.clear {
        let items = state.notifications.write().drain();
        Json(json!(items))
    } else {
        let items = state.notifications.read().get_all().to_vec();
        Json(json!(items))
    }
}

#[derive(Deserialize)]
pub struct ClearQuery {
    #[serde(default)]
    pub clear: bool,
}

pub async fn get_desktop_window_state() -> Json<JsonValue> {
    Json(json!({
        "active": false,
        "maximized": false,
    }))
}

#[derive(Deserialize)]
pub struct WindowActionRequest {
    #[allow(dead_code)]
    pub action: String,
}

pub async fn post_desktop_window_action(Json(_body): Json<WindowActionRequest>) -> Json<JsonValue> {
    // Placeholder — will be connected to desktop module
    Json(json!({"success": false, "message": "native GUI handles window actions"}))
}

// ── Provider CRUD ──────────────────────────────────────────────────────

pub async fn list_providers() -> Json<JsonValue> {
    let cfg = config::load_config();
    let providers: Vec<JsonValue> = config::get_providers(&cfg)
        .iter()
        .map(|p| {
            json!({
                "id": p.id,
                "name": p.name,
                "api_base": p.api_base,
                "api_key_set": !p.api_key.is_empty(),
                "model": p.model,
            })
        })
        .collect();
    Json(json!(providers))
}

pub async fn create_provider(Json(body): Json<JsonValue>) -> AppResult<Json<JsonValue>> {
    let id = body["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let name = body["name"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("名称不能为空".into()));
    }

    let provider = ProviderConfig {
        id: id.clone(),
        name,
        api_base: body["api_base"].as_str().unwrap_or("").to_string(),
        api_key: body["api_key"].as_str().unwrap_or("").to_string(),
        model: body["model"].as_str().unwrap_or("gpt-4o").to_string(),
    };

    config::add_provider(provider)?;

    Ok(Json(json!({
        "id": id,
        "success": true,
    })))
}

pub async fn update_provider_route(
    Path(provider_id): Path<String>,
    Json(body): Json<JsonValue>,
) -> AppResult<Json<JsonValue>> {
    config::update_provider(&provider_id, &body)?;
    Ok(Json(json!({"success": true})))
}

pub async fn delete_provider_route(Path(provider_id): Path<String>) -> AppResult<Json<JsonValue>> {
    config::delete_provider(&provider_id)?;
    Ok(Json(json!({"success": true})))
}
