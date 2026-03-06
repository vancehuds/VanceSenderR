/// System tray icon with context menu — using tray-icon + muda crates.

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

    pub fn is_started(&self) -> bool {
        self.started
    }
}

impl Drop for TrayManager {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Create a simple 32×32 RGBA tray icon with a "V" glyph.
fn create_tray_icon() -> Option<Icon> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    // Fill with dark background and a colored border
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let is_border = x < 2 || x >= size - 2 || y < 2 || y >= size - 2;
            let is_inner = x >= 4 && x < size - 4 && y >= 4 && y < size - 4;

            if is_border {
                // Accent color border (purple-ish)
                rgba[idx] = 108;     // R
                rgba[idx + 1] = 92;  // G
                rgba[idx + 2] = 231; // B
                rgba[idx + 3] = 255; // A
            } else if is_inner {
                // Dark fill
                rgba[idx] = 18;
                rgba[idx + 1] = 24;
                rgba[idx + 2] = 37;
                rgba[idx + 3] = 255;
            } else {
                // Rounded corner transparent
                rgba[idx + 3] = 0;
            }
        }
    }

    // Draw a simple "V" shape in the center
    let _center_x = size / 2;
    let draw_pixel = |rgba: &mut [u8], x: u32, y: u32| {
        if x < size && y < size {
            let idx = ((y * size + x) * 4) as usize;
            rgba[idx] = 87;
            rgba[idx + 1] = 224;
            rgba[idx + 2] = 255;
            rgba[idx + 3] = 255;
        }
    };

    // V shape: two diagonal lines meeting at bottom center
    for i in 0..10u32 {
        let lx = 8 + i;
        let rx = size - 9 - i;
        let y = 8 + i;
        for dx in 0..2 {
            draw_pixel(&mut rgba, lx + dx, y);
            draw_pixel(&mut rgba, rx + dx, y);
        }
    }

    Icon::from_rgba(rgba, size, size).ok()
}
