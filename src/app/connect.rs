use eframe::egui::{self, Color32, FontId, RichText, ScrollArea, Ui};

use super::state::App;
use super::widgets::log_line_color;
use crate::vpn::VpnStatus;

impl App {
    pub fn ui_connect(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.inline_status(ui);
                ui.add_space(8.0);

                self.selector_card(ui);
                ui.add_space(8.0);

                self.action_button(ui);
                ui.add_space(8.0);

                self.logs_card(ui);
            });
    }

    fn inline_status(&mut self, ui: &mut Ui) {
        if let VpnStatus::Failed(msg) = &self.vpn_status {
            ui.colored_label(Color32::from_rgb(235, 120, 120), msg);
        }
    }

    fn selector_card(&mut self, ui: &mut Ui) {
        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(25, 31, 41))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Conexión")
                        .size(14.5)
                        .strong()
                        .color(Color32::from_rgb(232, 236, 242)),
                );
                ui.add_space(6.0);

                compact_label(ui, "Perfil VPN");
                if self.config.vpn_profiles.is_empty() {
                    ui.colored_label(
                        Color32::from_rgb(232, 182, 75),
                        "No hay perfiles disponibles. Crea uno en “Perfiles VPN”.",
                    );
                } else {
                    let prev_profile = self.selected_profile_id.clone();

                    let current_name = self
                        .selected_profile_id
                        .as_deref()
                        .and_then(|id| self.config.find_profile(id))
                        .map(|p| p.name.as_str())
                        .unwrap_or("Seleccionar perfil");

                    egui::ComboBox::from_id_source("combo_profile")
                        .width(ui.available_width().max(120.0))
                        .selected_text(current_name)
                        .show_ui(ui, |ui| {
                            for profile in &self.config.vpn_profiles {
                                ui.selectable_value(
                                    &mut self.selected_profile_id,
                                    Some(profile.id.clone()),
                                    &profile.name,
                                );
                            }
                        });

                    if self.selected_profile_id != prev_profile {
                        self.save_config();
                    }
                }

                ui.add_space(8.0);

                compact_label(ui, "Proxy");
                let prev_proxy = self.selected_proxy_id.clone();

                let current_proxy_name = self
                    .selected_proxy_id
                    .as_deref()
                    .and_then(|id| self.config.find_proxy(id))
                    .map(|p| p.name.as_str())
                    .unwrap_or("Sin proxy");

                egui::ComboBox::from_id_source("combo_proxy")
                    .width(ui.available_width().max(120.0))
                    .selected_text(current_proxy_name)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.selected_proxy_id, None, "Sin proxy");

                        for proxy in &self.config.proxy_configs {
                            ui.selectable_value(
                                &mut self.selected_proxy_id,
                                Some(proxy.id.clone()),
                                &proxy.name,
                            );
                        }
                    });

                if self.selected_proxy_id != prev_proxy {
                    self.save_config();
                }
            });
    }

    fn action_button(&mut self, ui: &mut Ui) {
        let is_active = self.vpn_status.is_active();
        let has_profile = self.selected_profile_id.is_some();

        let (button_text, button_fill, button_stroke) = if is_active {
            (
                "Desconectar",
                Color32::from_rgb(170, 57, 57),
                Color32::from_rgb(207, 91, 91),
            )
        } else {
            (
                "Conectar",
                Color32::from_rgb(34, 139, 93),
                Color32::from_rgb(52, 176, 117),
            )
        };

        ui.vertical_centered(|ui| {
            let button_width = (ui.available_width() * 0.72).clamp(160.0, 240.0);

            if ui
                .add_enabled(
                    has_profile,
                    egui::Button::new(
                        RichText::new(button_text)
                            .size(14.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(button_fill)
                    .stroke(egui::Stroke::new(1.0, button_stroke))
                    .min_size(egui::vec2(button_width, 38.0)),
                )
                .clicked()
            {
                if is_active {
                    self.vpn.disconnect();
                } else {
                    self.do_connect();
                }
            }
        });

        if !has_profile && !self.config.vpn_profiles.is_empty() {
            ui.add_space(6.0);
            ui.colored_label(
                Color32::from_rgb(232, 182, 75),
                "Selecciona un perfil VPN para continuar.",
            );
        }
    }

    fn logs_card(&mut self, ui: &mut Ui) {
        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(18, 23, 31))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Registro de conexión")
                            .size(14.0)
                            .strong()
                            .color(Color32::from_rgb(232, 236, 242)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("🗑").on_hover_text("Limpiar logs").clicked() {
                            self.logs.clear();
                        }

                        ui.add_space(6.0);

                        ui.label(
                            RichText::new(format!("{} líneas", self.logs.len()))
                                .size(11.5)
                                .color(Color32::from_rgb(145, 154, 168)),
                        );
                    });
                });

                ui.add_space(8.0);

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .max_height(220.0)
                    .show(ui, |ui| {
                        let mono = FontId::monospace(12.0);

                        if self.logs.is_empty() {
                            ui.label(
                                RichText::new("Todavía no hay registros de conexión.")
                                    .size(12.0)
                                    .color(Color32::from_rgb(145, 154, 168)),
                            );
                        } else {
                            for line in &self.logs {
                                let color = log_line_color(line);
                                ui.label(RichText::new(line).font(mono.clone()).color(color));
                            }
                        }
                    });
            });
    }

    fn do_connect(&mut self) {
        let profile_id = match &self.selected_profile_id {
            Some(id) => id.clone(),
            None => return,
        };

        let profile = match self.config.find_profile(&profile_id).cloned() {
            Some(profile) => profile,
            None => {
                self.notify_error("Perfil VPN no encontrado.");
                return;
            }
        };

        if let Err(err) = profile.validate() {
            self.notify_error(err);
            return;
        }

        let proxy = self
            .selected_proxy_id
            .as_deref()
            .and_then(|id| self.config.find_proxy(id))
            .cloned();

        if let Some(selected_proxy) = &proxy {
            if let Err(err) = selected_proxy.validate() {
                self.notify_error(err);
                return;
            }
        }

        self.logs.clear();
        self.vpn.connect(profile, proxy);
    }
}

fn compact_label(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .strong()
            .size(12.5)
            .color(Color32::from_rgb(225, 230, 238)),
    );
    ui.add_space(2.0);
}
