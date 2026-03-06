/// Preset CRUD — individual JSON files under `data/presets/`.

use std::fs;
use std::path::PathBuf;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::config::presets_dir;
use crate::error::{AppError, AppResult};

// ── Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLine {
    #[serde(default = "default_type")]
    pub r#type: String,
    pub content: String,
}

fn default_type() -> String {
    "me".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub texts: Vec<TextLine>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

// ── Validation ─────────────────────────────────────────────────────────

fn is_safe_id(id: &str) -> bool {
    lazy_static_regex().is_match(id)
}

fn lazy_static_regex() -> &'static Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap())
}

pub fn validate_preset_id(id: &str) -> AppResult<String> {
    let safe_id = id.trim().to_string();
    if safe_id.is_empty() || !is_safe_id(&safe_id) {
        return Err(AppError::BadRequest(format!(
            "预设 ID '{id}' 包含非法字符"
        )));
    }
    Ok(safe_id)
}

fn preset_path(id: &str) -> AppResult<PathBuf> {
    let safe_id = validate_preset_id(id)?;
    Ok(presets_dir().join(format!("{safe_id}.json")))
}

// ── CRUD ───────────────────────────────────────────────────────────────

pub fn read_preset(id: &str) -> AppResult<Preset> {
    let path = preset_path(id)?;
    if !path.exists() {
        return Err(AppError::NotFound(format!("预设 '{id}' 不存在")));
    }
    let data = fs::read_to_string(&path)?;
    let preset: Preset =
        serde_json::from_str(&data).map_err(|e| AppError::Internal(format!("预设解析失败: {e}")))?;
    Ok(preset)
}

pub fn write_preset(id: &str, preset: &Preset) -> AppResult<()> {
    let dir = presets_dir();
    fs::create_dir_all(&dir)?;
    let path = preset_path(id)?;

    let json = serde_json::to_string_pretty(preset)
        .map_err(|e| AppError::Internal(format!("序列化失败: {e}")))?;

    // Atomic write
    let tmp = tempfile::NamedTempFile::new_in(&dir)?;
    fs::write(tmp.path(), json.as_bytes())?;
    tmp.persist(&path)
        .map_err(|e| AppError::Internal(format!("原子写入失败: {e}")))?;

    Ok(())
}

pub fn list_all_presets(tag_filter: Option<&str>) -> AppResult<Vec<Preset>> {
    let dir = presets_dir();
    fs::create_dir_all(&dir)?;

    let mut presets = Vec::new();
    let entries = fs::read_dir(&dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let data = match fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let preset: Preset = match serde_json::from_str(&data) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if let Some(tag) = tag_filter {
            if !preset.tags.iter().any(|t| t == tag) {
                continue;
            }
        }

        presets.push(preset);
    }

    presets.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(presets)
}

pub fn delete_preset_file(id: &str) -> AppResult<()> {
    let path = preset_path(id)?;
    if !path.exists() {
        return Err(AppError::NotFound(format!("预设 '{id}' 不存在")));
    }
    fs::remove_file(&path)?;
    Ok(())
}

/// Create a new preset from JSON data.
pub fn create_preset(data: &serde_json::Value) -> AppResult<Preset> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_iso();

    let name = data["name"].as_str().unwrap_or("未命名").to_string();
    let tags: Vec<String> = data["tags"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let texts: Vec<TextLine> = if let Some(arr) = data["texts"].as_array() {
        serde_json::from_value(serde_json::Value::Array(arr.clone())).unwrap_or_default()
    } else {
        vec![]
    };

    let preset = Preset {
        id: id.clone(),
        name,
        texts,
        tags,
        sort_order: 0,
        created_at: now.clone(),
        updated_at: now,
    };

    write_preset(&id, &preset)?;
    Ok(preset)
}

/// Update an existing preset with partial JSON data.
pub fn update_preset(id: &str, data: &serde_json::Value) -> AppResult<Preset> {
    let mut preset = read_preset(id)?;

    if let Some(name) = data["name"].as_str() {
        preset.name = name.to_string();
    }
    if let Some(arr) = data["tags"].as_array() {
        preset.tags = arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
    }
    if let Some(arr) = data["texts"].as_array() {
        if let Ok(texts) = serde_json::from_value::<Vec<TextLine>>(serde_json::Value::Array(arr.clone())) {
            preset.texts = texts;
        }
    }
    if let Some(order) = data["sort_order"].as_i64() {
        preset.sort_order = order;
    }
    preset.updated_at = now_iso();

    write_preset(id, &preset)?;
    Ok(preset)
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}
