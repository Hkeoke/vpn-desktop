use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke, Ui};

use super::state::{App, FormAction};
use crate::config::{AppConfig, VpnProfile};

impl App {
    pub fn ui_profiles(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                header(ui);

                let form_result = if let Some((form, is_new)) = self.profile_form.take() {
                    Some(render_profile_form(ui, form, is_new))
                } else {
                    None
                };

                match form_result {
                    Some(FormAction::Save { value, is_new }) => {
                        if let Err(err) = value.validate() {
                            self.notify_error(err);
                            self.profile_form = Some((value, is_new));
                        } else {
                            let profile_id = value.id.clone();
                            self.config.upsert_profile(value);

                            if self.selected_profile_id.is_none() {
                                self.selected_profile_id = Some(profile_id);
                            }

                            self.save_config();
                        }
                    }
                    Some(FormAction::Cancel) => {}
                    Some(FormAction::Continue { value, is_new }) => {
                        self.profile_form = Some((value, is_new));
                    }
                    None => {}
                }

                if self.profile_form.is_none() {
                    let add_button = egui::Button::new(
                        RichText::new("➕ Nuevo perfil")
                            .strong()
                            .color(Color32::from_rgb(245, 247, 250)),
                    )
                    .fill(Color32::from_rgb(44, 118, 255))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(66, 138, 255)))
                    .min_size(egui::vec2(ui.available_width(), 34.0));

                    if ui.add(add_button).clicked() {
                        self.profile_form = Some((VpnProfile::new(), true));
                    }

                    ui.add_space(10.0);
                }

                if self.config.vpn_profiles.is_empty() && self.profile_form.is_none() {
                    empty_state(ui);
                    return;
                }

                let mut edit_id: Option<String> = None;
                let mut delete_id: Option<String> = None;

                for profile in &self.config.vpn_profiles {
                    egui::Frame::group(ui.style())
                        .fill(Color32::from_rgb(26, 31, 40))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(48, 56, 72)))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("🔐 {}", profile.name))
                                            .strong()
                                            .size(15.0)
                                            .color(Color32::from_rgb(238, 242, 248)),
                                    );

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let delete_button = egui::Button::new(
                                                RichText::new("🗑")
                                                    .color(Color32::from_rgb(250, 250, 250)),
                                            )
                                            .fill(Color32::from_rgb(168, 54, 54))
                                            .stroke(Stroke::new(
                                                1.0,
                                                Color32::from_rgb(190, 72, 72),
                                            ))
                                            .min_size(egui::vec2(32.0, 32.0));

                                            if ui
                                                .add(delete_button)
                                                .on_hover_text("Borrar perfil")
                                                .clicked()
                                            {
                                                delete_id = Some(profile.id.clone());
                                            }

                                            let edit_button = egui::Button::new(
                                                RichText::new("✏")
                                                    .color(Color32::from_rgb(236, 240, 248)),
                                            )
                                            .fill(Color32::from_rgb(44, 52, 68))
                                            .stroke(Stroke::new(
                                                1.0,
                                                Color32::from_rgb(72, 84, 104),
                                            ))
                                            .min_size(egui::vec2(32.0, 32.0));

                                            if ui
                                                .add(edit_button)
                                                .on_hover_text("Editar perfil")
                                                .clicked()
                                            {
                                                edit_id = Some(profile.id.clone());
                                            }
                                        },
                                    );
                                });

                                ui.add_space(4.0);

                                let file_name = std::path::Path::new(&profile.config_file)
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| "sin archivo importado".to_string());

                                ui.label(
                                    RichText::new(format!("📄 {}", file_name))
                                        .color(Color32::from_rgb(170, 180, 195)),
                                );

                                if !profile.username.trim().is_empty() {
                                    ui.label(
                                        RichText::new(format!("👤 {}", profile.username))
                                            .color(Color32::from_rgb(170, 180, 195)),
                                    );
                                }

                                let dns_text = if profile.use_update_resolv_conf {
                                    "✔ DNS administrado con update-resolv-conf"
                                } else {
                                    "✖ DNS administrado manualmente"
                                };

                                let dns_color = if profile.use_update_resolv_conf {
                                    Color32::from_rgb(80, 190, 120)
                                } else {
                                    Color32::from_rgb(220, 160, 70)
                                };

                                ui.label(RichText::new(dns_text).color(dns_color));
                            });
                        });

                    ui.add_space(8.0);
                }

                if let Some(id) = edit_id {
                    if let Some(profile) = self.config.find_profile(&id).cloned() {
                        self.profile_form = Some((profile, false));
                    }
                }

                if let Some(id) = delete_id {
                    if let Some(profile) = self.config.find_profile(&id).cloned() {
                        if let Err(err) = AppConfig::cleanup_profile_assets(&profile) {
                            self.notify_error(format!(
                                "El perfil se eliminó, pero hubo un problema al limpiar sus archivos: {}",
                                err
                            ));
                        }
                    }

                    self.config.remove_profile(&id);

                    if self.selected_profile_id.as_deref() == Some(id.as_str()) {
                        self.selected_profile_id =
                            self.config.vpn_profiles.first().map(|p| p.id.clone());
                    }

                    self.save_config();
                }
            });
    }
}

fn header(ui: &mut Ui) {
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(24, 28, 36))
        .stroke(Stroke::new(1.0, Color32::from_rgb(52, 60, 76)))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("🗂 Perfiles VPN")
                        .size(19.0)
                        .strong()
                        .color(Color32::from_rgb(235, 240, 248)),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new(
                        "Gestiona perfiles OpenVPN con archivo importado, usuario y contraseña.",
                    )
                    .size(12.0)
                    .color(Color32::from_rgb(170, 180, 195)),
                );
            });
        });

    ui.add_space(10.0);
}

fn empty_state(ui: &mut Ui) {
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(24, 28, 36))
        .stroke(Stroke::new(1.0, Color32::from_rgb(52, 60, 76)))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("No hay perfiles VPN")
                        .strong()
                        .size(16.0)
                        .color(Color32::from_rgb(230, 235, 242)),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Pulsa “Nuevo perfil” para crear el primero.")
                        .color(Color32::from_rgb(160, 170, 185)),
                );
                ui.add_space(8.0);
            });
        });
}

fn render_profile_form(ui: &mut Ui, mut form: VpnProfile, is_new: bool) -> FormAction<VpnProfile> {
    let title = if is_new {
        "Nuevo perfil VPN"
    } else {
        "Editar perfil VPN"
    };

    let mut save = false;
    let mut cancel = false;

    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(24, 29, 38))
        .stroke(Stroke::new(1.0, Color32::from_rgb(52, 60, 76)))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(format!("📝 {}", title))
                        .strong()
                        .size(16.0)
                        .color(Color32::from_rgb(238, 242, 248)),
                );
                ui.separator();

                compact_label(ui, "Nombre");
                ui.add(
                    egui::TextEdit::singleline(&mut form.name)
                        .hint_text("Ej. Oficina principal")
                        .desired_width(ui.available_width()),
                );

                ui.add_space(8.0);

                compact_label(ui, "Fichero .ovpn");
                let imported_name = std::path::Path::new(&form.config_file)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Ningún fichero importado".to_string());

                ui.label(
                    RichText::new(format!("📄 {}", imported_name))
                        .color(Color32::from_rgb(180, 190, 205)),
                );

                let import_button = egui::Button::new(
                    RichText::new("📂 Importar .ovpn")
                        .strong()
                        .color(Color32::from_rgb(245, 247, 250)),
                )
                .fill(Color32::from_rgb(44, 118, 255))
                .stroke(Stroke::new(1.0, Color32::from_rgb(66, 138, 255)))
                .min_size(egui::vec2(ui.available_width(), 32.0));

                if ui.add(import_button).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Seleccionar fichero .ovpn")
                        .add_filter("OpenVPN Config", &["ovpn", "conf"])
                        .pick_file()
                    {
                        match AppConfig::import_profile_config(&form.id, &path) {
                            Ok(target) => {
                                form.config_file = target.display().to_string();
                            }
                            Err(err) => {
                                ui.colored_label(
                                    Color32::from_rgb(220, 80, 80),
                                    format!("Error al importar: {}", err),
                                );
                            }
                        }
                    }
                }

                ui.add_space(8.0);

                compact_label(ui, "Usuario VPN");
                ui.add(
                    egui::TextEdit::singleline(&mut form.username)
                        .hint_text("usuario")
                        .desired_width(ui.available_width()),
                );

                ui.add_space(8.0);

                compact_label(ui, "Contraseña VPN");
                ui.add(
                    egui::TextEdit::singleline(&mut form.password)
                        .password(true)
                        .hint_text("contraseña")
                        .desired_width(ui.available_width()),
                );

                ui.add_space(8.0);

                ui.checkbox(
                    &mut form.use_update_resolv_conf,
                    "Actualizar DNS con update-resolv-conf",
                );

                if form.use_update_resolv_conf {
                    ui.label(
                        RichText::new(
                            "Se añadirán los scripts de actualización DNS al ejecutar OpenVPN.",
                        )
                        .size(11.5)
                        .color(Color32::from_rgb(165, 175, 190)),
                    );
                }

                ui.add_space(10.0);
                ui.separator();

                let can_save = !form.name.trim().is_empty() && !form.config_file.trim().is_empty();

                let save_button = egui::Button::new(
                    RichText::new("💾 Guardar")
                        .strong()
                        .color(Color32::from_rgb(245, 247, 250)),
                )
                .fill(Color32::from_rgb(44, 118, 255))
                .stroke(Stroke::new(1.0, Color32::from_rgb(66, 138, 255)))
                .min_size(egui::vec2(ui.available_width(), 32.0));

                if ui.add_enabled(can_save, save_button).clicked() {
                    save = true;
                }

                ui.add_space(6.0);

                let cancel_button = egui::Button::new(
                    RichText::new("✖ Cancelar").color(Color32::from_rgb(230, 235, 242)),
                )
                .fill(Color32::from_rgb(44, 52, 68))
                .stroke(Stroke::new(1.0, Color32::from_rgb(72, 84, 104)))
                .min_size(egui::vec2(ui.available_width(), 32.0));

                if ui.add(cancel_button).clicked() {
                    cancel = true;
                }
            });
        });

    ui.add_space(10.0);

    if save {
        FormAction::Save {
            value: form,
            is_new,
        }
    } else if cancel {
        FormAction::Cancel
    } else {
        FormAction::Continue {
            value: form,
            is_new,
        }
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
