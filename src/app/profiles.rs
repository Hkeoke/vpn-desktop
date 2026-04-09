use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke, Ui};
use egui_phosphor::regular;

use super::state::{App, FormAction};
use crate::config::{AppConfig, VpnProfile};

impl App {
    pub fn ui_profiles(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
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
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(regular::PLUS)
                                        .size(16.0)
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(37, 99, 235))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(59, 130, 246)))
                                .min_size(egui::vec2(34.0, 34.0)),
                            )
                            .on_hover_text("Nuevo perfil")
                            .clicked()
                        {
                            self.profile_form = Some((VpnProfile::new(), true));
                        }
                    });

                    ui.add_space(4.0);
                }

                if self.config.vpn_profiles.is_empty() && self.profile_form.is_none() {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(regular::SHIELD_WARNING)
                                .size(36.0)
                                .color(Color32::from_rgb(100, 116, 139)),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Sin perfiles VPN")
                                .size(15.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                        ui.label(
                            RichText::new("Crea uno para empezar a conectar.")
                                .size(12.0)
                                .color(Color32::from_rgb(100, 116, 139)),
                        );
                    });
                    return;
                }

                let mut edit_id: Option<String> = None;
                let mut delete_id: Option<String> = None;

                for profile in &self.config.vpn_profiles {
                    let is_selected = self.selected_profile_id.as_deref() == Some(&profile.id);

                    let (bg, border) = if is_selected {
                        (
                            Color32::from_rgb(23, 37, 58),
                            Color32::from_rgb(59, 130, 246),
                        )
                    } else {
                        (
                            Color32::from_rgb(22, 28, 38),
                            Color32::from_rgb(40, 48, 62),
                        )
                    };

                    egui::Frame::none()
                        .fill(bg)
                        .stroke(Stroke::new(1.0, border))
                        .rounding(egui::Rounding::same(10.0))
                        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let icon_color = if is_selected {
                                    Color32::from_rgb(96, 165, 250)
                                } else {
                                    Color32::from_rgb(100, 116, 139)
                                };

                                ui.label(
                                    RichText::new(regular::SHIELD_CHECK)
                                        .size(18.0)
                                        .color(icon_color),
                                );

                                ui.label(
                                    RichText::new(&profile.name)
                                        .strong()
                                        .size(14.0)
                                        .color(Color32::from_rgb(226, 232, 240)),
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;

                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new(regular::TRASH)
                                                        .size(14.0)
                                                        .color(Color32::from_rgb(239, 68, 68)),
                                                )
                                                .fill(Color32::TRANSPARENT)
                                                .stroke(Stroke::NONE)
                                                .min_size(egui::vec2(28.0, 28.0)),
                                            )
                                            .on_hover_text("Eliminar")
                                            .clicked()
                                        {
                                            delete_id = Some(profile.id.clone());
                                        }

                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new(regular::PENCIL_SIMPLE)
                                                        .size(14.0)
                                                        .color(Color32::from_rgb(148, 163, 184)),
                                                )
                                                .fill(Color32::TRANSPARENT)
                                                .stroke(Stroke::NONE)
                                                .min_size(egui::vec2(28.0, 28.0)),
                                            )
                                            .on_hover_text("Editar")
                                            .clicked()
                                        {
                                            edit_id = Some(profile.id.clone());
                                        }
                                    },
                                );
                            });
                        });

                    ui.add_space(4.0);
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
                                "Error al limpiar archivos del perfil: {}",
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

fn render_profile_form(ui: &mut Ui, mut form: VpnProfile, is_new: bool) -> FormAction<VpnProfile> {
    let title = if is_new {
        format!("{} Nuevo perfil", regular::PLUS_CIRCLE)
    } else {
        format!("{} Editar perfil", regular::PENCIL_SIMPLE)
    };

    let mut save = false;
    let mut cancel = false;

    egui::Frame::none()
        .fill(Color32::from_rgb(20, 27, 38))
        .stroke(Stroke::new(1.0, Color32::from_rgb(45, 55, 72)))
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::symmetric(14.0, 14.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .strong()
                    .size(15.0)
                    .color(Color32::from_rgb(226, 232, 240)),
            );
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            field_label(ui, "Nombre");
            ui.add(
                egui::TextEdit::singleline(&mut form.name)
                    .hint_text("Ej. Oficina principal")
                    .desired_width(ui.available_width()),
            );

            ui.add_space(6.0);

            field_label(ui, "Fichero .ovpn");
            let imported_name = std::path::Path::new(&form.config_file)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Sin fichero".to_string());

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&imported_name)
                        .size(12.0)
                        .color(Color32::from_rgb(148, 163, 184)),
                );
            });

            if ui
                .add(
                    egui::Button::new(
                        RichText::new(format!("{} Importar .ovpn", regular::UPLOAD_SIMPLE))
                            .strong()
                            .size(12.5)
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(37, 99, 235))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(59, 130, 246)))
                    .min_size(egui::vec2(ui.available_width(), 30.0)),
                )
                .clicked()
            {
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
                                Color32::from_rgb(239, 68, 68),
                                format!("Error: {}", err),
                            );
                        }
                    }
                }
            }

            ui.add_space(6.0);

            field_label(ui, "Usuario VPN");
            ui.add(
                egui::TextEdit::singleline(&mut form.username)
                    .hint_text("usuario")
                    .desired_width(ui.available_width()),
            );

            ui.add_space(6.0);

            field_label(ui, "Contraseña VPN");
            ui.add(
                egui::TextEdit::singleline(&mut form.password)
                    .password(true)
                    .hint_text("contraseña")
                    .desired_width(ui.available_width()),
            );

            ui.add_space(6.0);

            ui.checkbox(
                &mut form.use_update_resolv_conf,
                RichText::new("Gestionar DNS automáticamente")
                    .size(12.5)
                    .color(Color32::from_rgb(203, 213, 225)),
            );

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            let can_save = !form.name.trim().is_empty() && !form.config_file.trim().is_empty();

            if ui
                .add_enabled(
                    can_save,
                    egui::Button::new(
                        RichText::new(format!("{} Guardar", regular::FLOPPY_DISK))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(37, 99, 235))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(59, 130, 246)))
                    .min_size(egui::vec2(ui.available_width(), 34.0)),
                )
                .clicked()
            {
                save = true;
            }

            ui.add_space(4.0);

            if ui
                .add(
                    egui::Button::new(
                        RichText::new(format!("{} Cancelar", regular::X))
                            .color(Color32::from_rgb(203, 213, 225)),
                    )
                    .fill(Color32::from_rgb(30, 38, 50))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(51, 65, 85)))
                    .min_size(egui::vec2(ui.available_width(), 34.0)),
                )
                .clicked()
            {
                cancel = true;
            }
        });

    ui.add_space(8.0);

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

fn field_label(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .strong()
            .size(12.0)
            .color(Color32::from_rgb(148, 163, 184)),
    );
}
