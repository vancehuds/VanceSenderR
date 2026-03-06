//! Dark theme with Glassmorphism-inspired styling.

use eframe::egui::{self, Color32, CornerRadius, Shadow, Stroke, Visuals};

pub const BG_MAIN: Color32 = Color32::from_rgb(13, 17, 28);
pub const BG_PANEL: Color32 = Color32::from_rgb(20, 25, 40);
pub const BG_CARD: Color32 = Color32::from_rgb(28, 35, 55);
pub const BG_INPUT: Color32 = Color32::from_rgb(22, 28, 45);
pub const ACCENT: Color32 = Color32::from_rgb(108, 92, 231);
#[allow(dead_code)]
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(130, 115, 245);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 235, 245);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 150, 170);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(90, 100, 120);
pub const BORDER: Color32 = Color32::from_rgb(50, 60, 80);
pub const SUCCESS: Color32 = Color32::from_rgb(46, 213, 115);
pub const WARNING: Color32 = Color32::from_rgb(255, 177, 66);
pub const DANGER: Color32 = Color32::from_rgb(255, 71, 87);

pub fn apply_theme(ctx: &egui::Context) {
    // ── Chinese font support ───────────────────────────────────────────
    configure_fonts(ctx);

    let mut visuals = Visuals::dark();

    // Window
    visuals.window_fill = BG_CARD;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_shadow = Shadow::NONE;
    visuals.window_corner_radius = CornerRadius::same(8);

    // Panel
    visuals.panel_fill = BG_PANEL;

    // Widgets
    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = BG_INPUT;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(35, 42, 65);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(6);

    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.active.corner_radius = CornerRadius::same(6);

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

/// Load system fonts as fallbacks:
/// 1. Segoe UI Symbol — for Unicode symbols (─ □ ✕ ⚙ ⚡ ⌂ etc.)
/// 2. Chinese font — for CJK characters
fn configure_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};

    let mut fonts = FontDefinitions::default();

    // ── 1. Symbol font (Segoe UI Symbol on Windows) ──────────────────
    // Contains geometric shapes, box drawing, dingbats, misc symbols.
    let symbol_paths: &[&str] = &[
        "C:\\Windows\\Fonts\\seguisym.ttf",  // Segoe UI Symbol (Win 7+)
        "C:\\Windows\\Fonts\\segmdl2.ttf",   // Segoe MDL2 Assets
    ];

    for path in symbol_paths {
        if let Ok(font_data) = std::fs::read(path) {
            tracing::info!("Loaded symbol font from: {path}");
            fonts.font_data.insert(
                "symbol_font".to_owned(),
                std::sync::Arc::new(FontData::from_owned(font_data)),
            );
            for family_key in [FontFamily::Proportional, FontFamily::Monospace] {
                if let Some(family) = fonts.families.get_mut(&family_key) {
                    family.push("symbol_font".to_owned());
                }
            }
            break;
        }
    }

    // ── 2. Chinese font ──────────────────────────────────────────────
    // Priority: Microsoft YaHei > SimHei > NotoSansSC
    let cjk_paths: &[&str] = &[
        "C:\\Windows\\Fonts\\msyh.ttc",     // Microsoft YaHei (Win 7+)
        "C:\\Windows\\Fonts\\msyhbd.ttc",   // Microsoft YaHei Bold
        "C:\\Windows\\Fonts\\simhei.ttf",   // SimHei
        "C:\\Windows\\Fonts\\simsun.ttc",   // SimSun
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",  // Linux
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",       // Linux alt
        "/System/Library/Fonts/PingFang.ttc",                       // macOS
    ];

    let mut cjk_loaded = false;
    for path in cjk_paths {
        if let Ok(font_data) = std::fs::read(path) {
            tracing::info!("Loaded Chinese font from: {path}");
            fonts.font_data.insert(
                "chinese_font".to_owned(),
                std::sync::Arc::new(FontData::from_owned(font_data)),
            );
            for family_key in [FontFamily::Proportional, FontFamily::Monospace] {
                if let Some(family) = fonts.families.get_mut(&family_key) {
                    family.push("chinese_font".to_owned());
                }
            }
            cjk_loaded = true;
            break;
        }
    }

    if !cjk_loaded {
        tracing::warn!("No Chinese font found on system — CJK characters may not render correctly");
    }

    ctx.set_fonts(fonts);
}
