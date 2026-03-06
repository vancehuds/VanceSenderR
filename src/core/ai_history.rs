/// AI generation history — persists generated text sets with starring.

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::ai_history_dir;
use crate::core::presets::TextLine;

const MAX_HISTORY: usize = 100;

fn history_file() -> PathBuf {
    ai_history_dir().join("history.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIHistoryEntry {
    pub id: String,
    pub scenario: String,
    pub texts: Vec<TextLine>,
    pub provider_id: String,
    #[serde(default)]
    pub starred: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Store {
    entries: VecDeque<AIHistoryEntry>,
}

pub fn save_generation(scenario: &str, texts: &[TextLine], provider_id: &str) {
    let mut store = load_store();
    let entry = AIHistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        scenario: scenario.to_string(),
        texts: texts.to_vec(),
        provider_id: provider_id.to_string(),
        starred: false,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    store.entries.push_front(entry);
    while store.entries.len() > MAX_HISTORY {
        // Only remove unstarred
        if let Some(pos) = store.entries.iter().rposition(|e| !e.starred) {
            store.entries.remove(pos);
        } else {
            break;
        }
    }
    save_store(&store);
}

pub fn list_history(limit: usize, offset: usize) -> Vec<AIHistoryEntry> {
    let store = load_store();
    store
        .entries
        .iter()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect()
}

pub fn toggle_star(gen_id: &str) -> bool {
    let mut store = load_store();
    if let Some(entry) = store.entries.iter_mut().find(|e| e.id == gen_id) {
        entry.starred = !entry.starred;
        let new_state = entry.starred;
        save_store(&store);
        return new_state;
    }
    false
}

pub fn delete_entry(gen_id: &str) -> bool {
    let mut store = load_store();
    let len_before = store.entries.len();
    store.entries.retain(|e| e.id != gen_id);
    if store.entries.len() < len_before {
        save_store(&store);
        return true;
    }
    false
}

pub fn clear_unstarred() -> usize {
    let mut store = load_store();
    let len_before = store.entries.len();
    store.entries.retain(|e| e.starred);
    let removed = len_before - store.entries.len();
    if removed > 0 {
        save_store(&store);
    }
    removed
}

fn load_store() -> Store {
    let path = history_file();
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Store::default(),
    }
}

fn save_store(store: &Store) {
    let path = history_file();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(store) {
        let _ = fs::write(&path, json);
    }
}
