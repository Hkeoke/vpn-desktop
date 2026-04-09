use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke, Ui};

use super::state::{App, FormAction};
use crate::config::{AppConfig, ProxyAuthMethod, ProxyConfig};
use egui_phosphor::regular;

impl App {
    pub fn ui_proxies(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let form_result = if let Some((form, is_new)) = self.proxy_form.take() {
                    Some(render_proxy_form(ui, form, is_new))
                } else {
                    None
                };

                match form_result {
                    Some(FormAction::Save { value, is_new }) => {
                        if let Err(err) = value.validate() {
                            self.notify_error(err);
                            self.proxy_form = Some((value, is_new));
                        } else {
                            let proxy_id = value.id.clone();
                            self.config.upsert_proxy(value);

                            if self.selected_proxy_id.is_none() {
                                self.selected_proxy_id = Some(proxy_id);
                            }

                            self.save_config();
                        }
                    }
                    Some(FormAction::Cancel) => {}
                    Some(FormAction::Continue { value, is_new }) => {
                        self.proxy_form = Some((value, is_new));
                    }
                    None => {}
                }

                if self.proxy_form.is_none() {
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
                            .on_hover_text("Nuevo proxy")
                            .clicked()
                        {
                            self.proxy_form = Some((ProxyConfig::new(), true));
                        }
                    });

                    ui.add_space(4.0);
                }

                if self.config.proxy_configs.is_empty() && self.proxy_form.is_none() {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(regular::GLOBE_HEMISPHERE_WEST)
                                .size(36.0)
                                .color(Color32::from_rgb(100, 116, 139)),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Sin proxies configurados")
                                .size(15.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                        ui.label(
                            RichText::new("Crea uno si tu red lo requiere.")
                                .size(12.0)
                                .color(Color32::from_rgb(100, 116, 139)),
                        );
                    });
                    return;
                }

                let mut edit_id: Option<String> = None;
                let mut delete_id: Option<String> = None;

                for proxy in &self.config.proxy_configs {
                    let is_selected = self.selected_proxy_id.as_deref() == Some(&proxy.id);

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
                                    RichText::new(regular::GLOBE_HEMISPHERE_WEST)
                                        .size(18.0)
                                        .color(icon_color),
                                );

                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;

                                    ui.label(
                                        RichText::new(&proxy.name)
                                            .strong()
                                            .size(14.0)
                                            .color(Color32::from_rgb(226, 232, 240)),
                                    );

                                    ui.label(
                                        RichText::new(format!("{}:{}", proxy.host, proxy.port))
                                            .size(11.5)
                                            .color(Color32::from_rgb(100, 116, 139)),
                                    );
                                });

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
                                            delete_id = Some(proxy.id.clone());
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
                                            edit_id = Some(proxy.id.clone());
                                        }
                                    },
                                );
                            });
                        });

                    ui.add_space(4.0);
                }

                if let Some(id) = edit_id {
                    if let Some(proxy) = self.config.find_proxy(&id).cloned() {
                        self.proxy_form = Some((proxy, false));
                    }
                }

                if let Some(id) = delete_id {
                    if let Some(proxy) = self.config.find_proxy(&id).cloned() {
                        if let Err(err) = AppConfig::delete_proxy_password(&proxy) {
                            self.notify_error(format!(
                                "No se pudo eliminar el secreto del proxy '{}': {}",
                                proxy.name, err
                            ));
                        }
                    }

                    self.config.remove_proxy(&id);

                    if self.selected_proxy_id.as_deref() == Some(id.as_str()) {
                        self.selected_proxy_id =
                            self.config.proxy_configs.first().map(|p| p.id.clone());
                    }

                    self.save_config();
                }
            });
    }
}

fn render_proxy_form(ui: &mut Ui, mut form: ProxyConfig, is_new: bool) -> FormAction<ProxyConfig> {
    let title = if is_new {
        format!("{} Nuevo proxy", regular::PLUS_CIRCLE)
    } else {
        format!("{} Editar proxy", regular::PENCIL_SIMPLE)
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
                    .hint_text("Proxy oficina")
                    .desired_width(ui.available_width()),
            );

            ui.add_space(6.0);

            field_label(ui, "Servidor");
            ui.add(
                egui::TextEdit::singleline(&mut form.host)
                    .hint_text("10.0.0.1 / proxy.empresa.com")
                    .desired_width(ui.available_width()),
            );

            ui.add_space(6.0);

            field_label(ui, "Puerto");
            {
                let mut port = i32::from(form.port);
                ui.add(
                    egui::DragValue::new(&mut port)
                        .range(1..=65535)
                        .speed(1.0),
                );
                form.port = port.clamp(1, 65535) as u16;
            }

            ui.add_space(6.0);

            field_label(ui, "Autenticación");
            ui.horizontal(|ui| {
                for method in ProxyAuthMethod::all() {
                    let selected = &form.auth_method == method;
                    if ui
                        .add(egui::RadioButton::new(selected, method.display_name()))
                        .clicked()
                    {
                        form.auth_method = method.clone();
                        if !method.needs_auth_file() {
                            form.username.clear();
                            form.password.clear();
                        }
                    }
                }
            });

            if form.auth_method.needs_auth_file() {
                ui.add_space(6.0);

                field_label(ui, "Usuario proxy");
                ui.add(
                    egui::TextEdit::singleline(&mut form.username)
                        .hint_text("usuario")
                        .desired_width(ui.available_width()),
                );

                ui.add_space(6.0);

                field_label(ui, "Contraseña proxy");
                ui.add(
                    egui::TextEdit::singleline(&mut form.password)
                        .password(true)
                        .hint_text("contraseña")
                        .desired_width(ui.available_width()),
                );
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            let can_save = !form.name.trim().is_empty() && !form.host.trim().is_empty();

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
