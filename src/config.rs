//! YAML configuration manager for VanceSender.
//!
//! - Thread-safe load/save with RwLock
//! - File-mtime caching to avoid redundant disk IO
//! - Atomic writes via temp-file + rename
//! - Deep merge for partial updates

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use serde_yaml::Value as YamlValue;

use crate::error::{AppError, AppResult};

// ── Paths ──────────────────────────────────────────────────────────────

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn exe_dir_public() -> PathBuf {
    exe_dir()
}

pub fn config_path() -> PathBuf {
    exe_dir().join("config.yaml")
}

pub fn data_dir() -> PathBuf {
    exe_dir().join("data")
}

pub fn presets_dir() -> PathBuf {
    data_dir().join("presets")
}

pub fn ai_history_dir() -> PathBuf {
    data_dir().join("ai_history")
}

// ── Config cache ───────────────────────────────────────────────────────

struct ConfigCache {
    data: YamlValue,
    mtime: SystemTime,
}

static CONFIG_CACHE: OnceLock<RwLock<Option<ConfigCache>>> = OnceLock::new();

fn cache_lock() -> &'static RwLock<Option<ConfigCache>> {
    CONFIG_CACHE.get_or_init(|| RwLock::new(None))
}

// ── Public API ─────────────────────────────────────────────────────────

/// Load configuration, using file-mtime cache when possible.
pub fn load_config() -> YamlValue {
    let path = config_path();
    let mut cache = cache_lock().write();

    let current_mtime = fs::metadata(&path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    if let Some(ref cached) = *cache {
        if cached.mtime == current_mtime {
            return cached.data.clone();
        }
    }

    let data = load_from_disk(&path);
    *cache = Some(ConfigCache {
        data: data.clone(),
        mtime: current_mtime,
    });
    data
}

/// Save full config to disk atomically, refreshing cache.
pub fn save_config(cfg: &YamlValue) -> AppResult<()> {
    let path = config_path();
    atomic_write_yaml(&path, cfg)?;

    let mtime = fs::metadata(&path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut cache = cache_lock().write();
    *cache = Some(ConfigCache {
        data: cfg.clone(),
        mtime,
    });
    Ok(())
}

/// Merge a partial patch into the existing config and save.
pub fn update_config(patch: &YamlValue) -> AppResult<()> {
    let mut cfg = load_config();
    deep_merge(&mut cfg, patch);
    save_config(&cfg)
}

// ── Helpers: get typed values from config ──────────────────────────────

pub fn get_str<'a>(cfg: &'a YamlValue, section: &str, key: &str) -> &'a str {
    cfg.get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

pub fn get_bool(cfg: &YamlValue, section: &str, key: &str) -> bool {
    cfg.get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn get_i64(cfg: &YamlValue, section: &str, key: &str, default: i64) -> i64 {
    cfg.get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_i64())
        .unwrap_or(default)
}

#[allow(dead_code)]
pub fn get_f64(cfg: &YamlValue, section: &str, key: &str, default: f64) -> f64 {
    cfg.get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_f64())
        .unwrap_or(default)
}

pub fn get_section(cfg: &YamlValue, section: &str) -> YamlValue {
    cfg.get(section).cloned().unwrap_or(YamlValue::Mapping(Default::default()))
}

// ── Provider helpers ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub api_base: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_model() -> String {
    "gpt-4o".to_string()
}

pub fn get_providers(cfg: &YamlValue) -> Vec<ProviderConfig> {
    cfg.get("ai")
        .and_then(|ai| ai.get("providers"))
        .and_then(|p| serde_yaml::from_value(p.clone()).ok())
        .unwrap_or_default()
}

#[allow(dead_code)]
pub fn get_provider_by_id(cfg: &YamlValue, id: &str) -> Option<ProviderConfig> {
    get_providers(cfg).into_iter().find(|p| p.id == id)
}

pub fn add_provider(provider: ProviderConfig) -> AppResult<()> {
    let mut cfg = load_config();
    let ai = cfg
        .get_mut("ai")
        .and_then(|v| v.as_mapping_mut());
    
    let ai = match ai {
        Some(m) => m,
        None => {
            let mapping = cfg.as_mapping_mut().ok_or_else(|| AppError::Internal("config is not a mapping".into()))?;
            mapping.insert(
                YamlValue::String("ai".into()),
                YamlValue::Mapping(Default::default()),
            );
            cfg.get_mut("ai").unwrap().as_mapping_mut().unwrap()
        }
    };

    let providers = ai
        .entry(YamlValue::String("providers".into()))
        .or_insert_with(|| YamlValue::Sequence(vec![]));

    if let YamlValue::Sequence(ref mut seq) = providers {
        let val = serde_yaml::to_value(&provider)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        seq.push(val);
    }

    save_config(&cfg)
}

pub fn update_provider(id: &str, patch: &serde_json::Value) -> AppResult<()> {
    let mut cfg = load_config();
    let providers = cfg
        .get_mut("ai")
        .and_then(|ai| ai.get_mut("providers"))
        .and_then(|p| p.as_sequence_mut());

    let providers = match providers {
        Some(p) => p,
        None => return Err(AppError::NotFound(format!("provider '{id}' not found"))),
    };

    let entry = providers.iter_mut().find(|p| {
        p.get("id").and_then(|v| v.as_str()) == Some(id)
    });

    match entry {
        Some(p) => {
            if let Some(name) = patch.get("name").and_then(|v| v.as_str()) {
                p["name"] = YamlValue::String(name.to_string());
            }
            if let Some(api_base) = patch.get("api_base").and_then(|v| v.as_str()) {
                p["api_base"] = YamlValue::String(api_base.to_string());
            }
            if let Some(api_key) = patch.get("api_key").and_then(|v| v.as_str()) {
                p["api_key"] = YamlValue::String(api_key.to_string());
            }
            if let Some(model) = patch.get("model").and_then(|v| v.as_str()) {
                p["model"] = YamlValue::String(model.to_string());
            }
            save_config(&cfg)
        }
        None => Err(AppError::NotFound(format!("provider '{id}' not found"))),
    }
}

pub fn delete_provider(id: &str) -> AppResult<()> {
    let mut cfg = load_config();
    let providers = cfg
        .get_mut("ai")
        .and_then(|ai| ai.get_mut("providers"))
        .and_then(|p| p.as_sequence_mut());

    match providers {
        Some(p) => {
            let len_before = p.len();
            p.retain(|v| v.get("id").and_then(|v| v.as_str()) != Some(id));
            if p.len() == len_before {
                return Err(AppError::NotFound(format!("provider '{id}' not found")));
            }
            save_config(&cfg)
        }
        None => Err(AppError::NotFound(format!("provider '{id}' not found"))),
    }
}

// ── Internal ───────────────────────────────────────────────────────────

fn load_from_disk(path: &Path) -> YamlValue {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return default_config(),
    };

    match serde_yaml::from_str::<YamlValue>(&raw) {
        Ok(mut cfg) => {
            merge_defaults(&mut cfg);
            cfg
        }
        Err(_) => default_config(),
    }
}

fn atomic_write_yaml(path: &Path, cfg: &YamlValue) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let yaml_str = serde_yaml::to_string(cfg)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let dir = path.parent().unwrap_or(Path::new("."));
    let tmp = tempfile::NamedTempFile::new_in(dir)?;
    fs::write(tmp.path(), yaml_str.as_bytes())?;
    tmp.persist(path)
        .map_err(|e| AppError::Internal(format!("atomic rename failed: {e}")))?;
    Ok(())
}

fn deep_merge(base: &mut YamlValue, patch: &YamlValue) {
    match (base, patch) {
        (YamlValue::Mapping(ref mut base_map), YamlValue::Mapping(patch_map)) => {
            for (k, v) in patch_map {
                let entry = base_map
                    .entry(k.clone())
                    .or_insert_with(|| YamlValue::Null);
                deep_merge(entry, v);
            }
        }
        (base, patch) => {
            *base = patch.clone();
        }
    }
}

fn merge_defaults(cfg: &mut YamlValue) {
    let defaults = default_config();
    // Merge defaults under cfg (cfg wins)
    let mut merged = defaults;
    deep_merge(&mut merged, cfg);
    *cfg = merged;
}

pub fn default_config() -> YamlValue {
    serde_yaml::from_str(
        r##"
server:
  host: "127.0.0.1"
  port: 8730
  lan_access: false
  token: ""
launch:
  open_webui_on_start: false
  open_intro_on_first_start: false
  onboarding_done: false
  show_console_on_start: false
  enable_tray_on_start: true
  close_action: ask
  intro_seen: false
sender:
  method: clipboard
  chat_open_key: "t"
  delay_open_chat: 450
  delay_after_paste: 160
  delay_after_send: 260
  delay_between_lines: 1800
  focus_timeout: 8000
  retry_count: 3
  retry_interval: 450
  typing_char_delay: 18
quick_overlay:
  enabled: true
  show_webui_send_status: true
  compact_mode: false
  trigger_hotkey: f7
  mouse_side_button: ""
  poll_interval_ms: 40
  theme:
    bg_opacity: 0.92
    accent_color: "#7c5cff"
    font_size: 12
public_config:
  source_url: ""
  timeout_seconds: 5
  cache_ttl_seconds: 120
ai:
  providers: []
  default_provider: ""
  system_prompt: ""
  custom_headers: {}
"##,
    )
    .unwrap()
}

// ── Config Import ──────────────────────────────────────────────────────

/// Result of importing an external config.
pub struct ImportResult {
    #[allow(dead_code)]
    pub config_merged: bool,
    pub presets_copied: usize,
}

/// Import configuration from an external `config.yaml` file (e.g. from original VanceSender).
///
/// 1. Reads and parses the external YAML.
/// 2. Deep-merges it into the current config (external values override current).
/// 3. Copies preset files from `<external_dir>/data/presets/` if they exist.
pub fn import_config_from(path: &Path) -> AppResult<ImportResult> {
    // 1. Read and parse external config
    let raw = fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("读取配置文件失败: {e}")))?;
    let external_cfg: YamlValue = serde_yaml::from_str(&raw)
        .map_err(|e| AppError::Internal(format!("解析配置文件失败: {e}")))?;

    // 2. Deep-merge into current config
    let mut current = load_config();
    deep_merge(&mut current, &external_cfg);
    save_config(&current)?;

    // 3. Copy presets if the external dir has data/presets/
    let mut presets_copied = 0usize;
    if let Some(external_dir) = path.parent() {
        let external_presets = external_dir.join("data").join("presets");
        if external_presets.is_dir() {
            let target_presets = presets_dir();
            let _ = fs::create_dir_all(&target_presets);

            if let Ok(entries) = fs::read_dir(&external_presets) {
                for entry in entries.flatten() {
                    let src = entry.path();
                    if src.is_file() {
                        let dst = target_presets.join(entry.file_name());
                        if fs::copy(&src, &dst).is_ok() {
                            presets_copied += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(ImportResult {
        config_merged: true,
        presets_copied,
    })
}

