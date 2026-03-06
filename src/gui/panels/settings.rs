//! Settings panel — sender, server, AI, overlay configuration.

use eframe::egui;
use crate::state::SharedState;
use crate::gui::theme;
use crate::gui::{AsyncResult, AsyncTx};
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
    pub retry_interval: String,
    pub typing_char_delay: String,
    // Server
    pub host: String,
    pub port: String,
    pub token: String,
    pub lan_access: bool,
    // AI
    pub default_provider: String,
    pub system_prompt: String,
    // AI provider add
    pub show_add_provider: bool,
    pub new_provider_name: String,
    pub new_provider_api_base: String,
    pub new_provider_api_key: String,
    pub new_provider_model: String,
    // Overlay
    pub overlay_enabled: bool,
    pub trigger_hotkey: String,
    pub mouse_side_button: String,
    pub poll_interval_ms: String,
    pub compact_mode: bool,
    pub show_webui_send_status: bool,
}

#[derive(Default, PartialEq)]
pub enum SettingsTab {
    #[default]
    Sender,
    Server,
    AI,
    Overlay,
}

pub fn render(
    ui: &mut egui::Ui,
    _state: &SharedState,
    ss: &mut SettingsState,
    toasts: &mut egui_notify::Toasts,
    async_tx: &AsyncTx,
    tokio_handle: &tokio::runtime::Handle,
) {
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

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(
                egui::Button::new(
                    egui::RichText::new("\u{2B73} 导入旧版配置").size(12.0)
                )
                .fill(theme::BG_CARD)
                .stroke(egui::Stroke::new(1.0, theme::ACCENT))
            ).on_hover_text("从原版 VanceSender 导入 config.yaml 和预设文件")
             .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("选择原版 VanceSender 的 config.yaml")
                    .add_filter("YAML 配置", &["yaml", "yml"])
                    .pick_file()
                {
                    match config::import_config_from(&path) {
                        Ok(result) => {
                            let msg = if result.presets_copied > 0 {
                                format!("配置已导入，同时复制了 {} 个预设文件", result.presets_copied)
                            } else {
                                "配置已导入".to_string()
                            };
                            toasts.success(msg);
                            // Reload settings UI
                            ss.loaded = false;
                        }
                        Err(e) => {
                            toasts.error(format!("导入失败: {e}"));
                        }
                    }
                }
            }
        });
    });

    ui.add_space(12.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        match ss.active_tab {
            SettingsTab::Sender => render_sender_settings(ui, ss, toasts),
            SettingsTab::Server => render_server_settings(ui, ss, toasts),
            SettingsTab::AI => render_ai_settings(ui, ss, toasts, async_tx, tokio_handle),
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
            setting_row(ui, "重试间隔(ms)", &mut ss.retry_interval, "450");
            setting_row(ui, "逐字延迟(ms)", &mut ss.typing_char_delay, "18");

            ui.add_space(8.0);
            if ui.add(egui::Button::new("\u{2714} 保存").fill(theme::ACCENT)).clicked() {
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

            if ss.lan_access && ss.token.trim().is_empty() {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("⚠ 开启局域网但未设置令牌，任何人都可以访问")
                        .size(12.0)
                        .color(theme::DANGER),
                );
            }

            ui.add_space(8.0);
            if ui.add(egui::Button::new("\u{2714} 保存").fill(theme::ACCENT)).clicked() {
                if let Err(e) = save_server_settings(ss) {
                    toasts.error(format!("保存失败: {e}"));
                } else {
                    toasts.success("服务器配置已保存（重启后生效）");
                }
            }
        });
}

fn render_ai_settings(
    ui: &mut egui::Ui,
    ss: &mut SettingsState,
    toasts: &mut egui_notify::Toasts,
    async_tx: &AsyncTx,
    tokio_handle: &tokio::runtime::Handle,
) {
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
            let mut provider_to_delete: Option<String> = None;

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
                        ui.label(
                            egui::RichText::new(if provider.api_key.is_empty() { "[\u{00D7}]" } else { "[\u{2714}]" })
                                .size(11.0)
                                .color(if provider.api_key.is_empty() { theme::DANGER } else { theme::SUCCESS }),
                        );

                        // Test button
                        if ui.small_button("\u{25B6} 测试").clicked() {
                            let pid = provider.id.clone();
                            let tx = async_tx.clone();
                            let ctx = ui.ctx().clone();
                            tokio_handle.spawn(async move {
                                let result = crate::core::ai_client::test_provider(&pid).await;
                                let (success, message) = match result {
                                    Ok(val) => {
                                        let msg = val["message"].as_str().unwrap_or("连接成功").to_string();
                                        (true, msg)
                                    }
                                    Err(e) => (false, e.to_string()),
                                };
                                let _ = tx.send(AsyncResult::AiProviderTestDone {
                                    provider_id: pid,
                                    success,
                                    message,
                                });
                                ctx.request_repaint();
                            });
                        }

                        // Delete button
                        if ui.small_button(
                            egui::RichText::new("\u{2716}").color(theme::DANGER),
                        ).clicked() {
                            provider_to_delete = Some(provider.id.clone());
                        }
                    });
                }
            }

            // Process deferred deletions
            if let Some(id) = provider_to_delete {
                match config::delete_provider(&id) {
                    Ok(()) => { toasts.success("已删除服务商"); }
                    Err(e) => { toasts.error(format!("删除失败: {e}")); }
                }
            }

            ui.add_space(8.0);

            // Add provider toggle
            if !ss.show_add_provider {
                if ui.button("+ 添加服务商").clicked() {
                    ss.show_add_provider = true;
                    ss.new_provider_name.clear();
                    ss.new_provider_api_base.clear();
                    ss.new_provider_api_key.clear();
                    ss.new_provider_model = "gpt-4o".to_string();
                }
            } else {
                egui::Frame::NONE
                    .fill(theme::BG_MAIN)
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .stroke(egui::Stroke::new(1.0, theme::ACCENT))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("添加新服务商")
                                .size(13.0)
                                .color(theme::ACCENT)
                                .strong(),
                        );
                        ui.add_space(4.0);
                        setting_row(ui, "名称", &mut ss.new_provider_name, "例: OpenAI");
                        setting_row(ui, "API Base", &mut ss.new_provider_api_base, "https://api.openai.com/v1");
                        setting_row(ui, "API Key", &mut ss.new_provider_api_key, "sk-...");
                        setting_row(ui, "模型", &mut ss.new_provider_model, "gpt-4o");

                        ui.horizontal(|ui| {
                            if ui.add(egui::Button::new("\u{2714} 确认").fill(theme::ACCENT)).clicked() {
                                if ss.new_provider_name.trim().is_empty() {
                                    toasts.error("服务商名称不能为空");
                                } else {
                                    let provider = config::ProviderConfig {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        name: ss.new_provider_name.trim().to_string(),
                                        api_base: ss.new_provider_api_base.trim().to_string(),
                                        api_key: ss.new_provider_api_key.trim().to_string(),
                                        model: ss.new_provider_model.trim().to_string(),
                                    };
                                    match config::add_provider(provider) {
                                        Ok(()) => {
                                            toasts.success("已添加服务商");
                                            ss.show_add_provider = false;
                                        }
                                        Err(e) => { toasts.error(format!("添加失败: {e}")); }
                                    }
                                }
                            }
                            if ui.button("\u{00D7} 取消").clicked() {
                                ss.show_add_provider = false;
                            }
                        });
                    });
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
            if ui.add(egui::Button::new("\u{2714} 保存").fill(theme::ACCENT)).clicked() {
                if let Err(e) = save_ai_settings(ss) {
                    toasts.error(format!("保存失败: {e}"));
                } else {
                    toasts.success("AI设置已保存");
                }
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
            setting_row(ui, "鼠标侧键", &mut ss.mouse_side_button, "xbutton1");
            setting_row(ui, "轮询间隔(ms)", &mut ss.poll_interval_ms, "40");

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("紧凑模式:").color(theme::TEXT_SECONDARY).size(13.0));
                ui.checkbox(&mut ss.compact_mode, "");
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("显示发送状态:").color(theme::TEXT_SECONDARY).size(13.0));
                ui.checkbox(&mut ss.show_webui_send_status, "");
            });

            ui.add_space(8.0);
            if ui.add(egui::Button::new("\u{2714} 保存").fill(theme::ACCENT)).clicked() {
                if let Err(e) = save_overlay_settings(ss) {
                    toasts.error(format!("保存失败: {e}"));
                } else {
                    toasts.success("悬浮窗设置已保存");
                }
            }
        });
}

fn load_settings(ss: &mut SettingsState) {
    let cfg = config::load_config();
    // Sender
    ss.method = config::get_str(&cfg, "sender", "method").to_string();
    ss.chat_open_key = config::get_str(&cfg, "sender", "chat_open_key").to_string();
    ss.delay_open_chat = config::get_i64(&cfg, "sender", "delay_open_chat", 450).to_string();
    ss.delay_after_paste = config::get_i64(&cfg, "sender", "delay_after_paste", 160).to_string();
    ss.delay_after_send = config::get_i64(&cfg, "sender", "delay_after_send", 260).to_string();
    ss.delay_between_lines = config::get_i64(&cfg, "sender", "delay_between_lines", 1800).to_string();
    ss.focus_timeout = config::get_i64(&cfg, "sender", "focus_timeout", 8000).to_string();
    ss.retry_count = config::get_i64(&cfg, "sender", "retry_count", 3).to_string();
    ss.retry_interval = config::get_i64(&cfg, "sender", "retry_interval", 450).to_string();
    ss.typing_char_delay = config::get_i64(&cfg, "sender", "typing_char_delay", 18).to_string();
    // Server
    ss.host = config::get_str(&cfg, "server", "host").to_string();
    ss.port = config::get_i64(&cfg, "server", "port", 8730).to_string();
    ss.token = config::get_str(&cfg, "server", "token").to_string();
    ss.lan_access = config::get_bool(&cfg, "server", "lan_access");
    // AI
    ss.default_provider = config::get_str(&cfg, "ai", "default_provider").to_string();
    ss.system_prompt = config::get_str(&cfg, "ai", "system_prompt").to_string();
    // Overlay
    ss.overlay_enabled = config::get_bool(&cfg, "quick_overlay", "enabled");
    ss.trigger_hotkey = config::get_str(&cfg, "quick_overlay", "trigger_hotkey").to_string();
    ss.mouse_side_button = config::get_str(&cfg, "quick_overlay", "mouse_side_button").to_string();
    ss.poll_interval_ms = config::get_i64(&cfg, "quick_overlay", "poll_interval_ms", 40).to_string();
    ss.compact_mode = config::get_bool(&cfg, "quick_overlay", "compact_mode");
    ss.show_webui_send_status = {
        cfg.get("quick_overlay")
            .and_then(|s| s.get("show_webui_send_status"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    };
}

fn save_sender_settings(ss: &SettingsState) -> Result<(), String> {
    let patch = serde_yaml::to_value(serde_json::json!({
        "sender": {
            "method": ss.method,
            "chat_open_key": ss.chat_open_key,
            "delay_open_chat": ss.delay_open_chat.parse::<i64>().unwrap_or(450),
            "delay_after_paste": ss.delay_after_paste.parse::<i64>().unwrap_or(160),
            "delay_after_send": ss.delay_after_send.parse::<i64>().unwrap_or(260),
            "delay_between_lines": ss.delay_between_lines.parse::<i64>().unwrap_or(1800),
            "focus_timeout": ss.focus_timeout.parse::<i64>().unwrap_or(8000),
            "retry_count": ss.retry_count.parse::<i64>().unwrap_or(3),
            "retry_interval": ss.retry_interval.parse::<i64>().unwrap_or(450),
            "typing_char_delay": ss.typing_char_delay.parse::<i64>().unwrap_or(18),
        }
    }))
    .map_err(|e| e.to_string())?;
    config::update_config(&patch).map_err(|e| e.to_string())
}

fn save_server_settings(ss: &SettingsState) -> Result<(), String> {
    let patch = serde_yaml::to_value(serde_json::json!({
        "server": {
            "host": ss.host,
            "port": ss.port.parse::<i64>().unwrap_or(8730),
            "token": ss.token,
            "lan_access": ss.lan_access,
        }
    }))
    .map_err(|e| e.to_string())?;
    config::update_config(&patch).map_err(|e| e.to_string())
}

fn save_ai_settings(ss: &SettingsState) -> Result<(), String> {
    let patch = serde_yaml::to_value(serde_json::json!({
        "ai": {
            "default_provider": ss.default_provider,
            "system_prompt": ss.system_prompt,
        }
    }))
    .map_err(|e| e.to_string())?;
    config::update_config(&patch).map_err(|e| e.to_string())
}

fn save_overlay_settings(ss: &SettingsState) -> Result<(), String> {
    let patch = serde_yaml::to_value(serde_json::json!({
        "quick_overlay": {
            "enabled": ss.overlay_enabled,
            "trigger_hotkey": ss.trigger_hotkey,
            "mouse_side_button": ss.mouse_side_button,
            "poll_interval_ms": ss.poll_interval_ms.parse::<i64>().unwrap_or(40),
            "compact_mode": ss.compact_mode,
            "show_webui_send_status": ss.show_webui_send_status,
        }
    }))
    .map_err(|e| e.to_string())?;
    config::update_config(&patch).map_err(|e| e.to_string())
}
