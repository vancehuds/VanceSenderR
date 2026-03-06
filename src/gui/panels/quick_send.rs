/// Quick Send panel — fast preset-based sending.
/// Wired up with async bridge for actual sending.

use eframe::egui;
use crate::state::SharedState;
use crate::gui::theme;
use crate::gui::{AsyncResult, AsyncTx};
use crate::config;
use crate::core::history;
use crate::core::presets::{self, Preset, TextLine};
use crate::core::sender::SenderConfig;

#[derive(Default)]
pub struct QuickSendState {
    pub presets: Vec<Preset>,
    pub selected_preset_idx: Option<usize>,
    pub loaded: bool,
    pub sending_line: Option<usize>, // which line is currently being sent
}

pub fn render(
    ui: &mut egui::Ui,
    state: &SharedState,
    qs: &mut QuickSendState,
    toasts: &mut egui_notify::Toasts,
    async_tx: &AsyncTx,
    tokio_handle: &tokio::runtime::Handle,
) {
    // Lazy-load presets
    if !qs.loaded {
        qs.presets = presets::list_all_presets(None).unwrap_or_default();
        qs.loaded = true;
    }

    ui.add_space(12.0);
    ui.label(
        egui::RichText::new("⚡ 快捷发送")
            .size(18.0)
            .color(theme::TEXT_PRIMARY)
            .strong(),
    );
    ui.add_space(8.0);

    // Preset selector
    egui::Frame::NONE
        .fill(theme::BG_CARD)
        .rounding(10.0)
        .inner_margin(12.0)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("预设:").color(theme::TEXT_SECONDARY));

                let selected_name = qs
                    .selected_preset_idx
                    .and_then(|i| qs.presets.get(i))
                    .map(|p| p.name.as_str())
                    .unwrap_or("选择预设...");

                egui::ComboBox::from_id_salt("qs_preset")
                    .selected_text(selected_name)
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        for (i, preset) in qs.presets.iter().enumerate() {
                            let is_selected = qs.selected_preset_idx == Some(i);
                            if ui.selectable_label(is_selected, &preset.name).clicked() {
                                qs.selected_preset_idx = Some(i);
                            }
                        }
                    });

                if ui.button("🔄 刷新").clicked() {
                    qs.presets = presets::list_all_presets(None).unwrap_or_default();
                    qs.selected_preset_idx = None;
                }
            });
        });

    ui.add_space(12.0);

    // Text lines from selected preset
    if let Some(idx) = qs.selected_preset_idx {
        if let Some(preset) = qs.presets.get(idx).cloned() {
            if preset.texts.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("此预设没有文本行")
                            .color(theme::TEXT_MUTED),
                    );
                });
            } else {
                // Send all button
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
                        let state_clone = state.clone();
                        let ctx = ui.ctx().clone();

                        tokio_handle.spawn_blocking(move || {
                            let cfg = config::load_config();
                            let sender_cfg = SenderConfig::from_yaml(&cfg);
                            let sender = state_clone.sender.read();
                            state_clone.stats.write().record_batch();

                            sender.send_batch_sync(&texts, &sender_cfg, None, |progress| {
                                if progress.status == "sent" {
                                    if let Some(ref text) = progress.text {
                                        history::record_send(text, true, "gui-quick");
                                        state_clone.stats.write().record_send(true, None);
                                    }
                                }
                                let _ = tx.send(AsyncResult::BatchSendProgress(progress));
                                ctx.request_repaint();
                            });

                            let _ = tx.send(AsyncResult::BatchSendDone);
                            ctx.request_repaint();
                        });

                        toasts.info("开始批量发送...");
                    }

                    ui.label(
                        egui::RichText::new(format!("共{}条", preset.texts.len()))
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                });

                ui.add_space(8.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, line) in preset.texts.iter().enumerate() {
                        egui::Frame::NONE
                            .fill(theme::BG_CARD)
                            .rounding(8.0)
                            .inner_margin(10.0)
                            .stroke(egui::Stroke::new(1.0, theme::BORDER))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let type_color = match line.r#type.as_str() {
                                        "me" => theme::ACCENT,
                                        "do" => theme::SUCCESS,
                                        "b" => theme::WARNING,
                                        _ => theme::TEXT_MUTED,
                                    };
                                    ui.label(
                                        egui::RichText::new(format!("/{}", line.r#type))
                                            .color(type_color)
                                            .size(12.0)
                                            .strong(),
                                    );
                                    ui.label(
                                        egui::RichText::new(&line.content)
                                            .color(theme::TEXT_PRIMARY)
                                            .size(13.0),
                                    );

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button("📤 发送").clicked() {
                                                // Send single line
                                                let text = format!("/{} {}", line.r#type, line.content);
                                                let tx = async_tx.clone();
                                                let state_clone = state.clone();
                                                let ctx = ui.ctx().clone();

                                                tokio_handle.spawn_blocking(move || {
                                                    let cfg = config::load_config();
                                                    let sender_cfg = SenderConfig::from_yaml(&cfg);
                                                    let sender = state_clone.sender.read();
                                                    let success = sender.send_single(&text, &sender_cfg).is_ok();
                                                    history::record_send(&text, success, "gui-quick");
                                                    state_clone.stats.write().record_send(success, None);
                                                    let _ = tx.send(AsyncResult::SendSingleDone {
                                                        text,
                                                        success,
                                                    });
                                                    ctx.request_repaint();
                                                });
                                            }
                                        },
                                    );
                                });
                            });
                        ui.add_space(4.0);
                    }
                });
            }
        }
    } else {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("⚡ 选择一个预设开始快捷发送")
                    .size(14.0)
                    .color(theme::TEXT_MUTED),
            );
        });
    }
}
