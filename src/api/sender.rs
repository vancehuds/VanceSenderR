//! Send routes — single + batch (SSE) + status + history.

use std::convert::Infallible;

use axum::extract::{Query, State};
use axum::response::sse::{Event, Sse};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use tokio_stream::StreamExt;

use crate::config;
use crate::core::history;
use crate::core::sender::{SenderConfig, SendProgress};


use crate::state::SharedState;

#[derive(Deserialize)]
pub struct SendSingleRequest {
    pub text: String,
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String {
    "webui".into()
}

#[derive(Deserialize)]
pub struct SendBatchRequest {
    pub texts: Vec<String>,
    pub delay_between: Option<u64>,
    #[serde(default = "default_source")]
    pub source: String,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

pub async fn send_single(
    State(state): State<SharedState>,
    Json(body): Json<SendSingleRequest>,
) -> Json<serde_json::Value> {
    let cfg = config::load_config();
    let sender_cfg = SenderConfig::from_yaml(&cfg);

    let sender = state.sender.read();
    match sender.send_single(&body.text, &sender_cfg) {
        Ok(()) => {
            drop(sender);
            history::record_send(&body.text, true, &body.source);
            state.stats.write().record_send(true, None);
            Json(json!({"success": true, "text": body.text}))
        }
        Err(e) => {
            drop(sender);
            history::record_send(&body.text, false, &body.source);
            state.stats.write().record_send(false, None);
            Json(json!({"success": false, "text": body.text, "error": e}))
        }
    }
}

pub async fn send_batch(
    State(state): State<SharedState>,
    Json(body): Json<SendBatchRequest>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let cfg = config::load_config();
    let sender_cfg = SenderConfig::from_yaml(&cfg);
    let texts = body.texts.clone();
    let delay_between = body.delay_between;
    let source = body.source.clone();

    let (tx, rx) = tokio::sync::mpsc::channel::<SendProgress>(32);

    // Spawn blocking send in background thread
    let state_clone = state.clone();
    tokio::task::spawn_blocking(move || {
        let sender = state_clone.sender.read();
        state_clone.stats.write().record_batch();

        let _ = sender.send_batch_sync(&texts, &sender_cfg, delay_between, |progress| {
            // Record to history
            if progress.status == "sent" {
                if let Some(ref text) = progress.text {
                    history::record_send(text, true, &source);
                    state_clone.stats.write().record_send(true, None);
                }
            } else if progress.status == "error" {
                if let Some(ref text) = progress.text {
                    history::record_send(text, false, &source);
                    state_clone.stats.write().record_send(false, None);
                }
            }
            let _ = tx.blocking_send(progress);
        });
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx).map(|progress| {
        let data = serde_json::to_string(&progress).unwrap_or_default();
        Ok(Event::default().data(data))
    });

    Sse::new(stream)
}

pub async fn stop_batch(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let sender = state.sender.read();
    sender.cancel();
    Json(json!({"success": true}))
}

pub async fn send_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let sender = state.sender.read();
    Json(json!({
        "sending": sender.is_sending(),
        "progress": sender.progress(),
    }))
}

pub async fn get_send_history(Query(q): Query<HistoryQuery>) -> Json<serde_json::Value> {
    let entries = history::get_history(q.limit, q.offset);
    let total = history::get_total();
    Json(json!({
        "entries": entries,
        "total": total,
    }))
}

pub async fn delete_send_history() -> Json<serde_json::Value> {
    history::clear_history();
    Json(json!({"success": true}))
}
