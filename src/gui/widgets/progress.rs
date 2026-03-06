/// Styled progress bar widget with status text.

use eframe::egui;
use crate::gui::theme;

/// Render a themed progress bar with label and percentage.
pub fn render_send_progress(
    ui: &mut egui::Ui,
    current: usize,
    total: usize,
    status: &str,
) {
    if total == 0 {
        return;
    }

    let fraction = current as f32 / total as f32;

    egui::Frame::NONE
        .fill(theme::BG_CARD)
        .rounding(8.0)
        .inner_margin(12.0)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Status icon
                let icon = match status {
                    "sending" => "🔄",
                    "complete" => "✅",
                    "cancelled" => "⏹",
                    "error" => "❌",
                    _ => "📤",
                };
                ui.label(egui::RichText::new(icon).size(16.0));

                // Status text
                let status_text = match status {
                    "sending" => format!("发送中 {current}/{total}"),
                    "complete" => format!("完成 {total}/{total}"),
                    "cancelled" => format!("已取消 ({current}/{total})"),
                    "error" => format!("错误 ({current}/{total})"),
                    _ => format!("{status} {current}/{total}"),
                };
                ui.label(
                    egui::RichText::new(&status_text)
                        .size(13.0)
                        .color(theme::TEXT_PRIMARY),
                );

                // Percentage
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.label(
                            egui::RichText::new(format!("{:.0}%", fraction * 100.0))
                                .size(13.0)
                                .color(theme::ACCENT)
                                .strong(),
                        );
                    },
                );
            });

            ui.add_space(6.0);

            // Progress bar
            let desired_size = egui::vec2(ui.available_width(), 6.0);
            let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

            let painter = ui.painter();

            // Background track
            painter.rect_filled(
                rect,
                3.0,
                theme::BG_INPUT,
            );

            // Filled portion
            let fill_width = rect.width() * fraction.clamp(0.0, 1.0);
            if fill_width > 0.0 {
                let fill_rect = egui::Rect::from_min_size(
                    rect.min,
                    egui::vec2(fill_width, rect.height()),
                );

                // Gradient-like: use accent for normal, success for complete
                let fill_color = if status == "complete" {
                    theme::SUCCESS
                } else if status == "error" || status == "cancelled" {
                    theme::WARNING
                } else {
                    theme::ACCENT
                };

                painter.rect_filled(fill_rect, 3.0, fill_color);
            }
        });
}
