use eframe::egui::{self, Color32, RichText, Rounding, ScrollArea, Stroke, Ui};

use super::state::App;
use super::widgets::{
    helper_error_colors, helper_ok_colors, helper_status_card, helper_warn_colors, info_card,
};
use crate::config::AppConfig;
use crate::ipc::DEFAULT_SOCKET_PATH;
use crate::ipc::{HealthStatus, HelperHealthReport};
use crate::vpn::{HelperDiagnosticKind, HelperStatus};

impl App {
    pub fn ui_settings(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("⚙ Ajustes");
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Consulta cómo funciona la app en Linux, dónde guarda sus datos y qué componentes del sistema necesita para operar correctamente. Aquí también puedes revisar rápidamente el estado esperado del helper privilegiado y del socket IPC.",
                    )
                    .size(12.0)
                    .color(Color32::from_rgb(170, 178, 191)),
                );
                ui.add_space(10.0);

                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(28, 33, 42))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(52, 60, 74)))
                    .rounding(Rounding::same(10.0))
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("🛡 Helper privilegiado")
                                .strong()
                                .size(15.0),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "La interfaz gráfica se ejecuta como usuario normal. Las operaciones privilegiadas de OpenVPN se delegan al helper root `vpn-desktopd`, que expone un socket Unix local.",
                            )
                            .size(11.5)
                            .color(Color32::from_rgb(175, 182, 194)),
                        );
                        ui.add_space(8.0);

                        info_row(ui, "Helper", "/usr/libexec/vpn-desktopd");
                        ui.add_space(6.0);
                        info_row(ui, "Socket IPC", DEFAULT_SOCKET_PATH);
                        ui.add_space(6.0);
                        info_row(ui, "GUI", "/usr/bin/vpn-desktop");
                    });

                ui.add_space(10.0);

                info_card(
                    ui,
                    "🛈 Cómo se conecta ahora la aplicación",
                    "La GUI ya no debería lanzar OpenVPN mediante pkexec o sudo en cada uso. En su lugar, se conecta al helper del sistema a través de un socket Unix. Si no puedes conectar, normalmente el problema estará en la instalación del helper, los permisos del socket o la activación del servicio/socket de systemd.",
                    Color32::from_rgb(100, 180, 255),
                );

                ui.add_space(10.0);

                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(28, 33, 42))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(52, 60, 74)))
                    .rounding(Rounding::same(10.0))
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("🩺 Diagnóstico rápido del helper")
                                .strong()
                                .size(15.0),
                        );
                        ui.add_space(6.0);

                        let (title, summary, details, accent, bg_fill, border) =
                            helper_health_card_parts(&self.helper_status);

                        helper_status_card(
                            ui,
                            title,
                            summary,
                            details,
                            accent,
                            bg_fill,
                            border,
                        );

                        ui.add_space(8.0);

                        helper_diag_card(
                            ui,
                            "Socket IPC esperado",
                            "Ruta configurada del socket del helper. Este valor debe coincidir con el socket expuesto por `vpn-desktopd`.",
                            DEFAULT_SOCKET_PATH,
                            Color32::from_rgb(100, 180, 255),
                        );
                        ui.add_space(8.0);

                        if let Some(report) = &self.helper_health_report {
                            render_helper_environment_report(ui, report);
                            ui.add_space(8.0);
                        }
                        ui.add_space(8.0);

                        helper_diag_card(
                            ui,
                            "Grupo esperado para acceso",
                            "Si la app no puede conectar por permisos, revisa que tu usuario pertenezca al grupo autorizado para usar el socket del helper.",
                            "Grupo esperado: vpn-desktop",
                            Color32::from_rgb(255, 200, 90),
                        );
                        ui.add_space(8.0);

                        helper_diag_card(
                            ui,
                            "Comprobación de systemd",
                            "Si el socket no existe, normalmente el helper no está instalado, el paquete no terminó bien o la unit de systemd no está habilitada. El health check real del helper se refleja en la tarjeta superior.",
                            "Comprueba: systemctl status vpn-desktopd.socket",
                            Color32::from_rgb(255, 140, 140),
                        );
                        ui.add_space(8.0);

                        helper_diag_card(
                            ui,
                            "Comprobación de OpenVPN",
                            "Si el helper arranca pero la conexión falla al lanzar el backend, asegúrate de que `openvpn` esté instalado y disponible en el sistema.",
                            "Comprueba: openvpn --version",
                            Color32::from_rgb(120, 220, 120),
                        );
                    });

                ui.add_space(10.0);

                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(28, 33, 42))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(52, 60, 74)))
                    .rounding(Rounding::same(10.0))
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("🧩 Requisitos del sistema")
                                .strong()
                                .size(15.0),
                        );
                        ui.add_space(6.0);

                        bullet(
                            ui,
                            "El paquete `openvpn` debe estar instalado en el sistema.",
                        );
                        bullet(
                            ui,
                            "El helper `vpn-desktopd` debe estar instalado y accesible por systemd.",
                        );
                        bullet(
                            ui,
                            "El socket `/run/vpn-desktopd.sock` debe existir con permisos de grupo adecuados.",
                        );
                        bullet(
                            ui,
                            "Tu usuario debe pertenecer al grupo autorizado para usar el socket del helper.",
                        );
                        bullet(
                            ui,
                            "Si el instalador añadió tu usuario a un grupo del sistema, puede hacer falta cerrar sesión y volver a entrar.",
                        );
                    });

                ui.add_space(10.0);

                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(45, 40, 25))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(90, 80, 45)))
                    .rounding(Rounding::same(10.0))
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("🧰 Instalación recomendada")
                                .strong()
                                .size(15.0),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "Instala la aplicación con el script de empaquetado Linux para registrar systemd, copiar el helper y preparar el acceso al socket.",
                            )
                            .size(11.5)
                            .color(Color32::from_rgb(210, 204, 170)),
                        );
                        ui.add_space(8.0);

                        ui.label(
                            RichText::new("sudo ./packaging/linux/install.sh")
                                .monospace()
                                .size(11.5)
                                .color(Color32::from_rgb(220, 220, 120)),
                        );
                    });

                ui.add_space(12.0);

                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(28, 33, 42))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(52, 60, 74)))
                    .rounding(Rounding::same(10.0))
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("ℹ Información local")
                                .strong()
                                .size(15.0),
                        );
                        ui.add_space(8.0);

                        let config_path = AppConfig::config_path();
                        let profiles_dir = AppConfig::profiles_dir();

                        ui.label(
                            RichText::new("📁 Fichero de configuración:")
                                .strong()
                                .size(12.5),
                        );
                        ui.add_space(2.0);
                        copyable_path(ui, config_path.display().to_string());

                        ui.add_space(8.0);

                        ui.label(
                            RichText::new("🗂 Directorio de perfiles gestionados:")
                                .strong()
                                .size(12.5),
                        );
                        ui.add_space(2.0);
                        copyable_path(ui, profiles_dir.display().to_string());

                        ui.add_space(8.0);

                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new("🏷 Versión:")
                                    .strong()
                                    .size(12.5),
                            );
                            ui.monospace(env!("CARGO_PKG_VERSION"));
                        });
                    });
            });
    }
}

fn helper_health_card_parts(
    status: &HelperStatus,
) -> (
    &'static str,
    &'static str,
    Option<&str>,
    Color32,
    Color32,
    Color32,
) {
    match status.kind {
        HelperDiagnosticKind::Connected => {
            let (accent, bg_fill, border) = helper_ok_colors();
            (
                "✅ Helper disponible",
                "La GUI puede comunicarse con el helper privilegiado.",
                Some(status.message.as_str()),
                accent,
                bg_fill,
                border,
            )
        }
        HelperDiagnosticKind::MissingSocket => {
            let (accent, bg_fill, border) = helper_warn_colors();
            (
                "⚠ Socket del helper no encontrado",
                "No se encontró el socket esperado del helper root.",
                Some(status.message.as_str()),
                accent,
                bg_fill,
                border,
            )
        }
        HelperDiagnosticKind::PermissionDenied => {
            let (accent, bg_fill, border) = helper_warn_colors();
            (
                "🔐 Permisos insuficientes",
                "Tu usuario no puede acceder al socket del helper.",
                Some(status.message.as_str()),
                accent,
                bg_fill,
                border,
            )
        }
        HelperDiagnosticKind::ConnectionLost => {
            let (accent, bg_fill, border) = helper_warn_colors();
            (
                "⚠ Conexión con el helper interrumpida",
                "La GUI perdió la comunicación con `vpn-desktopd`.",
                Some(status.message.as_str()),
                accent,
                bg_fill,
                border,
            )
        }
        HelperDiagnosticKind::OpenVpnMissing => {
            let (accent, bg_fill, border) = helper_error_colors();
            (
                "📦 OpenVPN no disponible",
                "El helper no pudo lanzar `openvpn` correctamente.",
                Some(status.message.as_str()),
                accent,
                bg_fill,
                border,
            )
        }
        HelperDiagnosticKind::OpenVpnUnavailable => {
            let (accent, bg_fill, border) = helper_error_colors();
            (
                "📦 OpenVPN no detectado en el entorno",
                "El helper no detecta `openvpn` correctamente en el sistema.",
                Some(status.message.as_str()),
                accent,
                bg_fill,
                border,
            )
        }
        HelperDiagnosticKind::Other => {
            let (accent, bg_fill, border) = helper_warn_colors();
            (
                "⚠ Estado del helper con incidencias",
                "Se detectó un problema relacionado con el helper o su runtime.",
                Some(status.message.as_str()),
                accent,
                bg_fill,
                border,
            )
        }
        HelperDiagnosticKind::Unknown => {
            let (accent, bg_fill, border) = helper_warn_colors();
            (
                "⏳ Comprobando helper",
                "La aplicación está esperando confirmar la disponibilidad del helper privilegiado.",
                Some(status.message.as_str()),
                accent,
                bg_fill,
                border,
            )
        }
    }
}

fn render_helper_environment_report(ui: &mut Ui, report: &HelperHealthReport) {
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(28, 33, 42))
        .stroke(Stroke::new(1.0, Color32::from_rgb(52, 60, 74)))
        .rounding(Rounding::same(10.0))
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new("🧪 Health check real del helper")
                    .strong()
                    .size(14.5),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Este bloque viene del helper `vpn-desktopd` y refleja comprobaciones reales del entorno del sistema.",
                )
                .size(11.5)
                .color(Color32::from_rgb(175, 182, 194)),
            );

            for item in &report.items {
                ui.add_space(8.0);
                let (accent, bg_fill, border) = match item.status {
                    HealthStatus::Ok => helper_ok_colors(),
                    HealthStatus::Warn => helper_warn_colors(),
                    HealthStatus::Error => helper_error_colors(),
                };

                helper_status_card(
                    ui,
                    item.summary.as_str(),
                    item.key.as_str(),
                    item.details.as_deref(),
                    accent,
                    bg_fill,
                    border,
                );
            }
        });
}

fn helper_diag_card(ui: &mut Ui, title: &str, text: &str, value: &str, accent: Color32) {
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(35, 40, 50))
        .stroke(Stroke::new(1.0, Color32::from_rgb(52, 60, 76)))
        .rounding(Rounding::same(10.0))
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.colored_label(accent, RichText::new(title).strong().size(13.0));
            ui.add_space(4.0);
            ui.label(
                RichText::new(text)
                    .size(11.5)
                    .color(Color32::from_rgb(210, 214, 220)),
            );
            ui.add_space(6.0);
            ui.monospace(value);
        });
}

fn info_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!("{}:", label))
                .strong()
                .size(12.5)
                .color(Color32::from_rgb(225, 230, 238)),
        );
        ui.monospace(value);
    });
}

fn bullet(ui: &mut Ui, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("•")
                .strong()
                .color(Color32::from_rgb(100, 180, 255)),
        );
        ui.label(
            RichText::new(text)
                .size(11.5)
                .color(Color32::from_rgb(175, 182, 194)),
        );
    });
    ui.add_space(4.0);
}

fn copyable_path(ui: &mut Ui, value: String) {
    ui.vertical(|ui| {
        ui.monospace(&value);

        if ui
            .add(egui::Button::new("📋 Copiar ruta").min_size(egui::vec2(110.0, 28.0)))
            .clicked()
        {
            ui.output_mut(|o| {
                o.copied_text = value.clone();
            });
        }
    });
}
