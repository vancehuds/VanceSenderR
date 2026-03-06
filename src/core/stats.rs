/// Send statistics tracking — in-memory with periodic JSON flush.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::data_dir;

const FLUSH_EVERY: u32 = 10;

fn stats_file() -> PathBuf {
    data_dir().join("stats.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub total_sent: u64,
    pub total_success: u64,
    pub total_failed: u64,
    pub total_batches: u64,
    #[serde(default)]
    pub preset_usage: HashMap<String, u64>,
    #[serde(default)]
    pub daily_counts: HashMap<String, u64>,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            total_sent: 0,
            total_success: 0,
            total_failed: 0,
            total_batches: 0,
            preset_usage: HashMap::new(),
            daily_counts: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsResponse {
    pub total_sent: u64,
    pub total_success: u64,
    pub total_failed: u64,
    pub total_batches: u64,
    pub success_rate: f64,
    pub most_used_presets: Vec<PresetUsage>,
    pub daily_counts: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PresetUsage {
    pub name: String,
    pub count: u64,
}

pub struct StatsTracker {
    stats: Stats,
    dirty_count: u32,
}

impl StatsTracker {
    pub fn new() -> Self {
        let stats = Self::load_from_disk();
        Self {
            stats,
            dirty_count: 0,
        }
    }

    pub fn record_send(&mut self, success: bool, preset_name: Option<&str>) {
        self.stats.total_sent += 1;
        if success {
            self.stats.total_success += 1;
        } else {
            self.stats.total_failed += 1;
        }

        // Daily count
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        *self.stats.daily_counts.entry(today).or_insert(0) += 1;

        // Trim to last 30 days
        if self.stats.daily_counts.len() > 30 {
            let mut days: Vec<String> = self.stats.daily_counts.keys().cloned().collect();
            days.sort();
            let remove_count = days.len() - 30;
            for day in &days[..remove_count] {
                self.stats.daily_counts.remove(day);
            }
        }

        // Preset usage
        if let Some(name) = preset_name {
            *self.stats.preset_usage.entry(name.to_string()).or_insert(0) += 1;
        }

        self.dirty_count += 1;
        if self.dirty_count >= FLUSH_EVERY {
            self.flush();
        }
    }

    pub fn record_batch(&mut self) {
        self.stats.total_batches += 1;
    }

    pub fn get_stats(&self) -> StatsResponse {
        let mut top_presets: Vec<(String, u64)> = self
            .stats
            .preset_usage
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        top_presets.sort_by(|a, b| b.1.cmp(&a.1));
        top_presets.truncate(5);

        let most_used = top_presets
            .into_iter()
            .map(|(name, count)| PresetUsage { name, count })
            .collect();

        let success_rate = if self.stats.total_sent > 0 {
            (self.stats.total_success as f64 / self.stats.total_sent as f64 * 1000.0).round()
                / 10.0
        } else {
            0.0
        };

        StatsResponse {
            total_sent: self.stats.total_sent,
            total_success: self.stats.total_success,
            total_failed: self.stats.total_failed,
            total_batches: self.stats.total_batches,
            success_rate,
            most_used_presets: most_used,
            daily_counts: self.stats.daily_counts.clone(),
        }
    }

    pub fn reset(&mut self) {
        self.stats = Stats::default();
        self.dirty_count = 0;
        self.flush();
    }

    pub fn flush(&mut self) {
        self.dirty_count = 0;
        let path = stats_file();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.stats) {
            let _ = fs::write(&path, json);
        }
    }

    fn load_from_disk() -> Stats {
        let path = stats_file();
        match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Stats::default(),
        }
    }
}
