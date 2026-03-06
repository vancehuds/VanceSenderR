//! Quick overlay — global hotkey polling + floating status bar (Win32).
//!
//! Architecture:
//! - A background thread polls `GetAsyncKeyState` for the configured hotkey
//! - When triggered, sends a command to the GUI thread via a channel
//! - Calls `egui::Context::request_repaint` to wake the GUI loop
//! - A Win32 `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE` window shows send status

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::status_overlay::{self, StatusOverlayHandle};



#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

// ── Key mapping ────────────────────────────────────────────────────────

const VK_SHIFT: i32 = 0x10;
const VK_CONTROL: i32 = 0x11;
const VK_MENU: i32 = 0x12; // Alt
const VK_LWIN: i32 = 0x5B;

fn special_key_vk(name: &str) -> Option<i32> {
    match name.to_lowercase().as_str() {
        "space" => Some(0x20),
        "enter" | "return" => Some(0x0D),
        "tab" => Some(0x09),
        "esc" | "escape" => Some(0x1B),
        "up" => Some(0x26),
        "down" => Some(0x28),
        "left" => Some(0x25),
        "right" => Some(0x27),
        "home" => Some(0x24),
        "end" => Some(0x23),
        "pageup" => Some(0x21),
        "pagedown" => Some(0x22),
        "insert" => Some(0x2D),
        "delete" => Some(0x2E),
        _ => None,
    }
}

fn modifier_vk(name: &str) -> Option<i32> {
    match name.to_lowercase().as_str() {
        "shift" => Some(VK_SHIFT),
        "ctrl" | "control" => Some(VK_CONTROL),
        "alt" => Some(VK_MENU),
        "win" | "meta" | "super" => Some(VK_LWIN),
        _ => None,
    }
}

fn parse_key_token(token: &str) -> Option<i32> {
    let lower = token.trim().to_lowercase();

    // Function keys F1-F24
    if lower.starts_with('f') && lower.len() <= 3 {
        if let Ok(n) = lower[1..].parse::<i32>() {
            if (1..=24).contains(&n) {
                return Some(0x6F + n); // VK_F1 = 0x70
            }
        }
    }

    // Special keys
    if let Some(vk) = special_key_vk(&lower) {
        return Some(vk);
    }

    // Single character
    if lower.len() == 1 {
        let ch = lower.chars().next().unwrap();
        if ch.is_ascii_alphanumeric() {
            return Some(ch.to_ascii_uppercase() as i32);
        }
    }

    None
}

/// Parse a hotkey string like "ctrl+f7" or "f7" into (modifiers[], main_key).
fn parse_hotkey(hotkey: &str) -> Option<(Vec<i32>, i32)> {
    let parts: Vec<&str> = hotkey.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers = Vec::new();
    let mut main_key = None;

    for part in &parts {
        if let Some(mod_vk) = modifier_vk(part) {
            modifiers.push(mod_vk);
        } else if let Some(vk) = parse_key_token(part) {
            main_key = Some(vk);
        }
    }

    main_key.map(|k| (modifiers, k))
}

/// Check if a virtual key is currently pressed.
#[cfg(windows)]
fn is_vk_pressed(vk: i32) -> bool {
    unsafe { GetAsyncKeyState(vk) < 0 }
}

#[cfg(not(windows))]
fn is_vk_pressed(_vk: i32) -> bool {
    false
}

/// Parse mouse side button config.
fn parse_mouse_side_button(btn: &str) -> Option<i32> {
    match btn.trim().to_lowercase().as_str() {
        "xbutton1" | "x1" | "mouse4" | "back" => Some(0x05), // VK_XBUTTON1
        "xbutton2" | "x2" | "mouse5" | "forward" => Some(0x06), // VK_XBUTTON2
        _ => None,
    }
}

// ── Overlay commands ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum OverlayCommand {
    /// Hotkey was triggered — show quick panel
    HotkeyTriggered,
    /// Status update from send operation
    #[allow(dead_code)]
    StatusUpdate { text: String, done: bool },
}

// ── QuickOverlay struct ────────────────────────────────────────────────

pub struct QuickOverlay {
    enabled: bool,
    running: Arc<AtomicBool>,
    poll_thread: Option<thread::JoinHandle<()>>,
    command_tx: Option<std::sync::mpsc::Sender<OverlayCommand>>,
    command_rx: Option<std::sync::mpsc::Receiver<OverlayCommand>>,
    egui_ctx: Option<eframe::egui::Context>,
    status_overlay: Option<StatusOverlayHandle>,
}

impl QuickOverlay {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            enabled: false,
            running: Arc::new(AtomicBool::new(false)),
            poll_thread: None,
            command_tx: Some(tx),
            command_rx: Some(rx),
            egui_ctx: None,
            status_overlay: None,
        }
    }

    /// Set the egui context so the polling thread can wake the GUI loop.
    pub fn set_ctx(&mut self, ctx: eframe::egui::Context) {
        self.egui_ctx = Some(ctx);
    }

    /// Start the hotkey polling background thread.
    pub fn start(&mut self, hotkey: &str, mouse_button: &str, poll_interval_ms: u64) {
        if self.running.load(Ordering::Relaxed) {
            return;
        }

        let parsed_hotkey = parse_hotkey(hotkey);
        let parsed_mouse = parse_mouse_side_button(mouse_button);

        if parsed_hotkey.is_none() && parsed_mouse.is_none() {
            tracing::warn!(
                "Quick overlay: no valid hotkey or mouse button configured (hotkey={hotkey:?})"
            );
            return;
        }

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);
        let tx = self.command_tx.clone().unwrap();
        let interval = Duration::from_millis(poll_interval_ms.max(20));
        let egui_ctx = self.egui_ctx.clone();

        let thread = thread::Builder::new()
            .name("quick-overlay-poll".into())
            .spawn(move || {
                tracing::info!("Quick overlay polling started");
                let mut hotkey_was_down = false;
                let mut mouse_was_down = false;

                while running.load(Ordering::Relaxed) {
                    // Check hotkey
                    if let Some((ref mods, main_key)) = parsed_hotkey {
                        let all_mods_pressed =
                            mods.iter().all(|&m| is_vk_pressed(m));
                        let main_pressed = is_vk_pressed(main_key);

                        let is_down = all_mods_pressed && main_pressed;
                        if is_down && !hotkey_was_down {
                            // Rising edge — trigger
                            let _ = tx.send(OverlayCommand::HotkeyTriggered);
                            if let Some(ref ctx) = egui_ctx {
                                ctx.request_repaint();
                            }
                        }
                        hotkey_was_down = is_down;
                    }

                    // Check mouse side button
                    if let Some(vk) = parsed_mouse {
                        let is_down = is_vk_pressed(vk);
                        if is_down && !mouse_was_down {
                            let _ = tx.send(OverlayCommand::HotkeyTriggered);
                            if let Some(ref ctx) = egui_ctx {
                                ctx.request_repaint();
                            }
                        }
                        mouse_was_down = is_down;
                    }

                    thread::sleep(interval);
                }
                tracing::info!("Quick overlay polling stopped");
            })
            .ok();

        self.poll_thread = thread;
        self.enabled = true;

        // Start the floating status bar window
        self.status_overlay = Some(status_overlay::start_status_overlay());

        tracing::info!("Quick overlay started (hotkey={hotkey:?}, mouse={mouse_button:?})");
    }

    /// Stop the polling thread.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.poll_thread.take() {
            let _ = handle.join();
        }
        if let Some(ref overlay) = self.status_overlay {
            overlay.destroy();
        }
        self.status_overlay = None;
        self.enabled = false;
    }

    /// Take the receiver end of the command channel.
    /// The GUI loop should call this once and then poll the receiver.
    pub fn take_receiver(&mut self) -> Option<std::sync::mpsc::Receiver<OverlayCommand>> {
        self.command_rx.take()
    }

    /// Send a status update (called from send operations).
    /// Shows the text on the floating status bar window.
    #[allow(dead_code)]
    pub fn send_status(&self, text: &str, done: bool) {
        // Show on floating Win32 status bar
        if let Some(ref overlay) = self.status_overlay {
            if done {
                // Show final message briefly, then auto-hide
                overlay.show_status(text);
            } else {
                overlay.show_status(text);
            }
        }
        // Also forward to egui channel for GUI panel updates
        if let Some(ref tx) = self.command_tx {
            let _ = tx.send(OverlayCommand::StatusUpdate {
                text: text.to_string(),
                done,
            });
        }
    }

    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Drop for QuickOverlay {
    fn drop(&mut self) {
        self.stop();
    }
}
