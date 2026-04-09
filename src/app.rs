mod connect;
mod profiles;
mod proxies;
mod state;
pub mod tray;
mod widgets;

pub use state::App;

use eframe::egui;

use crate::vpn::VpnStatus;
use state::Tab;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(tray_ctx) = &self.tray_ctx {
            let mut guard = tray_ctx.lock().unwrap();
            if guard.is_none() {
                *guard = Some(ctx.clone());
            }
        }

        if let Some(rx) = &self.tray_rx {
            while let Ok(action) = rx.try_recv() {
                match action {
                    crate::app::tray::TrayAction::ToggleWindow => {
                        self.is_window_visible = !self.is_window_visible;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.is_window_visible));
                    }
                    crate::app::tray::TrayAction::Quit => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }

        self.poll_vpn_events();

        if matches!(
            self.vpn_status,
            VpnStatus::Connecting | VpnStatus::Connected
        ) {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }

        let visuals = egui::Visuals {
            dark_mode: true,
            override_text_color: Some(egui::Color32::from_rgb(232, 236, 241)),
            panel_fill: egui::Color32::from_rgb(15, 23, 32),
            window_fill: egui::Color32::from_rgb(20, 29, 40),
            faint_bg_color: egui::Color32::from_rgb(28, 39, 52),
            extreme_bg_color: egui::Color32::from_rgb(10, 16, 24),
            code_bg_color: egui::Color32::from_rgb(18, 27, 37),
            warn_fg_color: egui::Color32::from_rgb(255, 193, 92),
            error_fg_color: egui::Color32::from_rgb(255, 107, 107),
            hyperlink_color: egui::Color32::from_rgb(120, 180, 255),
            selection: egui::style::Selection {
                bg_fill: egui::Color32::from_rgb(54, 114, 255),
                stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(190, 214, 255)),
            },
            widgets: egui::style::Widgets {
                noninteractive: egui::style::WidgetVisuals {
                    weak_bg_fill: egui::Color32::from_rgb(26, 35, 47),
                    bg_fill: egui::Color32::from_rgb(26, 35, 47),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 57, 74)),
                    fg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 225, 232)),
                    rounding: egui::Rounding::same(10.0),
                    expansion: 0.0,
                },
                inactive: egui::style::WidgetVisuals {
                    weak_bg_fill: egui::Color32::from_rgb(28, 39, 52),
                    bg_fill: egui::Color32::from_rgb(28, 39, 52),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 65, 84)),
                    fg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(230, 233, 238)),
                    rounding: egui::Rounding::same(10.0),
                    expansion: 0.0,
                },
                hovered: egui::style::WidgetVisuals {
                    weak_bg_fill: egui::Color32::from_rgb(37, 52, 70),
                    bg_fill: egui::Color32::from_rgb(37, 52, 70),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(88, 118, 255)),
                    fg_stroke: egui::Stroke::new(1.0, egui::Color32::WHITE),
                    rounding: egui::Rounding::same(10.0),
                    expansion: 1.0,
                },
                active: egui::style::WidgetVisuals {
                    weak_bg_fill: egui::Color32::from_rgb(50, 72, 104),
                    bg_fill: egui::Color32::from_rgb(50, 72, 104),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(115, 147, 255)),
                    fg_stroke: egui::Stroke::new(1.0, egui::Color32::WHITE),
                    rounding: egui::Rounding::same(10.0),
                    expansion: 1.0,
                },
                open: egui::style::WidgetVisuals {
                    weak_bg_fill: egui::Color32::from_rgb(31, 44, 59),
                    bg_fill: egui::Color32::from_rgb(31, 44, 59),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(72, 98, 130)),
                    fg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(235, 240, 248)),
                    rounding: egui::Rounding::same(10.0),
                    expansion: 0.0,
                },
            },
            ..egui::Visuals::dark()
        };
        ctx.set_visuals(visuals);

        egui::TopBottomPanel::top("tab_bar")
            .exact_height(82.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(12, 18, 27))
                    .inner_margin(egui::Margin::symmetric(8.0, 8.0)),
            )
            .show(ctx, |ui| {
                let tab_button = |ui: &mut egui::Ui,
                                  current_tab: &mut Tab,
                                  tab: Tab,
                                  icon: &str,
                                  label: &str| {
                    let selected = *current_tab == tab;
                    let text = format!("{} {}", icon, label);
                    let fill = if selected {
                        egui::Color32::from_rgb(47, 76, 147)
                    } else {
                        egui::Color32::from_rgb(24, 34, 46)
                    };

                    let stroke = if selected {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(110, 146, 255))
                    } else {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 60, 78))
                    };

                    let response = ui.add(
                        egui::Button::new(
                            egui::RichText::new(text)
                                .size(11.5)
                                .strong()
                                .color(egui::Color32::from_rgb(236, 240, 246)),
                        )
                        .fill(fill)
                        .stroke(stroke)
                        .rounding(egui::Rounding::same(10.0))
                        .min_size(egui::vec2(84.0, 28.0)),
                    );

                    if response.clicked() {
                        *current_tab = tab;
                    }
                };

                ui.horizontal(|_ui| {});

                ui.add_space(6.0);

                ui.horizontal_wrapped(|ui| {
                    tab_button(ui, &mut self.current_tab, Tab::Connect, "🔌", "Conectar");
                    tab_button(ui, &mut self.current_tab, Tab::Profiles, "📁", "Perfiles");
                    tab_button(ui, &mut self.current_tab, Tab::Proxies, "🌐", "Proxies");
                });
            });

        if self.notification.is_some() {
            let dismiss = egui::TopBottomPanel::top("notif_bar")
                .frame(
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(15, 23, 32))
                        .inner_margin(egui::Margin::symmetric(12.0, 6.0)),
                )
                .show(ctx, |ui| {
                    let mut dismiss = false;

                    if let Some(notif) = &self.notification {
                        let (accent, icon, bg_fill, border) = if notif.is_error {
                            (
                                egui::Color32::from_rgb(255, 107, 107),
                                "⨯",
                                egui::Color32::from_rgb(61, 28, 33),
                                egui::Color32::from_rgb(125, 52, 61),
                            )
                        } else {
                            (
                                egui::Color32::from_rgb(74, 222, 128),
                                "✓",
                                egui::Color32::from_rgb(25, 54, 40),
                                egui::Color32::from_rgb(52, 120, 84),
                            )
                        };

                        egui::Frame::none()
                            .fill(bg_fill)
                            .stroke(egui::Stroke::new(1.0, border))
                            .rounding(egui::Rounding::same(12.0))
                            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.colored_label(
                                        accent,
                                        egui::RichText::new(icon).size(18.0).strong(),
                                    );
                                    ui.label(
                                        egui::RichText::new(&notif.text)
                                            .size(13.0)
                                            .color(egui::Color32::from_rgb(235, 239, 244)),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("✕").color(
                                                            egui::Color32::from_rgb(215, 223, 232),
                                                        ),
                                                    )
                                                    .fill(egui::Color32::from_rgba_unmultiplied(
                                                        255, 255, 255, 10,
                                                    ))
                                                    .stroke(egui::Stroke::NONE)
                                                    .rounding(egui::Rounding::same(8.0)),
                                                )
                                                .clicked()
                                            {
                                                dismiss = true;
                                            }
                                        },
                                    );
                                });
                            });
                    }

                    dismiss
                })
                .inner;

            if dismiss {
                self.notification = None;
            }
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(15, 23, 32))
                    .inner_margin(egui::Margin::symmetric(10.0, 10.0)),
            )
            .show(ctx, |ui| match self.current_tab {
                Tab::Connect => self.ui_connect(ui),
                Tab::Profiles => self.ui_profiles(ui),
                Tab::Proxies => self.ui_proxies(ui),
            });
    }
}
