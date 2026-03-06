//! System tray icon with context menu — using tray-icon + muda crates.

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// Commands that the tray icon can emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    ShowWindow,
    Exit,
}

pub struct TrayManager {
    tray: Option<TrayIcon>,
    show_item_id: Option<tray_icon::menu::MenuId>,
    exit_item_id: Option<tray_icon::menu::MenuId>,
    started: bool,
}

impl TrayManager {
    pub fn new() -> Self {
        Self {
            tray: None,
            show_item_id: None,
            exit_item_id: None,
            started: false,
        }
    }

    /// Create and show the tray icon. Must be called from a thread with
    /// a Windows message loop (typically the main/GUI thread).
    pub fn start(&mut self, title: &str) -> bool {
        if self.started {
            return true;
        }

        // Build menu
        let show_item = MenuItem::new("打开主窗口", true, None);
        let exit_item = MenuItem::new("退出 VanceSender", true, None);

        let menu = Menu::new();
        if menu.append(&show_item).is_err() || menu.append(&exit_item).is_err() {
            tracing::warn!("Failed to build tray menu");
            return false;
        }

        self.show_item_id = Some(show_item.id().clone());
        self.exit_item_id = Some(exit_item.id().clone());

        // Build icon — create a simple 32x32 RGBA icon in memory
        let icon = match create_tray_icon() {
            Some(i) => i,
            None => {
                tracing::warn!("Failed to create tray icon image");
                return false;
            }
        };

        match TrayIconBuilder::new()
            .with_tooltip(title)
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .build()
        {
            Ok(tray) => {
                self.tray = Some(tray);
                self.started = true;
                tracing::info!("System tray icon started");
                true
            }
            Err(e) => {
                tracing::warn!("Failed to create tray icon: {e}");
                false
            }
        }
    }

    /// Poll for menu events. Call this from the GUI event loop.
    /// Returns `Some(TrayCommand)` if a menu item was clicked.
    pub fn poll_event(&self) -> Option<TrayCommand> {
        if !self.started {
            return None;
        }

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if Some(&event.id) == self.show_item_id.as_ref() {
                return Some(TrayCommand::ShowWindow);
            }
            if Some(&event.id) == self.exit_item_id.as_ref() {
                return Some(TrayCommand::Exit);
            }
        }
        None
    }

    pub fn stop(&mut self) {
        self.tray.take();
        self.show_item_id = None;
        self.exit_item_id = None;
        self.started = false;
        tracing::info!("System tray icon stopped");
    }

    #[allow(dead_code)]
    pub fn is_started(&self) -> bool {
        self.started
    }
}

impl Drop for TrayManager {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Gets the path to icon.ico next to the executable.
fn icon_path() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap_or_default();
    path.pop(); // remove exe filename
    path.push("icon.ico");
    path
}

/// Load the tray icon from icon.ico, falling back to a simple coloured square.
fn create_tray_icon() -> Option<Icon> {
    // Try loading from icon.ico next to the executable
    if let Ok(data) = std::fs::read(icon_path()) {
        if let Ok(img) = image::load_from_memory_with_format(&data, image::ImageFormat::Ico) {
            let rgba_img = img.resize(32, 32, image::imageops::FilterType::Lanczos3).into_rgba8();
            let (w, h) = rgba_img.dimensions();
            return Icon::from_rgba(rgba_img.into_raw(), w, h).ok();
        }
    }

    // Fallback: simple 32×32 coloured square
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            rgba[idx] = 108;
            rgba[idx + 1] = 92;
            rgba[idx + 2] = 231;
            rgba[idx + 3] = 255;
        }
    }
    Icon::from_rgba(rgba, size, size).ok()
}
