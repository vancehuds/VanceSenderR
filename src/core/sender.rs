/// Keyboard sender for FiveM — simulates T -> input -> Enter via Win32 SendInput.
///
/// Supports two input methods:
/// - clipboard: Ctrl+V paste (fast, reliable)
/// - typing: character-by-character Unicode input (slower, useful when clipboard is blocked)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::*;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::*;

// ── Constants ──────────────────────────────────────────────────────────

const VK_RETURN: u16 = 0x0D;
const VK_T: u16 = 0x54;
const VK_CONTROL: u16 = 0x11;
const VK_V: u16 = 0x56;
const VK_SHIFT: u16 = 0x10;
const VK_MENU: u16 = 0x12;     // Alt key

const DEFAULT_DELAY_OPEN_CHAT: u64 = 450;
const DEFAULT_DELAY_AFTER_PASTE: u64 = 160;
const DEFAULT_DELAY_AFTER_SEND: u64 = 260;
const DEFAULT_DELAY_BETWEEN_LINES: u64 = 1800;
const DEFAULT_FOCUS_TIMEOUT: u64 = 8000;
const DEFAULT_FOCUS_STABLE_MS: u64 = 260;
const DEFAULT_RETRY_COUNT: u32 = 3;
const DEFAULT_RETRY_INTERVAL: u64 = 450;
const DEFAULT_TYPING_CHAR_DELAY: u64 = 18;
const FOREGROUND_POLL_INTERVAL: u64 = 100;

// ── Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendProgress {
    pub status: String,   // "sending", "sent", "complete", "cancelled", "error"
    pub index: usize,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SenderConfig {
    pub method: String,
    pub chat_open_key: String,
    pub delay_open_chat: u64,
    pub delay_after_paste: u64,
    pub delay_after_send: u64,
    pub delay_between_lines: u64,
    pub focus_timeout: u64,
    pub retry_count: u32,
    pub retry_interval: u64,
    pub typing_char_delay: u64,
}

impl Default for SenderConfig {
    fn default() -> Self {
        Self {
            method: "clipboard".into(),
            chat_open_key: "t".into(),
            delay_open_chat: DEFAULT_DELAY_OPEN_CHAT,
            delay_after_paste: DEFAULT_DELAY_AFTER_PASTE,
            delay_after_send: DEFAULT_DELAY_AFTER_SEND,
            delay_between_lines: DEFAULT_DELAY_BETWEEN_LINES,
            focus_timeout: DEFAULT_FOCUS_TIMEOUT,
            retry_count: DEFAULT_RETRY_COUNT,
            retry_interval: DEFAULT_RETRY_INTERVAL,
            typing_char_delay: DEFAULT_TYPING_CHAR_DELAY,
        }
    }
}

impl SenderConfig {
    pub fn from_yaml(cfg: &serde_yaml::Value) -> Self {
        let s = cfg.get("sender");
        let get_i = |key: &str, default: i64| -> i64 {
            s.and_then(|s| s.get(key))
                .and_then(|v| v.as_i64())
                .unwrap_or(default)
        };
        let get_s = |key: &str, default: &str| -> String {
            s.and_then(|s| s.get(key))
                .and_then(|v| v.as_str())
                .unwrap_or(default)
                .to_string()
        };

        Self {
            method: get_s("method", "clipboard"),
            chat_open_key: get_s("chat_open_key", "t"),
            delay_open_chat: get_i("delay_open_chat", 450) as u64,
            delay_after_paste: get_i("delay_after_paste", 160) as u64,
            delay_after_send: get_i("delay_after_send", 260) as u64,
            delay_between_lines: get_i("delay_between_lines", 1800) as u64,
            focus_timeout: get_i("focus_timeout", 8000) as u64,
            retry_count: get_i("retry_count", 3) as u32,
            retry_interval: get_i("retry_interval", 450) as u64,
            typing_char_delay: get_i("typing_char_delay", 18) as u64,
        }
    }
}

// ── Low-level Win32 key helpers ────────────────────────────────────────

#[cfg(windows)]
fn send_key(vk: u16, up: bool) {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAP_VIRTUAL_KEY_TYPE(0)) as u16 };
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(not(windows))]
fn send_key(_vk: u16, _up: bool) {}

#[cfg(windows)]
fn send_unicode_key(ch: u16, up: bool) {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: ch,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(not(windows))]
fn send_unicode_key(_ch: u16, _up: bool) {}

fn press(vk: u16, hold_ms: u64) {
    send_key(vk, false);
    thread::sleep(Duration::from_millis(hold_ms));
    send_key(vk, true);
}

fn ctrl_v() {
    send_key(VK_CONTROL, false);
    thread::sleep(Duration::from_millis(20));
    send_key(VK_V, false);
    thread::sleep(Duration::from_millis(20));
    send_key(VK_V, true);
    send_key(VK_CONTROL, true);
}

fn release_pressed_modifiers() {
    for &vk in &[VK_SHIFT, VK_CONTROL, VK_MENU] {
        #[cfg(windows)]
        {
            let state = unsafe { GetAsyncKeyState(vk as i32) };
            if state < 0 {
                send_key(vk, true);
            }
        }
    }
}

fn chat_open_vk(key: &str) -> u16 {
    let key = key.trim().to_uppercase();
    if key.len() == 1 {
        let ch = key.chars().next().unwrap();
        if ch.is_ascii_alphanumeric() {
            return ch as u16;
        }
    }
    // Special keys
    match key.as_str() {
        "ENTER" | "RETURN" => 0x0D,
        "SPACE" => 0x20,
        "TAB" => 0x09,
        _ => VK_T, // default to T
    }
}

fn type_text(text: &str, char_delay_ms: u64) {
    for ch in text.encode_utf16() {
        send_unicode_key(ch, false);
        send_unicode_key(ch, true);
        if char_delay_ms > 0 {
            thread::sleep(Duration::from_millis(char_delay_ms));
        }
    }
}

// ── FiveM window detection ─────────────────────────────────────────────

#[cfg(windows)]
fn foreground_window_title() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 == std::ptr::null_mut() {
            return String::new();
        }
        let mut buf = [0u16; 256];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

#[cfg(not(windows))]
fn foreground_window_title() -> String {
    String::new()
}

fn is_fivem_window(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.contains("fivem") || lower.contains("gta")
        || lower.contains("cfx") || lower.contains("redm")
}

fn wait_for_fivem_foreground(timeout_ms: u64, stable_ms: u64) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    let stable_duration = Duration::from_millis(stable_ms);
    let mut stable_since: Option<std::time::Instant> = None;

    while std::time::Instant::now() < deadline {
        let title = foreground_window_title();
        if is_fivem_window(&title) {
            let now = std::time::Instant::now();
            match stable_since {
                Some(since) if now.duration_since(since) >= stable_duration => return true,
                None => stable_since = Some(now),
                _ => {}
            }
        } else {
            stable_since = None;
        }
        thread::sleep(Duration::from_millis(FOREGROUND_POLL_INTERVAL));
    }
    false
}

// ── Clipboard helper ───────────────────────────────────────────────────

fn set_clipboard(text: &str) -> bool {
    #[cfg(windows)]
    {
        clipboard_win::set_clipboard_string(text).is_ok()
    }
    #[cfg(not(windows))]
    {
        let _ = text;
        false
    }
}

// ── Sender struct ──────────────────────────────────────────────────────

pub struct KeyboardSender {
    cancel: Arc<AtomicBool>,
    sending: Arc<AtomicBool>,
    progress: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

impl KeyboardSender {
    pub fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            sending: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn is_sending(&self) -> bool {
        self.sending.load(Ordering::Relaxed)
    }

    pub fn progress(&self) -> HashMap<String, serde_json::Value> {
        self.progress.lock().unwrap().clone()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Send a single text line to FiveM.
    pub fn send_single(&self, text: &str, config: &SenderConfig) -> Result<(), String> {
        if self.sending.load(Ordering::Relaxed) {
            return Err("已有发送任务正在执行".into());
        }

        release_pressed_modifiers();

        let vk = chat_open_vk(&config.chat_open_key);
        press(vk, 40);
        thread::sleep(Duration::from_millis(config.delay_open_chat));

        if config.method == "typing" {
            type_text(text, config.typing_char_delay);
        } else {
            if !set_clipboard(text) {
                return Err("剪贴板写入失败".into());
            }
            ctrl_v();
        }

        thread::sleep(Duration::from_millis(config.delay_after_paste));
        press(VK_RETURN, 40);

        Ok(())
    }

    /// Send a batch of text lines with progress callbacks.
    pub fn send_batch_sync<F>(
        &self,
        texts: &[String],
        config: &SenderConfig,
        delay_between: Option<u64>,
        mut on_progress: F,
    ) -> Result<(), String>
    where
        F: FnMut(SendProgress),
    {
        if !self.sending.compare_exchange(
            false, true,
            Ordering::SeqCst, Ordering::Relaxed
        ).is_ok() {
            return Err("已有发送任务正在执行".into());
        }
        self.cancel.store(false, Ordering::SeqCst);

        let total = texts.len();
        let between = delay_between.unwrap_or(config.delay_between_lines);

        let result = (|| {
            // Wait for FiveM to be in foreground
            if !wait_for_fivem_foreground(config.focus_timeout, DEFAULT_FOCUS_STABLE_MS) {
                return Err("等待FiveM窗口超时".into());
            }

            for (i, text) in texts.iter().enumerate() {
                if self.cancel.load(Ordering::SeqCst) {
                    on_progress(SendProgress {
                        status: "cancelled".into(),
                        index: i,
                        total,
                        text: None,
                        error: None,
                    });
                    return Ok(());
                }

                on_progress(SendProgress {
                    status: "sending".into(),
                    index: i,
                    total,
                    text: Some(text.clone()),
                    error: None,
                });

                release_pressed_modifiers();

                let mut last_err = None;
                for _attempt in 0..config.retry_count {
                    let vk = chat_open_vk(&config.chat_open_key);
                    press(vk, 40);
                    thread::sleep(Duration::from_millis(config.delay_open_chat));

                    if config.method == "typing" {
                        type_text(text, config.typing_char_delay);
                    } else {
                        if !set_clipboard(text) {
                            last_err = Some("剪贴板写入失败".to_string());
                            thread::sleep(Duration::from_millis(config.retry_interval));
                            continue;
                        }
                        ctrl_v();
                    }

                    thread::sleep(Duration::from_millis(config.delay_after_paste));
                    press(VK_RETURN, 40);
                    last_err = None;
                    break;
                }

                if let Some(err) = last_err {
                    on_progress(SendProgress {
                        status: "error".into(),
                        index: i,
                        total,
                        text: Some(text.clone()),
                        error: Some(err.clone()),
                    });
                    return Err(err);
                }

                on_progress(SendProgress {
                    status: "sent".into(),
                    index: i,
                    total,
                    text: Some(text.clone()),
                    error: None,
                });

                thread::sleep(Duration::from_millis(config.delay_after_send));

                // Delay between lines (except after the last one)
                if i < total - 1 {
                    thread::sleep(Duration::from_millis(between));
                }
            }

            on_progress(SendProgress {
                status: "complete".into(),
                index: total,
                total,
                text: None,
                error: None,
            });

            Ok(())
        })();

        self.sending.store(false, Ordering::SeqCst);
        result
    }
}
