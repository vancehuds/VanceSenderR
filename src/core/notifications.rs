//! In-memory notification queue for runtime warnings/errors.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Notification {
    pub level: String,   // "info", "warning", "error"
    pub message: String,
    pub timestamp: String,
}

pub struct NotificationQueue {
    items: Vec<Notification>,
}

impl NotificationQueue {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    #[allow(dead_code)]
    pub fn push(&mut self, level: &str, message: &str) {
        self.items.push(Notification {
            level: level.to_string(),
            message: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    pub fn get_all(&self) -> &[Notification] {
        &self.items
    }

    pub fn drain(&mut self) -> Vec<Notification> {
        std::mem::take(&mut self.items)
    }
}
