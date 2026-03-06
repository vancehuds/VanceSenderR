//! Send history recording — persists last N sent texts.

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::data_dir;

const MAX_HISTORY: usize = 200;

fn history_file() -> PathBuf {
    data_dir().join("send_history.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub text: String,
    pub timestamp: String,
    pub success: bool,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HistoryStore {
    entries: VecDeque<HistoryEntry>,
}

pub fn record_send(text: &str, success: bool, source: &str) {
    let mut store = load_store();
    store.entries.push_front(HistoryEntry {
        text: text.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        success,
        source: source.to_string(),
    });
    while store.entries.len() > MAX_HISTORY {
        store.entries.pop_back();
    }
    save_store(&store);
}

pub fn get_history(limit: usize, offset: usize) -> Vec<HistoryEntry> {
    let store = load_store();
    store
        .entries
        .iter()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect()
}

pub fn get_total() -> usize {
    load_store().entries.len()
}

pub fn clear_history() {
    save_store(&HistoryStore::default());
}

fn load_store() -> HistoryStore {
    let path = history_file();
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => HistoryStore::default(),
    }
}

fn save_store(store: &HistoryStore) {
    let path = history_file();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(store) {
        let _ = fs::write(&path, json);
    }
}
