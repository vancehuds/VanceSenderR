/// Sidebar navigation with icon buttons.

use eframe::egui::{self, Color32, RichText, Vec2};

use super::theme;
use super::Panel;

struct NavItem {
    icon: &'static str,
    label: &'static str,
    panel: Panel,
}

const NAV_ITEMS: &[NavItem] = &[
    NavItem { icon: "🏠", label: "首页", panel: Panel::Home },
    NavItem { icon: "📤", label: "发送", panel: Panel::Send },
    NavItem { icon: "⚡", label: "快捷", panel: Panel::QuickSend },
    NavItem { icon: "🤖", label: "AI", panel: Panel::AiGenerate },
    NavItem { icon: "📋", label: "预设", panel: Panel::Presets },
    NavItem { icon: "⚙", label: "设置", panel: Panel::Settings },
];

pub fn render_sidebar(ui: &mut egui::Ui, current_panel: &mut Panel) {
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);

        for item in NAV_ITEMS {
            let is_active = *current_panel == item.panel;

            let bg = if is_active {
                theme::ACCENT.linear_multiply(0.2)
            } else {
                Color32::TRANSPARENT
            };

            let text_color = if is_active {
                theme::ACCENT
            } else {
                theme::TEXT_SECONDARY
            };

            let frame = egui::Frame::NONE
                .fill(bg)
                .rounding(8.0)
                .inner_margin(egui::Margin::symmetric(4.0, 8.0));

            let response = frame.show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(item.icon).size(20.0));
                    ui.label(
                        RichText::new(item.label)
                            .size(10.0)
                            .color(text_color),
                    );
                });
            });

            if response.response.interact(egui::Sense::click()).clicked() {
                *current_panel = item.panel;
            }

            // Active indicator
            if is_active {
                let rect = response.response.rect;
                let painter = ui.painter();
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(rect.left(), rect.top() + 4.0),
                        Vec2::new(3.0, rect.height() - 8.0),
                    ),
                    2.0,
                    theme::ACCENT,
                );
            }

            ui.add_space(4.0);
        }
    });
}
