/// Shared application state accessible by both native GUI and HTTP API.

use std::sync::Arc;

use parking_lot::RwLock;

use crate::core::notifications::NotificationQueue;
use crate::core::sender::KeyboardSender;
use crate::core::stats::StatsTracker;

/// Shared state wrapped in Arc for cross-thread access.
pub type SharedState = Arc<AppState>;

pub struct AppState {
    pub sender: RwLock<KeyboardSender>,
    pub stats: RwLock<StatsTracker>,
    pub notifications: RwLock<NotificationQueue>,

    // Runtime info
    pub runtime_host: RwLock<String>,
    pub runtime_port: RwLock<u16>,
    pub runtime_lan_access: RwLock<bool>,
    pub runtime_lan_ips: RwLock<Vec<String>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sender: RwLock::new(KeyboardSender::new()),
            stats: RwLock::new(StatsTracker::new()),
            notifications: RwLock::new(NotificationQueue::new()),
            runtime_host: RwLock::new("127.0.0.1".into()),
            runtime_port: RwLock::new(8730),
            runtime_lan_access: RwLock::new(false),
            runtime_lan_ips: RwLock::new(vec![]),
        }
    }
}
