/// Send panel — text list with type selector, batch controls, progress bar.
/// Wired up with async bridge for actual sending.

use eframe::egui;
use crate::state::SharedState;
use crate::config;
use crate::core::history;
use crate::core::sender::{SenderConfig, SendProgress};
use crate::gui::theme;
use crate::gui::{AsyncResult, AsyncTx};

#[derive(Default)]
pub struct SendState {
    pub texts: Vec<TextEntry>,
    pub sending: bool,
    pub progress_index: usize,
    pub progress_total: usize,
    pub progress_status: String,
    pub new_text_content: String,
    pub new_text_type: String,
}

#[derive(Clone)]
pub struct TextEntry {
    pub r#type: String,
    pub content: String,
}

impl Default for TextEntry {
    fn default() -> Self {
        Self {
            r#type: "me".into(),
            content: String::new(),
        }
    }
}

const TEXT_TYPES: &[(&str, &str)] = &[
    ("me", "/me"),
    ("do", "/do"),
    ("b", "/b"),
    ("e", "/e"),
];

pub fn render(
    ui: &mut egui::Ui,
    state: &SharedState,
    send: &mut SendState,
    toasts: &mut egui_notify::Toasts,
    async_tx: &AsyncTx,
    tokio_handle: &tokio::runtime::Handle,
) {
    if send.new_text_type.is_empty() {
        send.new_text_type = "me".into();
    }

    ui.add_space(12.0);

    // Header
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("📤 文本发送")
                .size(18.0)
                .color(theme::TEXT_PRIMARY)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let btn = ui.add_enabled(
                !send.sending && !send.texts.is_empty(),
                egui::Button::new(
                    egui::RichText::new("🚀 批量发送").color(egui::Color32::WHITE),
                )
                .fill(theme::ACCENT),
            );
            if btn.clicked() {
                // Trigger batch send via async bridge
                send.sending = true;
                send.progress_index = 0;
                send.progress_total = send.texts.len();
                send.progress_status = "sending".into();

                let texts: Vec<String> = send
                    .texts
                    .iter()
                    .map(|e| format!("/{} {}", e.r#type, e.content))
                    .collect();

                let tx = async_tx.clone();
                let state_clone = state.clone();
                let ctx = ui.ctx().clone();

                tokio_handle.spawn_blocking(move || {
                    let cfg = config::load_config();
                    let sender_cfg = SenderConfig::from_yaml(&cfg);
                    let sender = state_clone.sender.read();
                    state_clone.stats.write().record_batch();

                    sender.send_batch_sync(
                        &texts,
                        &sender_cfg,
                        None,
                        |progress| {
                            if progress.status == "sent" {
                                if let Some(ref text) = progress.text {
                                    history::record_send(text, true, "gui");
                                    state_clone.stats.write().record_send(true, None);
                                }
                            } else if progress.status == "error" {
                                if let Some(ref text) = progress.text {
                                    history::record_send(text, false, "gui");
                                    state_clone.stats.write().record_send(false, None);
                                }
                            }
                            let _ = tx.send(AsyncResult::BatchSendProgress(progress));
                            ctx.request_repaint();
                        },
                    );

                    let _ = tx.send(AsyncResult::BatchSendDone);
                    ctx.request_repaint();
                });
            }

            if send.sending {
                if ui.button("⏹ 停止").clicked() {
                    state.sender.read().cancel();
                    send.sending = false;
                }
            }

            // Clear all button
            if !send.texts.is_empty() && !send.sending {
                if ui.button("🗑 清空").clicked() {
                    send.texts.clear();
                }
            }
        });
    });

    ui.add_space(8.0);

    // Progress bar
    if send.sending && send.progress_total > 0 {
        crate::gui::widgets::progress::render_send_progress(
            ui,
            send.progress_index,
            send.progress_total,
            &send.progress_status,
        );
        ui.add_space(8.0);
    }

    // Add text input
    egui::Frame::NONE
        .fill(theme::BG_CARD)
        .rounding(10.0)
        .inner_margin(12.0)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("new_type")
                    .width(60.0)
                    .selected_text(format!("/{}", &send.new_text_type))
                    .show_ui(ui, |ui| {
                        for (val, label) in TEXT_TYPES {
                            ui.selectable_value(&mut send.new_text_type, val.to_string(), *label);
                        }
                    });

                let te = ui.add(
                    egui::TextEdit::singleline(&mut send.new_text_content)
                        .hint_text("输入文本内容...")
                        .desired_width(ui.available_width() - 80.0),
                );

                let add_btn = ui.button("➕ 添加");

                if add_btn.clicked()
                    || (te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    let content = send.new_text_content.trim().to_string();
                    if !content.is_empty() {
                        send.texts.push(TextEntry {
                            r#type: if send.new_text_type.is_empty() {
                                "me".into()
                            } else {
                                send.new_text_type.clone()
                            },
                            content,
                        });
                        send.new_text_content.clear();
                        te.request_focus();
                    }
                }
            });
        });

    ui.add_space(8.0);

    // Text list
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut widget_items: Vec<crate::gui::widgets::text_list::TextItem> = send
                .texts
                .iter()
                .map(|e| crate::gui::widgets::text_list::TextItem {
                    r#type: e.r#type.clone(),
                    content: e.content.clone(),
                })
                .collect();

            let action =
                crate::gui::widgets::text_list::render_text_list(ui, "send_list", &mut widget_items);

            if action == crate::gui::widgets::text_list::TextListAction::Changed {
                send.texts = widget_items
                    .into_iter()
                    .map(|w| TextEntry {
                        r#type: w.r#type,
                        content: w.content,
                    })
                    .collect();
            }

            if send.texts.is_empty() {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("📝 暂无文本")
                            .size(16.0)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.label(
                        egui::RichText::new("使用上方输入框添加文本，或从预设加载")
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                });
            }
        });
}
