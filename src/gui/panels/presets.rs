//! Presets management panel — list, create, edit, delete, import/export.

use eframe::egui;
use crate::state::SharedState;
use crate::gui::theme;
use crate::core::presets::{self, Preset, TextLine};

#[derive(Default)]
pub struct PresetsState {
    pub presets: Vec<Preset>,
    pub loaded: bool,
    pub search: String,
    pub selected_tag: Option<String>,
    pub all_tags: Vec<String>,
    // Create / edit
    pub show_form: bool,
    pub editing_id: Option<String>,
    pub form_name: String,
    pub form_tags_str: String,
    pub form_texts: Vec<TextLine>,
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
            egui::RichText::new("≡ 预设管理")
                .size(18.0)
                .color(theme::TEXT_PRIMARY)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new("+ 新建预设")
                        .fill(theme::ACCENT),
                )
                .clicked()
            {
                ps.show_form = true;
                ps.editing_id = None;
                ps.form_name.clear();
                ps.form_tags_str.clear();
                ps.form_texts = vec![TextLine { r#type: "me".into(), content: String::new() }];
            }
            if ui.button("⇩ 导入").clicked() {
                import_presets_dialog(ps, toasts);
            }
            if ui.button("⇧ 导出全部").clicked() {
                export_presets_dialog(ps, toasts);
            }
            if ui.button("↻ 刷新").clicked() {
                reload_presets(ps);
            }
        });
    });

    ui.add_space(8.0);

    // Create / Edit form
    if ps.show_form {
        render_preset_form(ui, ps, toasts);
        ui.add_space(8.0);
    }

    // Search & tag filter
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut ps.search)
                .hint_text("搜索预设...")
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
                egui::RichText::new("暂无预设")
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
        let mut action: Option<PresetAction> = None;

        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
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
                                            egui::RichText::new("✖").color(theme::DANGER),
                                        )
                                        .clicked()
                                    {
                                        action = Some(PresetAction::Delete(preset.id.clone()));
                                    }
                                    if ui.small_button("✎ 编辑").clicked() {
                                        action = Some(PresetAction::Edit(preset.clone()));
                                    }
                                    if ui.small_button("⇧ 导出").clicked() {
                                        action = Some(PresetAction::ExportSingle(preset.clone()));
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(6.0);
            }
        });

        // Handle deferred actions
        if let Some(act) = action {
            match act {
                PresetAction::Delete(id) => {
                    let _ = presets::delete_preset_file(&id);
                    reload_presets(ps);
                    toasts.info("已删除预设");
                }
                PresetAction::Edit(preset) => {
                    ps.show_form = true;
                    ps.editing_id = Some(preset.id.clone());
                    ps.form_name = preset.name.clone();
                    ps.form_tags_str = preset.tags.join(", ");
                    ps.form_texts = preset.texts.clone();
                }
                PresetAction::ExportSingle(preset) => {
                    export_single_preset(&preset, toasts);
                }
            }
        }
    }
}

enum PresetAction {
    Delete(String),
    Edit(Preset),
    ExportSingle(Preset),
}

const TEXT_TYPES: &[(&str, &str)] = &[
    ("me", "/me"),
    ("do", "/do"),
    ("b", "/b"),
    ("e", "/e"),
];

fn render_preset_form(ui: &mut egui::Ui, ps: &mut PresetsState, toasts: &mut egui_notify::Toasts) {
    let is_edit = ps.editing_id.is_some();
    let title = if is_edit { "✎ 编辑预设" } else { "+ 新建预设" };

    egui::Frame::NONE
        .fill(theme::BG_CARD)
        .corner_radius(10.0)
        .inner_margin(14.0)
        .stroke(egui::Stroke::new(1.0, theme::ACCENT))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(title)
                    .size(15.0)
                    .color(theme::ACCENT)
                    .strong(),
            );
            ui.add_space(6.0);

            // Name
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("名称:").size(13.0).color(theme::TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::singleline(&mut ps.form_name)
                        .hint_text("预设名称")
                        .desired_width(300.0),
                );
            });
            ui.add_space(4.0);

            // Tags
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("标签:").size(13.0).color(theme::TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::singleline(&mut ps.form_tags_str)
                        .hint_text("用逗号分隔，如: 巡逻, 交警")
                        .desired_width(300.0),
                );
            });
            ui.add_space(6.0);

            // Text lines
            ui.label(egui::RichText::new("文本行:").size(13.0).color(theme::TEXT_SECONDARY));
            ui.add_space(4.0);

            let mut remove_idx: Option<usize> = None;
            for (i, text_line) in ps.form_texts.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    // Type selector
                    egui::ComboBox::from_id_salt(format!("text_type_{i}"))
                        .selected_text(
                            TEXT_TYPES
                                .iter()
                                .find(|(k, _)| *k == text_line.r#type)
                                .map(|(_, v)| *v)
                                .unwrap_or("/me"),
                        )
                        .width(60.0)
                        .show_ui(ui, |ui| {
                            for (k, v) in TEXT_TYPES {
                                ui.selectable_value(&mut text_line.r#type, k.to_string(), *v);
                            }
                        });

                    ui.add(
                        egui::TextEdit::singleline(&mut text_line.content)
                            .hint_text("输入文本内容...")
                            .desired_width(ui.available_width() - 40.0),
                    );

                    if ui.small_button(
                        egui::RichText::new("✕").color(theme::DANGER),
                    ).clicked() {
                        remove_idx = Some(i);
                    }
                });
            }
            if let Some(idx) = remove_idx {
                if ps.form_texts.len() > 1 {
                    ps.form_texts.remove(idx);
                }
            }

            ui.add_space(4.0);
            if ui.button("＋ 添加行").clicked() {
                ps.form_texts.push(TextLine {
                    r#type: "me".into(),
                    content: String::new(),
                });
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("✔ 保存").fill(theme::ACCENT)).clicked() {
                    if ps.form_name.trim().is_empty() {
                        toasts.error("预设名称不能为空");
                    } else {
                        let tags: Vec<String> = ps.form_tags_str
                            .split(',')
                            .map(|t| t.trim().to_string())
                            .filter(|t| !t.is_empty())
                            .collect();

                        let texts: Vec<TextLine> = ps.form_texts
                            .iter()
                            .filter(|t| !t.content.trim().is_empty())
                            .cloned()
                            .collect();

                        if is_edit {
                            let id = ps.editing_id.as_ref().unwrap();
                            let update = serde_json::json!({
                                "name": ps.form_name.trim(),
                                "tags": tags,
                                "texts": texts,
                            });
                            match presets::update_preset(id, &update) {
                                Ok(_) => {
                                    toasts.success("已更新预设");
                                    ps.show_form = false;
                                    ps.editing_id = None;
                                    reload_presets(ps);
                                }
                                Err(e) => { toasts.error(format!("更新失败: {e}")); }
                            }
                        } else {
                            let preset_data = serde_json::json!({
                                "name": ps.form_name.trim(),
                                "tags": tags,
                                "texts": texts,
                            });
                            match presets::create_preset(&preset_data) {
                                Ok(_) => {
                                    toasts.success("已创建预设");
                                    ps.show_form = false;
                                    reload_presets(ps);
                                }
                                Err(e) => { toasts.error(format!("创建失败: {e}")); }
                            }
                        }
                    }
                }
                if ui.button("× 取消").clicked() {
                    ps.show_form = false;
                    ps.editing_id = None;
                }
            });
        });
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

// ── Import / Export ────────────────────────────────────────────────────

fn import_presets_dialog(ps: &mut PresetsState, toasts: &mut egui_notify::Toasts) {
    let file = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_title("导入预设")
        .pick_file();

    if let Some(path) = file {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(val) => {
                        let items = if val.is_array() {
                            val.as_array().cloned().unwrap_or_default()
                        } else {
                            vec![val]
                        };

                        let mut imported = 0;
                        for item in items {
                            if presets::create_preset(&item).is_ok() {
                                imported += 1;
                            }
                        }
                        reload_presets(ps);
                        toasts.success(format!("已导入 {imported} 个预设"));
                    }
                    Err(e) => { toasts.error(format!("JSON解析失败: {e}")); }
                }
            }
            Err(e) => { toasts.error(format!("读取文件失败: {e}")); }
        }
    }
}

fn export_presets_dialog(ps: &PresetsState, toasts: &mut egui_notify::Toasts) {
    let file = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_file_name("presets_export.json")
        .set_title("导出全部预设")
        .save_file();

    if let Some(path) = file {
        let json = serde_json::to_string_pretty(&ps.presets).unwrap_or_default();
        match std::fs::write(&path, json) {
            Ok(()) => { toasts.success(format!("已导出 {} 个预设", ps.presets.len())); }
            Err(e) => { toasts.error(format!("写入文件失败: {e}")); }
        }
    }
}

fn export_single_preset(preset: &Preset, toasts: &mut egui_notify::Toasts) {
    let file = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_file_name(format!("{}.json", preset.name))
        .set_title("导出预设")
        .save_file();

    if let Some(path) = file {
        let json = serde_json::to_string_pretty(preset).unwrap_or_default();
        match std::fs::write(&path, json) {
            Ok(()) => { toasts.success(format!("已导出预设: {}", preset.name)); }
            Err(e) => { toasts.error(format!("写入文件失败: {e}")); }
        }
    }
}
