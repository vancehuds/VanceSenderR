//! Stats routes.

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value as JsonValue};

use crate::state::SharedState;

pub async fn get_stats(State(state): State<SharedState>) -> Json<JsonValue> {
    let stats = state.stats.read().get_stats();
    Json(serde_json::to_value(stats).unwrap_or_default())
}

pub async fn reset_stats(State(state): State<SharedState>) -> Json<JsonValue> {
    state.stats.write().reset();
    Json(json!({"success": true}))
}
