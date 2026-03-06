//! API route assembly + auth middleware.

pub mod ai;
pub mod presets;
pub mod sender;
pub mod settings;
pub mod stats;



use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::Router;

use crate::config;
use crate::state::SharedState;

/// Build the complete API router with auth middleware.
pub fn build_router(state: SharedState) -> Router {
    let api_v1 = Router::new()
        // Sender routes
        .route("/send/single", post(sender::send_single))
        .route("/send/batch", post(sender::send_batch))
        .route("/send/stop", post(sender::stop_batch))
        .route("/send/status", get(sender::send_status))
        .route("/send/history", get(sender::get_send_history))
        .route("/send/history", delete(sender::delete_send_history))
        // Preset routes (order matters: specific before parameterized)
        .route("/presets/export", get(presets::export_all_presets))
        .route("/presets/import", post(presets::import_presets))
        .route("/presets/batch-delete", post(presets::batch_delete_presets))
        .route("/presets/reorder", post(presets::reorder_presets))
        .route("/presets", get(presets::list_presets))
        .route("/presets", post(presets::create_preset))
        .route("/presets/{preset_id}", get(presets::get_preset))
        .route("/presets/{preset_id}", patch(presets::update_preset))
        .route("/presets/{preset_id}", delete(presets::delete_preset))
        .route("/presets/{preset_id}/export", get(presets::export_single_preset))
        // AI routes
        .route("/ai/generate", post(ai::ai_generate))
        .route("/ai/generate/stream", post(ai::ai_generate_stream))
        .route("/ai/rewrite", post(ai::ai_rewrite))
        .route("/ai/test/{provider_id}", post(ai::test_ai_provider))
        .route("/ai/history", get(ai::get_ai_history))
        .route("/ai/history/{gen_id}/star", post(ai::star_ai_history))
        .route("/ai/history/{gen_id}", delete(ai::delete_ai_history))
        .route("/ai/history/clear", post(ai::clear_ai_history))
        // Settings routes
        .route("/settings", get(settings::get_settings))
        .route("/settings/sender", patch(settings::update_sender_settings))
        .route("/settings/server", patch(settings::update_server_settings))
        .route("/settings/launch", patch(settings::update_launch_settings))
        .route("/settings/ai", patch(settings::update_ai_settings))
        .route("/settings/quick-overlay", patch(settings::update_quick_overlay_settings))
        .route("/settings/update", get(settings::check_update))
        .route("/settings/public-config", get(settings::get_public_config))
        .route("/settings/notifications", get(settings::get_notifications))
        .route("/settings/desktop/state", get(settings::get_desktop_window_state))
        .route("/settings/desktop/action", post(settings::post_desktop_window_action))
        // Provider CRUD
        .route("/settings/providers", get(settings::list_providers))
        .route("/settings/providers", post(settings::create_provider))
        .route("/settings/providers/{provider_id}", patch(settings::update_provider_route))
        .route("/settings/providers/{provider_id}", delete(settings::delete_provider_route))
        // Stats routes
        .route("/stats", get(stats::get_stats))
        .route("/stats/reset", post(stats::reset_stats))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .nest("/api/v1", api_v1)
        .with_state(state)
}

/// Constant-time byte comparison to prevent timing attacks.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

/// Build a 401 Unauthorized response with proper headers and JSON body.
fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [
            (axum::http::header::WWW_AUTHENTICATE, "Bearer"),
            (axum::http::header::CONTENT_TYPE, "application/json"),
        ],
        r#"{"detail":"未授权访问，请提供有效的 Token"}"#,
    )
        .into_response()
}

/// Auth middleware — checks Bearer token with constant-time comparison.
async fn auth_middleware(
    axum::extract::State(_state): axum::extract::State<SharedState>,
    request: Request,
    next: Next,
) -> Response {
    let cfg = config::load_config();
    let token = cfg
        .get("server")
        .and_then(|s| s.get("token"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    if token.is_empty() {
        return next.run(request).await;
    }

    // Check Authorization header
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let query_token;
    let provided_token = if let Some(bearer) = auth_header.strip_prefix("Bearer ") {
        bearer.trim()
    } else {
        // Also check query parameter
        let uri = request.uri();
        let query = uri.query().unwrap_or("");
        let params: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        query_token = params
            .iter()
            .find(|(k, _)| k == "vs_token" || k == "token")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        &query_token
    };

    if ct_eq(provided_token.as_bytes(), token.as_bytes()) {
        next.run(request).await
    } else {
        unauthorized_response()
    }
}
