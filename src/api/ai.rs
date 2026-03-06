//! AI routes — generate, stream, rewrite, test, history.

use std::convert::Infallible;

use axum::extract::{Path, Query};
use axum::response::sse::{Event, Sse};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

use crate::core::ai_client;
use crate::core::ai_history;
use crate::core::presets::TextLine;
use crate::error::AppResult;


#[derive(Deserialize)]
pub struct AIGenerateRequest {
    pub scenario: String,
    pub provider_id: Option<String>,
    pub count: Option<u32>,
    #[serde(default = "default_text_type")]
    pub text_type: String,
    pub style: Option<String>,
    pub temperature: Option<f64>,
}

fn default_text_type() -> String {
    "mixed".into()
}

#[derive(Deserialize)]
pub struct AIRewriteRequest {
    pub texts: Vec<TextLine>,
    pub provider_id: Option<String>,
    pub instruction: Option<String>,
    pub style: Option<String>,
    pub requirements: Option<String>,
    pub text_type: Option<String>,
    pub temperature: Option<f64>,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    20
}

pub async fn ai_generate(Json(body): Json<AIGenerateRequest>) -> AppResult<Json<JsonValue>> {
    let (texts, provider_id) = ai_client::generate_texts(
        &body.scenario,
        body.provider_id.as_deref(),
        body.count,
        &body.text_type,
        body.style.as_deref(),
        body.temperature,
    )
    .await?;

    // Save to history
    ai_history::save_generation(&body.scenario, &texts, &provider_id);

    Ok(Json(json!({
        "texts": texts,
        "provider_id": provider_id,
    })))
}

pub async fn ai_generate_stream(
    Json(body): Json<AIGenerateRequest>,
) -> AppResult<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>> {
    let (stream, provider_id) = ai_client::generate_texts_stream(
        &body.scenario,
        body.provider_id.as_deref(),
        body.count,
        &body.text_type,
        body.style.as_deref(),
        body.temperature,
    )
    .await?;

    let scenario = body.scenario.clone();
    let sse_stream = async_stream::stream! {
        use tokio_stream::StreamExt;
        let mut full_content = String::new();

        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(content) => {
                    full_content.push_str(&content);
                    let event = Event::default().data(
                        serde_json::to_string(&json!({
                            "type": "chunk",
                            "content": content,
                        })).unwrap_or_default()
                    );
                    yield Ok::<_, Infallible>(event);
                }
                Err(e) => {
                    let event = Event::default().data(
                        serde_json::to_string(&json!({
                            "type": "error",
                            "error": e.to_string(),
                        })).unwrap_or_default()
                    );
                    yield Ok(event);
                    break;
                }
            }
        }

        // Parse final result
        let texts = ai_client::parse_generate_output(&full_content);
        if !texts.is_empty() {
            ai_history::save_generation(&scenario, &texts, &provider_id);
        }

        let event = Event::default().data(
            serde_json::to_string(&json!({
                "type": "done",
                "texts": texts,
                "provider_id": provider_id,
            })).unwrap_or_default()
        );
        yield Ok(event);
    };

    Ok(Sse::new(sse_stream))
}

pub async fn ai_rewrite(Json(body): Json<AIRewriteRequest>) -> AppResult<Json<JsonValue>> {
    let (texts, provider_id) = ai_client::rewrite_texts(
        &body.texts,
        body.provider_id.as_deref(),
        body.instruction.as_deref(),
        body.style.as_deref(),
        body.requirements.as_deref(),
        body.text_type.as_deref(),
        body.temperature,
    )
    .await?;

    Ok(Json(json!({
        "texts": texts,
        "provider_id": provider_id,
    })))
}

pub async fn test_ai_provider(Path(provider_id): Path<String>) -> AppResult<Json<JsonValue>> {
    let result = ai_client::test_provider(&provider_id).await?;
    Ok(Json(result))
}

pub async fn get_ai_history(Query(q): Query<HistoryQuery>) -> Json<JsonValue> {
    let entries = ai_history::list_history(q.limit, q.offset);
    Json(json!(entries))
}

pub async fn star_ai_history(Path(gen_id): Path<String>) -> Json<JsonValue> {
    let new_state = ai_history::toggle_star(&gen_id);
    Json(json!({"starred": new_state}))
}

pub async fn delete_ai_history(Path(gen_id): Path<String>) -> Json<JsonValue> {
    let deleted = ai_history::delete_entry(&gen_id);
    Json(json!({"success": deleted}))
}

pub async fn clear_ai_history() -> Json<JsonValue> {
    let removed = ai_history::clear_unstarred();
    Json(json!({"success": true, "removed": removed}))
}
