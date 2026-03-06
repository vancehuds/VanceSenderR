/// Preset card widget — shows name, text count, tags, action buttons.

use eframe::egui;
use crate::gui::theme;

/// Actions the caller should handle after rendering a preset card.
#[derive(Debug, Clone)]
pub enum PresetCardAction {
    None,
    Use(String),    // preset id
    Edit(String),
    Delete(String),
    Export(String),
}

/// Render a single preset card. Returns the action the user triggered.
pub fn render_preset_card(
    ui: &mut egui::Ui,
    id: &str,
    name: &str,
    text_count: usize,
    tags: &[String],
) -> PresetCardAction {
    let mut action = PresetCardAction::None;

    egui::Frame::NONE
        .fill(theme::BG_CARD)
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Left: name + metadata
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(name)
                            .size(14.0)
                            .color(theme::TEXT_PRIMARY)
                            .strong(),
                    );
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{text_count}条文本"))
                                .size(11.0)
                                .color(theme::TEXT_MUTED),
                        );
                        for tag in tags {
                            ui.label(
                                egui::RichText::new(format!("#{tag}"))
                                    .size(11.0)
                                    .color(theme::ACCENT),
                            );
                        }
                    });
                });

                // Right: action buttons
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if ui
                            .small_button(
                                egui::RichText::new("🗑").color(theme::DANGER),
                            )
                            .on_hover_text("删除")
                            .clicked()
                        {
                            action = PresetCardAction::Delete(id.to_string());
                        }

                        if ui
                            .small_button("✏ 编辑")
                            .on_hover_text("编辑预设")
                            .clicked()
                        {
                            action = PresetCardAction::Edit(id.to_string());
                        }

                        if ui
                            .small_button("📤 使用")
                            .on_hover_text("加载到发送面板")
                            .clicked()
                        {
                            action = PresetCardAction::Use(id.to_string());
                        }
                    },
                );
            });
        });

    ui.add_space(5.0);
    action
}
