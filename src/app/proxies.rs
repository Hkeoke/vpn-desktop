use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke, Ui};

use super::state::{App, FormAction};
use crate::config::{AppConfig, ProxyAuthMethod, ProxyConfig};
use egui_phosphor::regular;

impl App {
    pub fn ui_proxies(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("{} Proxies", regular::GLOBE_HEMISPHERE_WEST))
                        .size(20.0)
                        .strong()
                        .color(Color32::from_rgb(235, 240, 255)),
                );
                ui.label(
                    RichText::new(
                        "Gestiona proxies HTTP con un layout compacto para ventanas estrechas.",
                    )
                    .size(12.0)
                    .color(Color32::from_rgb(170, 178, 191)),
                );
                ui.add_space(8.0);

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
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(format!("{} Nuevo proxy", regular::PLUS_CIRCLE))
                                    .strong(),
                            )
                            .fill(Color32::from_rgb(44, 102, 194))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(72, 132, 224)))
                            .min_size(egui::vec2(ui.available_width(), 34.0)),
                        )
                        .clicked()
                    {
                        self.proxy_form = Some((ProxyConfig::new(), true));
                    }

                    ui.add_space(10.0);
                }

                if self.config.proxy_configs.is_empty() && self.proxy_form.is_none() {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(
                            Color32::from_rgb(150, 150, 150),
                            "No hay proxies configurados. Pulsa 'Nuevo proxy' para crear uno.",
                        );
                    });
                    return;
                }

                let mut edit_id: Option<String> = None;
                let mut delete_id: Option<String> = None;

                for proxy in &self.config.proxy_configs {
                    egui::Frame::group(ui.style())
                        .fill(Color32::from_rgb(28, 33, 43))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(52, 60, 76)))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        regular::GLOBE_HEMISPHERE_WEST,
                                        proxy.name
                                    ))
                                    .strong()
                                    .size(15.0)
                                    .color(Color32::from_rgb(240, 244, 255)),
                                );

                                ui.add_space(4.0);
                                ui.small(format!(
                                    "{} {}:{}",
                                    regular::DESKTOP,
                                    proxy.host,
                                    proxy.port
                                ));
                                ui.small(format!(
                                    "{} {}",
                                    regular::LOCK,
                                    proxy.auth_method.display_name()
                                ));

                                if proxy.auth_method.needs_auth_file() && !proxy.username.is_empty()
                                {
                                    ui.small(format!("{} {}", regular::USER, proxy.username));
                                }

                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(format!(
                                                    "{} Editar",
                                                    regular::PENCIL_SIMPLE
                                                ))
                                                .color(Color32::from_rgb(235, 240, 255)),
                                            )
                                            .fill(Color32::from_rgb(46, 54, 68))
                                            .stroke(Stroke::new(
                                                1.0,
                                                Color32::from_rgb(75, 86, 104),
                                            ))
                                            .min_size(egui::vec2(0.0, 30.0)),
                                        )
                                        .clicked()
                                    {
                                        edit_id = Some(proxy.id.clone());
                                    }

                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(format!("{} Borrar", regular::TRASH))
                                                    .color(Color32::WHITE),
                                            )
                                            .fill(Color32::from_rgb(160, 52, 52))
                                            .stroke(Stroke::new(
                                                1.0,
                                                Color32::from_rgb(190, 78, 78),
                                            ))
                                            .min_size(egui::vec2(0.0, 30.0)),
                                        )
                                        .clicked()
                                    {
                                        delete_id = Some(proxy.id.clone());
                                    }
                                });
                            });
                        });

                    ui.add_space(6.0);
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
        format!("{} Nuevo proxy", regular::GLOBE)
    } else {
        format!("{} Editar proxy", regular::PENCIL_SIMPLE)
    };

    let mut save = false;
    let mut cancel = false;

    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(24, 29, 38))
        .stroke(Stroke::new(1.0, Color32::from_rgb(56, 66, 84)))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .strong()
                    .size(16.0)
                    .color(Color32::from_rgb(240, 244, 255)),
            );
            ui.separator();

            compact_field(ui, "Nombre");
            ui.add(
                egui::TextEdit::singleline(&mut form.name)
                    .hint_text("Proxy oficina")
                    .desired_width(ui.available_width()),
            );

            ui.add_space(8.0);

            compact_field(ui, "Servidor");
            ui.add(
                egui::TextEdit::singleline(&mut form.host)
                    .hint_text("10.0.0.1 / proxy.empresa.com")
                    .desired_width(ui.available_width()),
            );

            ui.add_space(8.0);

            compact_field(ui, "Puerto");
            {
                let mut port = i32::from(form.port);
                ui.add(
                    egui::DragValue::new(&mut port)
                        .range(1..=65535)
                        .speed(1.0)
                        .prefix(format!("{} ", regular::PLUG)),
                );
                form.port = port.clamp(1, 65535) as u16;
            }

            ui.add_space(8.0);

            compact_field(ui, "Autenticación");
            ui.vertical(|ui| {
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
                ui.add_space(8.0);

                compact_field(ui, "Usuario proxy");
                ui.add(
                    egui::TextEdit::singleline(&mut form.username)
                        .hint_text("usuario del proxy")
                        .desired_width(ui.available_width()),
                );

                ui.add_space(8.0);

                compact_field(ui, "Contraseña proxy");
                ui.add(
                    egui::TextEdit::singleline(&mut form.password)
                        .password(true)
                        .hint_text("contraseña del proxy")
                        .desired_width(ui.available_width()),
                );

                ui.add_space(8.0);

                ui.small(format!(
                    "{} Las credenciales se guardan de forma segura y se convierten en un fichero temporal al conectar.",
                    regular::SHIELD_CHECK
                ));
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            let can_save = !form.name.trim().is_empty() && !form.host.trim().is_empty();

            if ui
                .add_enabled(
                    can_save,
                    egui::Button::new(
                        RichText::new(format!("{} Guardar", regular::FLOPPY_DISK))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(42, 116, 196))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(68, 144, 228)))
                    .min_size(egui::vec2(ui.available_width(), 34.0)),
                )
                .clicked()
            {
                save = true;
            }

            ui.add_space(6.0);

            if ui
                .add(
                    egui::Button::new(
                        RichText::new(format!("{} Cancelar", regular::X))
                            .color(Color32::from_rgb(230, 235, 245)),
                    )
                    .fill(Color32::from_rgb(52, 58, 70))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(82, 92, 108)))
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

fn compact_field(ui: &mut Ui, label: &str) {
    ui.label(
        RichText::new(label)
            .strong()
            .size(12.5)
            .color(Color32::from_rgb(215, 221, 230)),
    );
}
