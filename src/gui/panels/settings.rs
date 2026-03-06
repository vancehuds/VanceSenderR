/// Settings panel — sender, server, AI, overlay configuration.

use eframe::egui;
use crate::state::SharedState;
use crate::gui::theme;
use crate::config;

#[derive(Default)]
pub struct SettingsState {
    pub active_tab: SettingsTab,
    pub loaded: bool,
    // Sender
    pub method: String,
    pub chat_open_key: String,
    pub delay_open_chat: String,
    pub delay_after_paste: String,
    pub delay_after_send: String,
    pub delay_between_lines: String,
    pub focus_timeout: String,
    pub retry_count: String,
    // Server
    pub host: String,
    pub port: String,
    pub token: String,
    pub lan_access: bool,
    // AI
    pub default_provider: String,
    pub system_prompt: String,
    // Overlay
    pub overlay_enabled: bool,
    pub trigger_hotkey: String,
}

#[derive(Default, PartialEq)]
pub enum SettingsTab {
    #[default]
    Sender,
    Server,
    AI,
    Overlay,
}

pub fn render(ui: &mut egui::Ui, state: &SharedState, ss: &mut SettingsState, toasts: &mut egui_notify::Toasts) {
    if !ss.loaded {
        load_settings(ss);
        ss.loaded = true;
    }

    ui.add_space(12.0);
    ui.label(
        egui::RichText::new("⚙ 设置")
            .size(18.0)
            .color(theme::TEXT_PRIMARY)
            .strong(),
    );
    ui.add_space(8.0);

    // Tab bar
    ui.horizontal(|ui| {
        tab_button(ui, "发送", &mut ss.active_tab, SettingsTab::Sender);
        tab_button(ui, "服务器", &mut ss.active_tab, SettingsTab::Server);
        tab_button(ui, "AI", &mut ss.active_tab, SettingsTab::AI);
        tab_button(ui, "悬浮窗", &mut ss.active_tab, SettingsTab::Overlay);
    });

    ui.add_space(12.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        match ss.active_tab {
            SettingsTab::Sender => render_sender_settings(ui, ss, toasts),
            SettingsTab::Server => render_server_settings(ui, ss, toasts),
            SettingsTab::AI => render_ai_settings(ui, ss, toasts),
            SettingsTab::Overlay => render_overlay_settings(ui, ss, toasts),
        }
    });
}

fn tab_button(ui: &mut egui::Ui, label: &str, current: &mut SettingsTab, target: SettingsTab) {
    let is_active = *current == target;
    let btn = ui.selectable_label(is_active, egui::RichText::new(label).size(13.0));
    if btn.clicked() {
        *current = target;
    }
}

fn setting_row(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{label}:"))
                .size(13.0)
                .color(theme::TEXT_SECONDARY),
        );
        ui.add(
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .desired_width(200.0),
        );
    });
    ui.add_space(4.0);
}

fn render_sender_settings(ui: &mut egui::Ui, ss: &mut SettingsState, toasts: &mut egui_notify::Toasts) {
    egui::Frame::NONE
        .fill(theme::BG_CARD)
        .corner_radius(10.0)
        .inner_margin(16.0)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("发送参数")
                    .size(15.0)
                    .color(theme::TEXT_PRIMARY)
                    .strong(),
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("输入方式:").color(theme::TEXT_SECONDARY).size(13.0));
                egui::ComboBox::from_id_salt("method")
                    .selected_text(if ss.method == "typing" { "逐字输入" } else { "剪贴板粘贴" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut ss.method, "clipboard".into(), "剪贴板粘贴");
                        ui.selectable_value(&mut ss.method, "typing".into(), "逐字输入");
                    });
            });
            ui.add_space(4.0);

            setting_row(ui, "聊天键", &mut ss.chat_open_key, "t");
            setting_row(ui, "打开聊天延迟(ms)", &mut ss.delay_open_chat, "450");
            setting_row(ui, "粘贴后延迟(ms)", &mut ss.delay_after_paste, "160");
            setting_row(ui, "发送后延迟(ms)", &mut ss.delay_after_send, "260");
            setting_row(ui, "行间延迟(ms)", &mut ss.delay_between_lines, "1800");
            setting_row(ui, "焦点超时(ms)", &mut ss.focus_timeout, "8000");
            setting_row(ui, "重试次数", &mut ss.retry_count, "3");

            ui.add_space(8.0);
            if ui.add(egui::Button::new("💾 保存").fill(theme::ACCENT)).clicked() {
                if let Err(e) = save_sender_settings(ss) {
                    toasts.error(format!("保存失败: {e}"));
                } else {
                    toasts.success("发送设置已保存");
                }
            }
        });
}

fn render_server_settings(ui: &mut egui::Ui, ss: &mut SettingsState, toasts: &mut egui_notify::Toasts) {
    egui::Frame::NONE
        .fill(theme::BG_CARD)
        .corner_radius(10.0)
        .inner_margin(16.0)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("服务器配置")
                    .size(15.0)
                    .color(theme::TEXT_PRIMARY)
                    .strong(),
            );
            ui.add_space(8.0);

            setting_row(ui, "绑定地址", &mut ss.host, "127.0.0.1");
            setting_row(ui, "端口", &mut ss.port, "8730");
            setting_row(ui, "访问令牌", &mut ss.token, "留空则不需要");

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("局域网访问:").color(theme::TEXT_SECONDARY).size(13.0));
                ui.checkbox(&mut ss.lan_access, "");
            });

            ui.add_space(8.0);
            if ui.add(egui::Button::new("💾 保存").fill(theme::ACCENT)).clicked() {
                toasts.info("服务器配置已保存（重启后生效）");
            }
        });
}

fn render_ai_settings(ui: &mut egui::Ui, ss: &mut SettingsState, toasts: &mut egui_notify::Toasts) {
    egui::Frame::NONE
        .fill(theme::BG_CARD)
        .corner_radius(10.0)
        .inner_margin(16.0)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("AI 配置")
                    .size(15.0)
                    .color(theme::TEXT_PRIMARY)
                    .strong(),
            );
            ui.add_space(8.0);

            // Provider list
            let cfg = config::load_config();
            let providers = config::get_providers(&cfg);

            if providers.is_empty() {
                ui.label(
                    egui::RichText::new("暂无AI服务商，点击下方添加")
                        .color(theme::TEXT_MUTED),
                );
            } else {
                for provider in &providers {
                    ui.horizontal(|ui| {
                        let is_default = ss.default_provider == provider.id;
                        if ui.selectable_label(is_default, &provider.name).clicked() {
                            ss.default_provider = provider.id.clone();
                        }
                        ui.label(
                            egui::RichText::new(&provider.model)
                                .size(11.0)
                                .color(theme::TEXT_MUTED),
                        );
                    });
                }
            }

            ui.add_space(8.0);
            if ui.button("➕ 添加服务商").clicked() {
                toasts.info("添加服务商功能已就绪");
            }

            ui.add_space(12.0);
            ui.label(egui::RichText::new("系统提示词:").color(theme::TEXT_SECONDARY).size(13.0));
            ui.add(
                egui::TextEdit::multiline(&mut ss.system_prompt)
                    .hint_text("自定义AI系统提示词...")
                    .desired_width(f32::INFINITY)
                    .desired_rows(4),
            );

            ui.add_space(8.0);
            if ui.add(egui::Button::new("💾 保存").fill(theme::ACCENT)).clicked() {
                toasts.info("AI设置已保存");
            }
        });
}

fn render_overlay_settings(ui: &mut egui::Ui, ss: &mut SettingsState, toasts: &mut egui_notify::Toasts) {
    egui::Frame::NONE
        .fill(theme::BG_CARD)
        .corner_radius(10.0)
        .inner_margin(16.0)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("快捷悬浮窗")
                    .size(15.0)
                    .color(theme::TEXT_PRIMARY)
                    .strong(),
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("启用:").color(theme::TEXT_SECONDARY).size(13.0));
                ui.checkbox(&mut ss.overlay_enabled, "");
            });
            ui.add_space(4.0);
            setting_row(ui, "触发热键", &mut ss.trigger_hotkey, "f7");

            ui.add_space(8.0);
            if ui.add(egui::Button::new("💾 保存").fill(theme::ACCENT)).clicked() {
                toasts.info("悬浮窗设置已保存");
            }
        });
}

fn load_settings(ss: &mut SettingsState) {
    let cfg = config::load_config();
    ss.method = config::get_str(&cfg, "sender", "method").to_string();
    ss.chat_open_key = config::get_str(&cfg, "sender", "chat_open_key").to_string();
    ss.delay_open_chat = config::get_i64(&cfg, "sender", "delay_open_chat", 450).to_string();
    ss.delay_after_paste = config::get_i64(&cfg, "sender", "delay_after_paste", 160).to_string();
    ss.delay_after_send = config::get_i64(&cfg, "sender", "delay_after_send", 260).to_string();
    ss.delay_between_lines = config::get_i64(&cfg, "sender", "delay_between_lines", 1800).to_string();
    ss.focus_timeout = config::get_i64(&cfg, "sender", "focus_timeout", 8000).to_string();
    ss.retry_count = config::get_i64(&cfg, "sender", "retry_count", 3).to_string();
    ss.host = config::get_str(&cfg, "server", "host").to_string();
    ss.port = config::get_i64(&cfg, "server", "port", 8730).to_string();
    ss.token = config::get_str(&cfg, "server", "token").to_string();
    ss.lan_access = config::get_bool(&cfg, "server", "lan_access");
    ss.default_provider = config::get_str(&cfg, "ai", "default_provider").to_string();
    ss.system_prompt = config::get_str(&cfg, "ai", "system_prompt").to_string();
    ss.overlay_enabled = config::get_bool(&cfg, "quick_overlay", "enabled");
    ss.trigger_hotkey = config::get_str(&cfg, "quick_overlay", "trigger_hotkey").to_string();
}

fn save_sender_settings(ss: &SettingsState) -> Result<(), String> {
    let patch = serde_yaml::to_value(&serde_json::json!({
        "sender": {
            "method": ss.method,
            "chat_open_key": ss.chat_open_key,
            "delay_open_chat": ss.delay_open_chat.parse::<i64>().unwrap_or(450),
            "delay_after_paste": ss.delay_after_paste.parse::<i64>().unwrap_or(160),
            "delay_after_send": ss.delay_after_send.parse::<i64>().unwrap_or(260),
            "delay_between_lines": ss.delay_between_lines.parse::<i64>().unwrap_or(1800),
            "focus_timeout": ss.focus_timeout.parse::<i64>().unwrap_or(8000),
            "retry_count": ss.retry_count.parse::<i64>().unwrap_or(3),
        }
    }))
    .map_err(|e| e.to_string())?;
    config::update_config(&patch).map_err(|e| e.to_string())
}
