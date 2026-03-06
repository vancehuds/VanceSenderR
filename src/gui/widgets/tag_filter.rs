//! Tag filter pill-style widget — multi-select filtering.

use eframe::egui;
use crate::gui::theme;

/// Render a row of tag filter pills. Returns true if selection changed.
#[allow(dead_code)]
pub fn render_tag_filter(
    ui: &mut egui::Ui,
    tags: &[String],
    selected: &mut Option<String>,
) -> bool {
    let mut changed = false;

    ui.horizontal_wrapped(|ui| {
        // "All" pill
        let all_active = selected.is_none();
        let all_btn = pill_button(ui, "全部", all_active);
        if all_btn.clicked() && !all_active {
            *selected = None;
            changed = true;
        }

        // Tag pills
        for tag in tags {
            let is_active = selected.as_deref() == Some(tag.as_str());
            let btn = pill_button(ui, &format!("#{tag}"), is_active);
            if btn.clicked() {
                if is_active {
                    *selected = None;
                } else {
                    *selected = Some(tag.clone());
                }
                changed = true;
            }
        }
    });

    changed
}

#[allow(dead_code)]
fn pill_button(ui: &mut egui::Ui, label: &str, active: bool) -> egui::Response {
    let bg = if active {
        theme::ACCENT.linear_multiply(0.25)
    } else {
        theme::BG_INPUT
    };
    let text_color = if active {
        theme::ACCENT
    } else {
        theme::TEXT_SECONDARY
    };

    let button = egui::Button::new(
        egui::RichText::new(label).size(12.0).color(text_color),
    )
    .fill(bg)
    .corner_radius(16.0)
    .stroke(egui::Stroke::new(
        1.0,
        if active {
            theme::ACCENT.linear_multiply(0.4)
        } else {
            theme::BORDER
        },
    ));

    ui.add(button)
}
