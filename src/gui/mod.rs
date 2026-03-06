/// Native GUI entry point — egui application with async bridge.

pub mod panels;
pub mod sidebar;
pub mod theme;
pub mod titlebar;
pub mod widgets;



use eframe::egui;


use crate::core::presets::TextLine;
use crate::desktop::tray::{TrayCommand, TrayManager};
use crate::state::SharedState;

/// Active panel in the sidebar navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Home,
    Send,
    QuickSend,
    AiGenerate,
    Presets,
    Settings,
}

impl Default for Panel {
    fn default() -> Self {
        Self::Home
    }
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
        provider_id: String,
    },
    /// AI generation error
    AiGenerateError(String),
    /// AI stream chunk
    AiStreamChunk(String),
    /// AI stream completed
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

        Self {
            state,
            current_panel: Panel::Home,
            toasts: egui_notify::Toasts::default()
                .with_anchor(egui_notify::Anchor::TopRight),
            async_tx: tx,
            async_rx: rx,
            tokio_handle,
            tray,
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
                    self.tray.stop();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
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
                Panel::Settings => panels::settings::render(ui, &self.state, &mut self.settings_state, &mut self.toasts),
            }
        });

        // Render toasts
        self.toasts.show(ctx);
    }
}

/// Launch the native GUI window.
pub fn run_gui(state: SharedState, tokio_handle: tokio::runtime::Handle) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("VanceSender")
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([800.0, 500.0])
            .with_decorations(false)
            .with_transparent(false),
        ..Default::default()
    };

    eframe::run_native(
        "VanceSender",
        options,
        Box::new(move |cc| Ok(Box::new(VanceSenderApp::new(cc, state, tokio_handle)))),
    )
}
