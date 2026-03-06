/// Reusable text list widget with type badges, inline editing, and delete.

use eframe::egui;
use crate::gui::theme;

#[derive(Debug, Clone)]
pub struct TextItem {
    pub r#type: String,
    pub content: String,
}

/// Render a text list with type badges and action buttons.
/// Returns actions the caller should handle.
pub fn render_text_list(
    ui: &mut egui::Ui,
    id_salt: &str,
    items: &mut Vec<TextItem>,
) -> TextListAction {
    let mut action = TextListAction::None;
    let mut to_remove = None;
    let mut to_move_up = None;
    let mut to_move_down = None;

    for (i, item) in items.iter().enumerate() {
        let item_id = format!("{id_salt}_item_{i}");

        egui::Frame::NONE
            .fill(theme::BG_CARD)
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Index
                    ui.label(
                        egui::RichText::new(format!("{}.", i + 1))
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );

                    // Type badge
                    let (type_color, type_label) = type_badge_info(&item.r#type);
                    let badge_rect = ui.label(
                        egui::RichText::new(type_label)
                            .size(11.0)
                            .color(type_color)
                            .strong(),
                    );

                    // Content
                    ui.label(
                        egui::RichText::new(&item.content)
                            .size(13.0)
                            .color(theme::TEXT_PRIMARY),
                    );

                    // Actions (right-aligned)
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            // Delete
                            if ui
                                .small_button(egui::RichText::new("✕").size(12.0).color(theme::DANGER))
                                .on_hover_text("删除")
                                .clicked()
                            {
                                to_remove = Some(i);
                            }

                            // Move down
                            if i < items.len() - 1 {
                                if ui
                                    .small_button(egui::RichText::new("▼").size(10.0).color(theme::TEXT_MUTED))
                                    .on_hover_text("下移")
                                    .clicked()
                                {
                                    to_move_down = Some(i);
                                }
                            }

                            // Move up
                            if i > 0 {
                                if ui
                                    .small_button(egui::RichText::new("▲").size(10.0).color(theme::TEXT_MUTED))
                                    .on_hover_text("上移")
                                    .clicked()
                                {
                                    to_move_up = Some(i);
                                }
                            }
                        },
                    );
                });
            });

        ui.add_space(3.0);
    }

    // Apply deferred mutations
    if let Some(idx) = to_remove {
        items.remove(idx);
        action = TextListAction::Changed;
    } else if let Some(idx) = to_move_up {
        if idx > 0 {
            items.swap(idx, idx - 1);
            action = TextListAction::Changed;
        }
    } else if let Some(idx) = to_move_down {
        if idx + 1 < items.len() {
            items.swap(idx, idx + 1);
            action = TextListAction::Changed;
        }
    }

    action
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextListAction {
    None,
    Changed,
}

fn type_badge_info(t: &str) -> (egui::Color32, &'static str) {
    match t {
        "me" => (theme::ACCENT, "/me"),
        "do" => (theme::SUCCESS, "/do"),
        "b" => (theme::WARNING, "/b"),
        "e" => (egui::Color32::from_rgb(255, 150, 200), "/e"),
        _ => (theme::TEXT_MUTED, "/??"),
    }
}
