/// AI Generate panel — scenario input, streaming generation, history.
/// Wired up with async bridge for actual AI calls.

use eframe::egui;
use crate::state::SharedState;
use crate::gui::theme;
use crate::gui::{AsyncResult, AsyncTx};
use crate::core::presets::TextLine;
use crate::core::ai_client;
use crate::core::ai_history;

#[derive(Default)]
pub struct AiState {
    pub scenario: String,
    pub text_type: String,
    pub style: String,
    pub generating: bool,
    pub generated_texts: Vec<TextLine>,
    pub stream_content: String,
    pub history_expanded: bool,
}

pub fn render(
    ui: &mut egui::Ui,
    state: &SharedState,
    ai: &mut AiState,
    toasts: &mut egui_notify::Toasts,
    async_tx: &AsyncTx,
    tokio_handle: &tokio::runtime::Handle,
) {
    if ai.text_type.is_empty() {
        ai.text_type = "mixed".into();
    }

    ui.add_space(12.0);
    ui.label(
        egui::RichText::new("🤖 AI 文本生成")
            .size(18.0)
            .color(theme::TEXT_PRIMARY)
            .strong(),
    );
    ui.add_space(8.0);

    // Input area
    egui::Frame::NONE
        .fill(theme::BG_CARD)
        .corner_radius(10.0)
        .inner_margin(16.0)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("场景描述")
                    .size(13.0)
                    .color(theme::TEXT_SECONDARY),
            );
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::multiline(&mut ai.scenario)
                    .hint_text("描述一个角色扮演场景，AI将生成对应的 /me /do 文本...")
                    .desired_width(f32::INFINITY)
                    .desired_rows(3),
            );

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("类型:").color(theme::TEXT_SECONDARY));
                egui::ComboBox::from_id_salt("ai_type")
                    .selected_text(match ai.text_type.as_str() {
                        "me_only" => "仅 /me",
                        "do_only" => "仅 /do",
                        _ => "混合",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut ai.text_type, "mixed".into(), "混合");
                        ui.selectable_value(&mut ai.text_type, "me_only".into(), "仅 /me");
                        ui.selectable_value(&mut ai.text_type, "do_only".into(), "仅 /do");
                    });

                ui.add_space(16.0);
                ui.label(egui::RichText::new("风格:").color(theme::TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::singleline(&mut ai.style)
                        .hint_text("可选: 简洁/详细/古风...")
                        .desired_width(150.0),
                );
            });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let generate_btn = ui.add_enabled(
                    !ai.generating && !ai.scenario.trim().is_empty(),
                    egui::Button::new(
                        egui::RichText::new("✨ 生成").color(egui::Color32::WHITE),
                    )
                    .fill(theme::ACCENT),
                );
                if generate_btn.clicked() {
                    // Trigger async AI generation
                    ai.generating = true;
                    ai.stream_content.clear();
                    ai.generated_texts.clear();

                    let scenario = ai.scenario.clone();
                    let text_type = ai.text_type.clone();
                    let style = if ai.style.trim().is_empty() {
                        None
                    } else {
                        Some(ai.style.clone())
                    };

                    let tx = async_tx.clone();
                    let ctx = ui.ctx().clone();

                    tokio_handle.spawn(async move {
                        match ai_client::generate_texts(
                            &scenario,
                            None,
                            None,
                            &text_type,
                            style.as_deref(),
                            None,
                        )
                        .await
                        {
                            Ok((texts, provider_id)) => {
                                ai_history::save_generation(&scenario, &texts, &provider_id);
                                let _ = tx.send(AsyncResult::AiGenerateDone {
                                    texts,
                                    provider_id,
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(AsyncResult::AiGenerateError(e.to_string()));
                            }
                        }
                        ctx.request_repaint();
                    });
                }

                if ai.generating {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new("生成中...")
                            .color(theme::TEXT_SECONDARY),
                    );
                }
            });
        });

    ui.add_space(12.0);

    // Results display
    if !ai.stream_content.is_empty() || !ai.generated_texts.is_empty() {
        egui::Frame::NONE
            .fill(theme::BG_CARD)
            .corner_radius(10.0)
            .inner_margin(16.0)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("生成结果")
                        .size(14.0)
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                );
                ui.add_space(8.0);

                if ai.generating && !ai.stream_content.is_empty() {
                    ui.label(
                        egui::RichText::new(&ai.stream_content)
                            .size(13.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                } else {
                    for line in &ai.generated_texts {
                        ui.horizontal(|ui| {
                            let color = match line.r#type.as_str() {
                                "me" => theme::ACCENT,
                                "do" => theme::SUCCESS,
                                "b" => theme::WARNING,
                                _ => theme::TEXT_MUTED,
                            };
                            ui.label(
                                egui::RichText::new(format!("/{}", line.r#type))
                                    .color(color)
                                    .size(12.0)
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new(&line.content)
                                    .color(theme::TEXT_PRIMARY)
                                    .size(13.0),
                            );
                        });
                        ui.add_space(2.0);
                    }

                    if !ai.generated_texts.is_empty() {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("📋 复制全部").clicked() {
                                let text = ai
                                    .generated_texts
                                    .iter()
                                    .map(|l| format!("/{} {}", l.r#type, l.content))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                ui.ctx().copy_text(text);
                                toasts.success("已复制到剪贴板");
                            }
                            if ui.button("🔄 重新生成").clicked() && !ai.scenario.trim().is_empty() {
                                // Re-trigger
                                ai.generating = true;
                                ai.stream_content.clear();
                                ai.generated_texts.clear();

                                let scenario = ai.scenario.clone();
                                let text_type = ai.text_type.clone();
                                let style = if ai.style.trim().is_empty() {
                                    None
                                } else {
                                    Some(ai.style.clone())
                                };

                                let tx = async_tx.clone();
                                let ctx = ui.ctx().clone();

                                tokio_handle.spawn(async move {
                                    match ai_client::generate_texts(
                                        &scenario, None, None, &text_type, style.as_deref(), None,
                                    )
                                    .await
                                    {
                                        Ok((texts, provider_id)) => {
                                            ai_history::save_generation(
                                                &scenario, &texts, &provider_id,
                                            );
                                            let _ = tx.send(AsyncResult::AiGenerateDone {
                                                texts,
                                                provider_id,
                                            });
                                        }
                                        Err(e) => {
                                            let _ = tx.send(AsyncResult::AiGenerateError(
                                                e.to_string(),
                                            ));
                                        }
                                    }
                                    ctx.request_repaint();
                                });
                            }
                        });
                    }
                }
            });
    }

    ui.add_space(12.0);

    // AI history section
    egui::CollapsingHeader::new(
        egui::RichText::new("📜 生成历史")
            .size(14.0)
            .color(theme::TEXT_PRIMARY),
    )
    .default_open(false)
    .show(ui, |ui| {
        let entries = ai_history::list_history(10, 0);
        if entries.is_empty() {
            ui.label(
                egui::RichText::new("暂无历史记录")
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
        } else {
            for entry in &entries {
                egui::Frame::NONE
                    .fill(theme::BG_INPUT)
                    .corner_radius(6.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let star = if entry.starred { "⭐" } else { "☆" };
                            if ui.small_button(star).clicked() {
                                ai_history::toggle_star(&entry.id);
                            }
                            ui.label(
                                egui::RichText::new(&entry.scenario)
                                    .size(12.0)
                                    .color(theme::TEXT_PRIMARY),
                            );
                            ui.label(
                                egui::RichText::new(format!("{}条", entry.texts.len()))
                                    .size(11.0)
                                    .color(theme::TEXT_MUTED),
                            );
                        });
                    });
                ui.add_space(3.0);
            }
        }
    });
}
