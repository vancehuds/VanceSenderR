/// Home panel — welcome banner, stats, update check, public config.
/// Update check is triggered via async bridge.

use eframe::egui;
use crate::state::SharedState;
use crate::gui::theme;
use crate::app_meta::{APP_NAME, APP_VERSION};

#[derive(Default)]
pub struct HomeState {
    pub update_result: Option<crate::core::update_checker::UpdateResult>,
    pub public_config: Option<crate::core::public_config::PublicConfigResult>,
    pub checking_update: bool,
}

pub fn render(ui: &mut egui::Ui, state: &SharedState, home: &mut HomeState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(16.0);

        // Hero card
        egui::Frame::NONE
            .fill(theme::BG_CARD)
            .rounding(12.0)
            .inner_margin(20.0)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!("⚡ 欢迎使用 {APP_NAME}"))
                        .size(22.0)
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("FiveM 文字扮演发送助手")
                        .size(14.0)
                        .color(theme::TEXT_SECONDARY),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("v{APP_VERSION}"))
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                    // Server status
                    let host = state.runtime_host.read().clone();
                    let port = *state.runtime_port.read();
                    ui.label(
                        egui::RichText::new(format!("  •  WebUI: http://{host}:{port}"))
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                });
            });

        ui.add_space(12.0);

        // Stats card
        let stats = state.stats.read().get_stats();
        ui.columns(4, |cols| {
            stat_card(&mut cols[0], "总发送", &stats.total_sent.to_string(), "📤");
            stat_card(&mut cols[1], "成功", &stats.total_success.to_string(), "✅");
            stat_card(&mut cols[2], "失败", &stats.total_failed.to_string(), "❌");
            stat_card(&mut cols[3], "成功率", &format!("{}%", stats.success_rate), "📊");
        });

        ui.add_space(12.0);

        // LAN access info
        {
            let lan_access = *state.runtime_lan_access.read();
            if lan_access {
                let ips = state.runtime_lan_ips.read().clone();
                let port = *state.runtime_port.read();
                if !ips.is_empty() {
                    egui::Frame::NONE
                        .fill(theme::BG_CARD)
                        .rounding(12.0)
                        .inner_margin(16.0)
                        .stroke(egui::Stroke::new(1.0, theme::SUCCESS.linear_multiply(0.3)))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("📱 局域网访问")
                                    .size(14.0)
                                    .color(theme::SUCCESS)
                                    .strong(),
                            );
                            ui.add_space(4.0);
                            for ip in &ips {
                                ui.label(
                                    egui::RichText::new(format!("  http://{ip}:{port}"))
                                        .size(13.0)
                                        .color(theme::TEXT_SECONDARY),
                                );
                            }
                        });
                    ui.add_space(12.0);
                }
            }
        }

        // Update check
        egui::Frame::NONE
            .fill(theme::BG_CARD)
            .rounding(12.0)
            .inner_margin(16.0)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("🔄 更新检查")
                            .size(15.0)
                            .color(theme::TEXT_PRIMARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Note: actual update check is triggered on startup via async bridge.
                        // Manual re-check would need the async_tx, which the Home panel
                        // currently doesn't have. Users can restart to re-check.
                        if home.checking_update {
                            ui.spinner();
                        }
                    });
                });

                if let Some(ref result) = home.update_result {
                    ui.add_space(8.0);
                    if result.update_available {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&result.message)
                                    .color(theme::SUCCESS)
                                    .size(13.0),
                            );
                            if let Some(ref url) = result.release_url {
                                if ui.small_button("📥 查看").clicked() {
                                    let _ = open::that(url);
                                }
                            }
                        });
                    } else {
                        ui.label(
                            egui::RichText::new(&result.message)
                                .color(theme::TEXT_SECONDARY)
                                .size(13.0),
                        );
                    }
                }
            });

        // Public config / announcement
        if let Some(ref pc) = home.public_config {
            if pc.visible {
                ui.add_space(12.0);
                egui::Frame::NONE
                    .fill(theme::BG_CARD)
                    .rounding(12.0)
                    .inner_margin(16.0)
                    .stroke(egui::Stroke::new(1.0, theme::ACCENT.linear_multiply(0.3)))
                    .show(ui, |ui| {
                        if let Some(ref title) = pc.title {
                            ui.label(
                                egui::RichText::new(format!("📢 {title}"))
                                    .size(15.0)
                                    .color(theme::ACCENT)
                                    .strong(),
                            );
                            ui.add_space(4.0);
                        }
                        if let Some(ref content) = pc.content {
                            ui.label(
                                egui::RichText::new(content)
                                    .size(13.0)
                                    .color(theme::TEXT_SECONDARY),
                            );
                        }
                        if let Some(ref link_url) = pc.link_url {
                            ui.add_space(4.0);
                            let link_text = pc
                                .link_text
                                .as_deref()
                                .unwrap_or("查看详情");
                            if ui.small_button(link_text).clicked() {
                                let _ = open::that(link_url);
                            }
                        }
                    });
            }
        }

        ui.add_space(20.0);
    });
}

fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, icon: &str) {
    egui::Frame::NONE
        .fill(theme::BG_CARD)
        .rounding(10.0)
        .inner_margin(12.0)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(icon).size(20.0));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(value)
                        .size(20.0)
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(label)
                        .size(11.0)
                        .color(theme::TEXT_SECONDARY),
                );
            });
        });
}
