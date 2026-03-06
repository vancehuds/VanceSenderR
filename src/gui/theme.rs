/// Dark theme with Glassmorphism-inspired styling.

use eframe::egui::{self, Color32, Rounding, Shadow, Stroke, Visuals};

pub const BG_MAIN: Color32 = Color32::from_rgb(13, 17, 28);
pub const BG_PANEL: Color32 = Color32::from_rgb(20, 25, 40);
pub const BG_CARD: Color32 = Color32::from_rgb(28, 35, 55);
pub const BG_INPUT: Color32 = Color32::from_rgb(22, 28, 45);
pub const ACCENT: Color32 = Color32::from_rgb(108, 92, 231);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(130, 115, 245);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 235, 245);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 150, 170);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(90, 100, 120);
pub const BORDER: Color32 = Color32::from_rgb(50, 60, 80);
pub const SUCCESS: Color32 = Color32::from_rgb(46, 213, 115);
pub const WARNING: Color32 = Color32::from_rgb(255, 177, 66);
pub const DANGER: Color32 = Color32::from_rgb(255, 71, 87);

pub fn apply_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    // Window
    visuals.window_fill = BG_CARD;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_shadow = Shadow::NONE;
    visuals.window_rounding = Rounding::same(8.0);

    // Panel
    visuals.panel_fill = BG_PANEL;

    // Widgets
    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
    visuals.widgets.noninteractive.rounding = Rounding::same(6.0);

    visuals.widgets.inactive.bg_fill = BG_INPUT;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.inactive.rounding = Rounding::same(6.0);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(35, 42, 65);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.hovered.rounding = Rounding::same(6.0);

    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.active.rounding = Rounding::same(6.0);

    // Selection
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.3);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);

    // Extreme bg color for text input
    visuals.extreme_bg_color = BG_INPUT;

    ctx.set_visuals(visuals);

    // Font sizing
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    ctx.set_style(style);
}
