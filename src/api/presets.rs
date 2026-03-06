//! Preset CRUD routes.

use axum::extract::{Path, Query};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

use crate::core::presets::{self, now_iso, Preset, TextLine};
use crate::error::{AppError, AppResult};


#[derive(Deserialize)]
pub struct PresetCreateRequest {
    pub name: String,
    #[serde(default)]
    pub texts: Vec<TextLine>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub sort_order: i64,
}

#[derive(Deserialize)]
pub struct PresetUpdateRequest {
    pub name: Option<String>,
    pub texts: Option<Vec<TextLine>>,
    pub tags: Option<Vec<String>>,
    pub sort_order: Option<i64>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub tag: Option<String>,
}

pub async fn list_presets(Query(q): Query<ListQuery>) -> AppResult<Json<Vec<Preset>>> {
    let presets = presets::list_all_presets(q.tag.as_deref())?;
    Ok(Json(presets))
}

pub async fn create_preset(Json(body): Json<PresetCreateRequest>) -> AppResult<Json<Preset>> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_iso();
    let preset = Preset {
        id: id.clone(),
        name: body.name,
        texts: body.texts,
        tags: body.tags,
        sort_order: body.sort_order,
        created_at: now.clone(),
        updated_at: now,
    };
    presets::write_preset(&id, &preset)?;
    Ok(Json(preset))
}

pub async fn get_preset(Path(preset_id): Path<String>) -> AppResult<Json<Preset>> {
    let preset = presets::read_preset(&preset_id)?;
    Ok(Json(preset))
}

pub async fn update_preset(
    Path(preset_id): Path<String>,
    Json(body): Json<PresetUpdateRequest>,
) -> AppResult<Json<Preset>> {
    let mut preset = presets::read_preset(&preset_id)?;

    if let Some(name) = body.name {
        preset.name = name;
    }
    if let Some(texts) = body.texts {
        preset.texts = texts;
    }
    if let Some(tags) = body.tags {
        preset.tags = tags;
    }
    if let Some(sort_order) = body.sort_order {
        preset.sort_order = sort_order;
    }
    preset.updated_at = now_iso();

    presets::write_preset(&preset_id, &preset)?;
    Ok(Json(preset))
}

pub async fn delete_preset(Path(preset_id): Path<String>) -> AppResult<Json<JsonValue>> {
    presets::delete_preset_file(&preset_id)?;
    Ok(Json(json!({"success": true})))
}

pub async fn export_all_presets() -> AppResult<Json<Vec<Preset>>> {
    let all = presets::list_all_presets(None)?;
    Ok(Json(all))
}

pub async fn export_single_preset(Path(preset_id): Path<String>) -> AppResult<Json<Preset>> {
    let preset = presets::read_preset(&preset_id)?;
    Ok(Json(preset))
}

pub async fn import_presets(Json(body): Json<JsonValue>) -> AppResult<Json<JsonValue>> {
    let items: Vec<JsonValue> = if body.is_array() {
        body.as_array().unwrap().clone()
    } else if body.is_object() {
        vec![body]
    } else {
        return Err(AppError::BadRequest("无效的导入数据格式".into()));
    };

    let mut imported = 0;
    let mut skipped = 0;

    for item in items {
        let name = item["name"].as_str().unwrap_or("").trim();
        if name.is_empty() {
            skipped += 1;
            continue;
        }

        let id = item["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let texts: Vec<TextLine> = serde_json::from_value(
            item["texts"].clone(),
        )
        .unwrap_or_default();

        let tags: Vec<String> = item["tags"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let now = now_iso();
        let preset = Preset {
            id: id.clone(),
            name: name.to_string(),
            texts,
            tags,
            sort_order: item["sort_order"].as_i64().unwrap_or(0),
            created_at: item["created_at"]
                .as_str()
                .unwrap_or(&now)
                .to_string(),
            updated_at: now,
        };

        if presets::write_preset(&id, &preset).is_ok() {
            imported += 1;
        } else {
            skipped += 1;
        }
    }

    Ok(Json(json!({
        "success": true,
        "imported": imported,
        "skipped": skipped,
    })))
}

pub async fn batch_delete_presets(Json(body): Json<JsonValue>) -> AppResult<Json<JsonValue>> {
    let ids: Vec<String> = body["ids"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let mut deleted = 0;
    for id in &ids {
        if presets::delete_preset_file(id).is_ok() {
            deleted += 1;
        }
    }

    Ok(Json(json!({
        "success": true,
        "deleted": deleted,
    })))
}

pub async fn reorder_presets(Json(body): Json<JsonValue>) -> AppResult<Json<JsonValue>> {
    let ids: Vec<String> = body["ids"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    for (i, id) in ids.iter().enumerate() {
        if let Ok(mut preset) = presets::read_preset(id) {
            preset.sort_order = i as i64;
            preset.updated_at = now_iso();
            let _ = presets::write_preset(id, &preset);
        }
    }

    Ok(Json(json!({"success": true})))
}
