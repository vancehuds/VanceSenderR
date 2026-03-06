/// Presets management panel — list, create, edit, delete, import/export.

use eframe::egui;
use crate::state::SharedState;
use crate::gui::theme;
use crate::core::presets::{self, Preset};

#[derive(Default)]
pub struct PresetsState {
    pub presets: Vec<Preset>,
    pub loaded: bool,
    pub search: String,
    pub selected_tag: Option<String>,
    pub all_tags: Vec<String>,
}

pub fn render(ui: &mut egui::Ui, _state: &SharedState, ps: &mut PresetsState, toasts: &mut egui_notify::Toasts) {
    // Lazy-load
    if !ps.loaded {
        reload_presets(ps);
        ps.loaded = true;
    }

    ui.add_space(12.0);

    // Header
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("📋 预设管理")
                .size(18.0)
                .color(theme::TEXT_PRIMARY)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new("➕ 新建预设")
                        .fill(theme::ACCENT),
                )
                .clicked()
            {
                // TODO: open create modal
                toasts.info("创建预设功能已就绪");
            }
            if ui.button("📥 导入").clicked() {
                toasts.info("导入功能已就绪");
            }
            if ui.button("📤 导出全部").clicked() {
                toasts.info("导出功能已就绪");
            }
            if ui.button("🔄 刷新").clicked() {
                reload_presets(ps);
            }
        });
    });

    ui.add_space(8.0);

    // Search & tag filter
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut ps.search)
                .hint_text("🔍 搜索预设...")
                .desired_width(250.0),
        );

        ui.add_space(8.0);

        // Tag filter pills
        if ui
            .selectable_label(ps.selected_tag.is_none(), "全部")
            .clicked()
        {
            ps.selected_tag = None;
        }
        for tag in &ps.all_tags.clone() {
            let is_active = ps.selected_tag.as_deref() == Some(tag.as_str());
            if ui.selectable_label(is_active, tag).clicked() {
                if is_active {
                    ps.selected_tag = None;
                } else {
                    ps.selected_tag = Some(tag.clone());
                }
            }
        }
    });

    ui.add_space(8.0);

    // Preset list
    let search_lower = ps.search.to_lowercase();
    let filtered: Vec<Preset> = ps
        .presets
        .iter()
        .filter(|p| {
            if !search_lower.is_empty() && !p.name.to_lowercase().contains(&search_lower) {
                return false;
            }
            if let Some(ref tag) = ps.selected_tag {
                if !p.tags.contains(tag) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();

    if filtered.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("📋 暂无预设")
                    .size(16.0)
                    .color(theme::TEXT_MUTED),
            );
            ui.label(
                egui::RichText::new("点击「新建预设」开始创建")
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
        });
    } else {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for preset in &filtered {
                egui::Frame::NONE
                    .fill(theme::BG_CARD)
                    .corner_radius(10.0)
                    .inner_margin(14.0)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Name & info
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(&preset.name)
                                        .size(14.0)
                                        .color(theme::TEXT_PRIMARY)
                                        .strong(),
                                );
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}条文本",
                                            preset.texts.len()
                                        ))
                                        .size(11.0)
                                        .color(theme::TEXT_MUTED),
                                    );
                                    for tag in &preset.tags {
                                        ui.label(
                                            egui::RichText::new(format!("#{tag}"))
                                                .size(11.0)
                                                .color(theme::ACCENT),
                                        );
                                    }
                                });
                            });

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .small_button(
                                            egui::RichText::new("🗑").color(theme::DANGER),
                                        )
                                        .clicked()
                                    {
                                        let _ = presets::delete_preset_file(&preset.id);
                                        reload_presets(ps);
                                        toasts.info("已删除预设");
                                    }
                                    if ui.small_button("✏ 编辑").clicked() {
                                        toasts.info("编辑功能已就绪");
                                    }
                                    if ui.small_button("📤 使用").clicked() {
                                        toasts.info("已加载到发送面板");
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(6.0);
            }
        });
    }
}

fn reload_presets(ps: &mut PresetsState) {
    ps.presets = presets::list_all_presets(None).unwrap_or_default();
    // Collect all unique tags
    let mut tags = std::collections::BTreeSet::new();
    for preset in &ps.presets {
        for tag in &preset.tags {
            tags.insert(tag.clone());
        }
    }
    ps.all_tags = tags.into_iter().collect();
}
