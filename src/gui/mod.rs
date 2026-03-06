//! Native GUI entry point — egui application with async bridge.

pub mod panels;
pub mod sidebar;
pub mod theme;
pub mod titlebar;
pub mod widgets;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::config;
use crate::core::presets::{self, Preset, TextLine};
use crate::core::sender::SenderConfig;
use crate::core::history;
use crate::desktop::quick_overlay::{QuickOverlay, OverlayCommand};
use crate::desktop::tray::{TrayCommand, TrayManager};
use crate::state::SharedState;

/// Active panel in the sidebar navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    #[default]
    Home,
    Send,
    QuickSend,
    AiGenerate,
    Presets,
    Settings,
}

// ── Async bridge ───────────────────────────────────────────────────────
// egui runs synchronously on the main thread. Async operations (AI gen,
// update check, network requests) run on a tokio runtime and send results
// back via a channel.

#[derive(Debug)]
pub enum AsyncResult {
    /// Update check completed
    UpdateCheckDone(crate::core::update_checker::UpdateResult),
    /// Public config fetched
    PublicConfigDone(crate::core::public_config::PublicConfigResult),
    /// AI generation completed (non-stream)
    AiGenerateDone {
        texts: Vec<TextLine>,
        #[allow(dead_code)]
        provider_id: String,
    },
    /// AI generation error
    AiGenerateError(String),
    /// AI stream chunk
    #[allow(dead_code)]
    AiStreamChunk(String),
    /// AI stream completed
    #[allow(dead_code)]
    AiStreamDone {
        texts: Vec<TextLine>,
        provider_id: String,
    },
    /// Single send completed
    SendSingleDone { text: String, success: bool },
    /// Batch send progress
    BatchSendProgress(crate::core::sender::SendProgress),
    /// Batch send completed
    BatchSendDone,
    /// AI provider test result
    AiProviderTestDone {
        provider_id: String,
        success: bool,
        message: String,
    },
}

pub type AsyncTx = std::sync::mpsc::Sender<AsyncResult>;
pub type AsyncRx = std::sync::mpsc::Receiver<AsyncResult>;

// ── Quick overlay shared state ─────────────────────────────────────────
// Shared between the main viewport and the overlay deferred viewport.

pub struct QuickOverlayState {
    pub presets: Vec<Preset>,
    pub selected_preset_idx: Option<usize>,
    pub loaded: bool,
    pub status_message: Option<String>,
}

impl Default for QuickOverlayState {
    fn default() -> Self {
        Self {
            presets: Vec::new(),
            selected_preset_idx: None,
            loaded: false,
            status_message: None,
        }
    }
}

/// Main application state for the egui GUI.
pub struct VanceSenderApp {
    pub state: SharedState,
    pub current_panel: Panel,
    pub toasts: egui_notify::Toasts,

    // Async bridge
    pub async_tx: AsyncTx,
    pub async_rx: AsyncRx,
    pub tokio_handle: tokio::runtime::Handle,

    // Desktop integration
    pub tray: TrayManager,
    pub quick_overlay: QuickOverlay,
    pub overlay_rx: Option<std::sync::mpsc::Receiver<OverlayCommand>>,
    pub close_action: String,  // "ask", "minimize_to_tray", "exit"
    pub show_close_dialog: bool,
    pub force_exit: bool,

    // Quick overlay window (separate viewport)
    pub show_quick_overlay: Arc<AtomicBool>,
    pub overlay_window_state: Arc<Mutex<QuickOverlayState>>,

    // Panel states
    pub home_state: panels::home::HomeState,
    pub send_state: panels::send::SendState,
    pub quick_send_state: panels::quick_send::QuickSendState,
    pub ai_state: panels::ai_generate::AiState,
    pub presets_state: panels::presets::PresetsState,
    pub settings_state: panels::settings::SettingsState,
}

impl VanceSenderApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        state: SharedState,
        tokio_handle: tokio::runtime::Handle,
    ) -> Self {
        // Apply custom theme
        theme::apply_theme(&cc.egui_ctx);

        let (tx, rx) = std::sync::mpsc::channel();

        // Start tray icon
        let mut tray = TrayManager::new();
        tray.start("VanceSender");

        // Fire initial async operations
        let tx_clone = tx.clone();
        let ctx_clone = cc.egui_ctx.clone();
        tokio_handle.spawn(async move {
            let result = crate::core::update_checker::check_github_update(false).await;
            let _ = tx_clone.send(AsyncResult::UpdateCheckDone(result));
            ctx_clone.request_repaint();
        });

        let tx_clone = tx.clone();
        let ctx_clone = cc.egui_ctx.clone();
        tokio_handle.spawn(async move {
            let result = crate::core::public_config::fetch_public_config(false).await;
            let _ = tx_clone.send(AsyncResult::PublicConfigDone(result));
            ctx_clone.request_repaint();
        });

        // Start quick overlay if enabled
        let cfg = config::load_config();
        let mut quick_overlay = QuickOverlay::new();
        quick_overlay.set_ctx(cc.egui_ctx.clone());
        let overlay_enabled = config::get_bool(&cfg, "quick_overlay", "enabled");
        if overlay_enabled {
            let hotkey = config::get_str(&cfg, "quick_overlay", "trigger_hotkey").to_string();
            let mouse_btn = config::get_str(&cfg, "quick_overlay", "mouse_side_button").to_string();
            let poll_ms = config::get_i64(&cfg, "quick_overlay", "poll_interval_ms", 40) as u64;
            quick_overlay.start(&hotkey, &mouse_btn, poll_ms);
        }
        let overlay_rx = quick_overlay.take_receiver();

        let close_action = config::get_str(&cfg, "launch", "close_action").to_string();

        Self {
            state,
            current_panel: Panel::Home,
            toasts: egui_notify::Toasts::default()
                .with_anchor(egui_notify::Anchor::TopRight),
            async_tx: tx,
            async_rx: rx,
            tokio_handle,
            tray,
            quick_overlay,
            overlay_rx,
            close_action,
            show_close_dialog: false,
            force_exit: false,
            show_quick_overlay: Arc::new(AtomicBool::new(false)),
            overlay_window_state: Arc::new(Mutex::new(QuickOverlayState::default())),
            home_state: panels::home::HomeState::default(),
            send_state: panels::send::SendState::default(),
            quick_send_state: panels::quick_send::QuickSendState::default(),
            ai_state: panels::ai_generate::AiState::default(),
            presets_state: panels::presets::PresetsState::default(),
            settings_state: panels::settings::SettingsState::default(),
        }
    }

    /// Process all pending async results.
    fn drain_async_results(&mut self, ctx: &egui::Context) {
        while let Ok(result) = self.async_rx.try_recv() {
            match result {
                AsyncResult::UpdateCheckDone(r) => {
                    if r.update_available {
                        self.toasts.info(format!("发现新版本: {}", r.latest_version.as_deref().unwrap_or("?")));
                    }
                    self.home_state.update_result = Some(r);
                }
                AsyncResult::PublicConfigDone(r) => {
                    self.home_state.public_config = Some(r);
                }
                AsyncResult::AiGenerateDone { texts, provider_id: _ } => {
                    self.ai_state.generating = false;
                    self.ai_state.generated_texts = texts;
                    self.ai_state.stream_content.clear();
                    self.toasts.success("AI 生成完成");
                }
                AsyncResult::AiGenerateError(err) => {
                    self.ai_state.generating = false;
                    self.toasts.error(format!("AI 错误: {err}"));
                }
                AsyncResult::AiStreamChunk(chunk) => {
                    self.ai_state.stream_content.push_str(&chunk);
                    ctx.request_repaint();
                }
                AsyncResult::AiStreamDone { texts, provider_id: _ } => {
                    self.ai_state.generating = false;
                    self.ai_state.generated_texts = texts;
                    self.ai_state.stream_content.clear();
                    self.toasts.success("AI 生成完成");
                }
                AsyncResult::SendSingleDone { text, success } => {
                    if success {
                        self.toasts.success(format!("已发送: {}", &text[..text.len().min(20)]));
                    } else {
                        self.toasts.error(format!("发送失败: {}", &text[..text.len().min(20)]));
                    }
                }
                AsyncResult::BatchSendProgress(progress) => {
                    self.send_state.progress_index = progress.index;
                    self.send_state.progress_total = progress.total;
                    self.send_state.progress_status = progress.status.clone();
                    ctx.request_repaint();
                }
                AsyncResult::BatchSendDone => {
                    self.send_state.sending = false;
                    self.toasts.success("批量发送完成");
                }
                AsyncResult::AiProviderTestDone { provider_id, success, message } => {
                    if success {
                        self.toasts.success(format!("{provider_id}: {message}"));
                    } else {
                        self.toasts.error(format!("{provider_id}: {message}"));
                    }
                }
            }
        }
    }

    /// Handle tray icon events.
    fn handle_tray_events(&mut self, ctx: &egui::Context) {
        if let Some(cmd) = self.tray.poll_event() {
            match cmd {
                TrayCommand::ShowWindow => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                TrayCommand::Exit => {
                    self.quick_overlay.stop();
                    self.tray.stop();
                    self.force_exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    /// Handle quick overlay events.
    fn handle_overlay_events(&mut self, ctx: &egui::Context) {
        if let Some(ref rx) = self.overlay_rx {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    OverlayCommand::HotkeyTriggered => {
                        // Toggle the overlay window
                        let was_showing = self.show_quick_overlay.load(Ordering::Relaxed);
                        self.show_quick_overlay.store(!was_showing, Ordering::SeqCst);
                        ctx.request_repaint();
                    }
                    OverlayCommand::StatusUpdate { text, done } => {
                        if done {
                            self.toasts.success(text.clone());
                        } else {
                            self.toasts.info(text);
                        }
                        ctx.request_repaint();
                    }
                }
            }
        }
    }

    /// Show the quick send overlay as a separate native window.
    fn show_overlay_viewport(&self, ctx: &egui::Context) {
        if !self.show_quick_overlay.load(Ordering::Relaxed) {
            return;
        }

        let show_flag = self.show_quick_overlay.clone();
        let overlay_state = self.overlay_window_state.clone();
        let app_state = self.state.clone();
        let async_tx = self.async_tx.clone();

        let viewport_id = egui::ViewportId::from_hash_of("quick_overlay_window");
        ctx.show_viewport_deferred(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("VanceSender 快速发送")
                .with_inner_size([500.0, 450.0])
                .with_min_inner_size([400.0, 350.0])
                .with_always_on_top(),
            move |ctx, _class| {
                render_overlay_window(ctx, &show_flag, &overlay_state, &app_state, &async_tx);
            },
        );
    }

    /// Handle close-to-tray when window is about to close.
    fn handle_close_request(&mut self, ctx: &egui::Context) {
        // If force_exit is set, let the close proceed without interception.
        if self.force_exit {
            self.quick_overlay.stop();
            self.tray.stop();
            return;
        }

        match self.close_action.as_str() {
            "minimize_to_tray" => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
            "exit" => {
                // Let the close happen
                self.quick_overlay.stop();
                self.tray.stop();
            }
            _ => {
                // "ask" — show close dialog
                self.show_close_dialog = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
    }
}

impl eframe::App for VanceSenderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process async results
        self.drain_async_results(ctx);

        // Tray events
        self.handle_tray_events(ctx);

        // Overlay events
        self.handle_overlay_events(ctx);

        // Quick overlay viewport (separate window)
        self.show_overlay_viewport(ctx);

        // Close dialog
        if self.show_close_dialog {
            egui::Window::new("关闭确认")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("您想要关闭还是最小化到托盘？");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("最小化到托盘").clicked() {
                            self.show_close_dialog = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        }
                        if ui.add(egui::Button::new("退出程序").fill(theme::DANGER)).clicked() {
                            self.show_close_dialog = false;
                            self.quick_overlay.stop();
                            self.tray.stop();
                            self.force_exit = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("取消").clicked() {
                            self.show_close_dialog = false;
                        }
                    });
                });
        }

        // Custom titlebar
        titlebar::render_titlebar(ctx);

        // Sidebar navigation
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(60.0)
            .show(ctx, |ui| {
                sidebar::render_sidebar(ui, &mut self.current_panel);
            });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_panel {
                Panel::Home => panels::home::render(ui, &self.state, &mut self.home_state),
                Panel::Send => panels::send::render(
                    ui,
                    &self.state,
                    &mut self.send_state,
                    &mut self.toasts,
                    &self.async_tx,
                    &self.tokio_handle,
                ),
                Panel::QuickSend => panels::quick_send::render(
                    ui,
                    &self.state,
                    &mut self.quick_send_state,
                    &mut self.toasts,
                    &self.async_tx,
                    &self.tokio_handle,
                ),
                Panel::AiGenerate => panels::ai_generate::render(
                    ui,
                    &self.state,
                    &mut self.ai_state,
                    &mut self.toasts,
                    &self.async_tx,
                    &self.tokio_handle,
                ),
                Panel::Presets => panels::presets::render(ui, &self.state, &mut self.presets_state, &mut self.toasts),
                Panel::Settings => panels::settings::render(
                    ui,
                    &self.state,
                    &mut self.settings_state,
                    &mut self.toasts,
                    &self.async_tx,
                    &self.tokio_handle,
                ),
            }
        });

        // Render toasts
        self.toasts.show(ctx);

        // Handle viewport close request (minimize to tray / ask / exit)
        if ctx.input(|i| i.viewport().close_requested()) {
            self.handle_close_request(ctx);
        }
    }
}

// ── Overlay window rendering ───────────────────────────────────────────

fn render_overlay_window(
    ctx: &egui::Context,
    show_flag: &Arc<AtomicBool>,
    overlay_state: &Arc<Mutex<QuickOverlayState>>,
    app_state: &SharedState,
    async_tx: &AsyncTx,
) {
    // Apply theme to this viewport too
    theme::apply_theme(ctx);

    // Handle close / Escape
    if ctx.input(|i| i.viewport().close_requested()) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        show_flag.store(false, Ordering::SeqCst);
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        return;
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        let mut state = overlay_state.lock().unwrap();

        // Lazy-load presets
        if !state.loaded {
            state.presets = presets::list_all_presets(None).unwrap_or_default();
            state.loaded = true;
        }

        ui.add_space(8.0);

        // Header
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("⚡ 快速发送")
                    .size(18.0)
                    .color(theme::TEXT_PRIMARY)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("F7 / Esc 关闭")
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                );
            });
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        // Preset selector card
        egui::Frame::NONE
            .fill(theme::BG_CARD)
            .corner_radius(8.0)
            .inner_margin(10.0)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("预设:").color(theme::TEXT_SECONDARY));

                    let preset_names: Vec<String> = state.presets.iter().map(|p| p.name.clone()).collect();
                    let current_sel = state.selected_preset_idx;
                    let selected_name = current_sel
                        .and_then(|i| preset_names.get(i))
                        .cloned()
                        .unwrap_or_else(|| "选择预设...".into());

                    egui::ComboBox::from_id_salt("overlay_preset")
                        .selected_text(&selected_name)
                        .width(ui.available_width() - 60.0)
                        .show_ui(ui, |ui| {
                            for (i, name) in preset_names.iter().enumerate() {
                                let is_selected = current_sel == Some(i);
                                if ui.selectable_label(is_selected, name).clicked() {
                                    state.selected_preset_idx = Some(i);
                                }
                            }
                        });

                    if ui.button("🔄").on_hover_text("刷新预设").clicked() {
                        state.presets = presets::list_all_presets(None).unwrap_or_default();
                        state.selected_preset_idx = None;
                    }
                });
            });

        ui.add_space(8.0);

        // Status message
        if let Some(ref msg) = state.status_message.clone() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(msg)
                        .size(12.0)
                        .color(theme::SUCCESS),
                );
            });
            ui.add_space(4.0);
        }

        // Text lines from selected preset
        if let Some(idx) = state.selected_preset_idx {
            if let Some(preset) = state.presets.get(idx).cloned() {
                if preset.texts.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("此预设没有文本行")
                                .color(theme::TEXT_MUTED),
                        );
                    });
                } else {
                    // Action buttons
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("🚀 发送全部").color(egui::Color32::WHITE),
                                )
                                .fill(theme::ACCENT),
                            )
                            .clicked()
                        {
                            let texts: Vec<String> = preset
                                .texts
                                .iter()
                                .map(|l| format!("/{} {}", l.r#type, l.content))
                                .collect();

                            let tx = async_tx.clone();
                            let st = app_state.clone();
                            let ctx_clone = ctx.clone();
                            let overlay_st = overlay_state.clone();

                            std::thread::spawn(move || {
                                let cfg = config::load_config();
                                let sender_cfg = SenderConfig::from_yaml(&cfg);
                                let sender = st.sender.read();
                                st.stats.write().record_batch();

                                let _ = sender.send_batch_sync(&texts, &sender_cfg, None, |progress| {
                                    if progress.status == "sent" {
                                        if let Some(ref text) = progress.text {
                                            history::record_send(text, true, "gui-overlay");
                                            st.stats.write().record_send(true, None);
                                        }
                                    }
                                    if let Ok(mut s) = overlay_st.lock() {
                                        s.status_message = Some(format!(
                                            "发送中 {}/{}",
                                            progress.index + 1,
                                            progress.total,
                                        ));
                                    }
                                    let _ = tx.send(AsyncResult::BatchSendProgress(progress));
                                    ctx_clone.request_repaint();
                                });

                                if let Ok(mut s) = overlay_st.lock() {
                                    s.status_message = Some("✅ 发送完成".into());
                                }
                                let _ = tx.send(AsyncResult::BatchSendDone);
                                ctx_clone.request_repaint();
                            });
                        }

                        ui.label(
                            egui::RichText::new(format!("共{}条", preset.texts.len()))
                                .size(12.0)
                                .color(theme::TEXT_MUTED),
                        );
                    });

                    ui.add_space(6.0);

                    // Scrollable text lines
                    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                        for line in preset.texts.iter() {
                            egui::Frame::NONE
                                .fill(theme::BG_CARD)
                                .corner_radius(6.0)
                                .inner_margin(8.0)
                                .stroke(egui::Stroke::new(1.0, theme::BORDER))
                                .show(ui, |ui| {
                                    // Type tag + text content (wrapping)
                                    ui.horizontal_wrapped(|ui| {
                                        let type_color = match line.r#type.as_str() {
                                            "me" => theme::ACCENT,
                                            "do" => theme::SUCCESS,
                                            "b" => theme::WARNING,
                                            _ => theme::TEXT_MUTED,
                                        };
                                        ui.label(
                                            egui::RichText::new(format!("/{}", line.r#type))
                                                .color(type_color)
                                                .size(11.0)
                                                .strong(),
                                        );
                                        ui.label(
                                            egui::RichText::new(&line.content)
                                                .color(theme::TEXT_PRIMARY)
                                                .size(12.0),
                                        );
                                    });

                                    // Send button on its own row, right-aligned
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button("📤").on_hover_text("发送").clicked() {
                                                let text = format!("/{} {}", line.r#type, line.content);
                                                let tx = async_tx.clone();
                                                let st = app_state.clone();
                                                let ctx_clone = ctx.clone();

                                                std::thread::spawn(move || {
                                                    let cfg = config::load_config();
                                                    let sender_cfg = SenderConfig::from_yaml(&cfg);
                                                    let sender = st.sender.read();
                                                    let success = sender.send_single(&text, &sender_cfg).is_ok();
                                                    history::record_send(&text, success, "gui-overlay");
                                                    st.stats.write().record_send(success, None);
                                                    let _ = tx.send(AsyncResult::SendSingleDone {
                                                        text,
                                                        success,
                                                    });
                                                    ctx_clone.request_repaint();
                                                });
                                            }
                                        },
                                    );
                                });
                            ui.add_space(3.0);
                        }
                    });
                }
            }
        } else {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("⚡ 选择一个预设开始快捷发送")
                        .size(14.0)
                        .color(theme::TEXT_MUTED),
                );
            });
        }
    });
}

/// Load icon.ico from next to the executable as an egui IconData.
fn load_window_icon() -> Option<egui::IconData> {
    let mut path = std::env::current_exe().ok()?;
    path.pop();
    path.push("icon.ico");
    let data = std::fs::read(&path).ok()?;
    let img = image::load_from_memory_with_format(&data, image::ImageFormat::Ico).ok()?;
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    Some(egui::IconData {
        rgba: rgba.into_raw(),
        width: w,
        height: h,
    })
}

/// Launch the native GUI window.
pub fn run_gui(state: SharedState, tokio_handle: tokio::runtime::Handle) -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("VanceSender")
        .with_inner_size([1100.0, 750.0])
        .with_min_inner_size([800.0, 500.0])
        .with_decorations(false)
        .with_transparent(false);

    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "VanceSender",
        options,
        Box::new(move |cc| Ok(Box::new(VanceSenderApp::new(cc, state, tokio_handle)))),
    )
}
