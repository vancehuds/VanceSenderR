/// Custom frameless titlebar with window controls.

use eframe::egui::{self, Sense, Vec2};

use super::theme;

pub fn render_titlebar(ctx: &egui::Context) {
    egui::TopBottomPanel::top("titlebar")
        .exact_height(36.0)
        .frame(egui::Frame::NONE.fill(theme::BG_MAIN))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(12.0);

                // App title
                ui.label(
                    egui::RichText::new("⚡ VanceSender")
                        .color(theme::ACCENT)
                        .size(14.0)
                        .strong(),
                );

                // Drag area (fills remaining space)
                let available = ui.available_width() - 120.0;
                let drag_rect = ui.allocate_exact_size(
                    Vec2::new(available.max(0.0), 36.0),
                    Sense::drag(),
                );
                if drag_rect.1.dragged() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                // Window controls
                let button_size = Vec2::new(36.0, 28.0);

                // Minimize
                if ui
                    .add_sized(button_size, egui::Button::new("─").frame(false))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }

                // Maximize/Restore
                if ui
                    .add_sized(button_size, egui::Button::new("□").frame(false))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(
                        !ui.input(|i| i.viewport().maximized.unwrap_or(false)),
                    ));
                }

                // Close
                let close_btn = ui
                    .add_sized(
                        button_size,
                        egui::Button::new(
                            egui::RichText::new("✕").color(theme::TEXT_PRIMARY),
                        )
                        .frame(false),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);

                if close_btn.hovered() {
                    let painter = ui.painter();
                    painter.rect_filled(close_btn.rect, 4.0, theme::DANGER.linear_multiply(0.3));
                }

                if close_btn.clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
}
