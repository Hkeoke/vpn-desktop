use crate::config::{AppConfig, ProxyConfig, VpnProfile};
use crate::ipc::HelperHealthReport;
use crate::vpn::{HelperStatus, VpnEvent, VpnManager, VpnStatus};
use eframe::egui::{self, Color32, FontDefinitions, Visuals};

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Connect,
    Profiles,
    Proxies,
}

pub enum FormAction<T> {
    Save { value: T, is_new: bool },
    Cancel,
    Continue { value: T, is_new: bool },
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub text: String,
    pub is_error: bool,
}

pub struct App {
    pub config: AppConfig,
    pub vpn: VpnManager,
    pub current_tab: Tab,
    pub selected_profile_id: Option<String>,
    pub selected_proxy_id: Option<String>,
    pub vpn_status: VpnStatus,
    pub helper_status: HelperStatus,
    pub helper_health_report: Option<HelperHealthReport>,
    pub logs: Vec<String>,
    pub profile_form: Option<(VpnProfile, bool)>,
    pub proxy_form: Option<(ProxyConfig, bool)>,
    pub notification: Option<Notification>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_theme_and_fonts(&cc.egui_ctx);

        let config = AppConfig::load();

        let selected_profile_id = config.vpn_profiles.first().map(|p| p.id.clone());
        let selected_proxy_id = config.proxy_configs.first().map(|p| p.id.clone());

        Self {
            config,
            vpn: VpnManager::new(),
            current_tab: Tab::Connect,
            selected_profile_id,
            selected_proxy_id,
            vpn_status: VpnStatus::Disconnected,
            helper_status: HelperStatus::unknown(),
            helper_health_report: None,
            logs: Vec::new(),
            profile_form: None,
            proxy_form: None,
            notification: None,
        }
    }

    pub fn save_config(&mut self) {
        if let Err(e) = self.config.save() {
            self.notify_error(format!("Error al guardar la configuración: {}", e));
        }
    }

    pub fn notify_error(&mut self, msg: impl Into<String>) {
        self.notification = Some(Notification {
            text: msg.into(),
            is_error: true,
        });
    }

    pub fn poll_vpn_events(&mut self) {
        for event in self.vpn.poll() {
            match event {
                VpnEvent::Log(line) => {
                    self.logs.push(line);
                    if self.logs.len() > 3000 {
                        self.logs.drain(..500);
                    }
                }
                VpnEvent::StatusChanged(status) => {
                    self.vpn_status = status;
                }
                VpnEvent::HelperStatusChanged(status) => {
                    self.helper_status = status;
                }
                VpnEvent::HealthReportChanged(status) => {
                    self.helper_health_report = Some(status);
                }
            }
        }
    }
}

fn install_theme_and_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);

    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(Color32::from_rgb(232, 236, 241));
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(15, 23, 42);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 41, 59);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(37, 99, 235);
    visuals.widgets.active.bg_fill = Color32::from_rgb(29, 78, 216);
    visuals.widgets.open.bg_fill = Color32::from_rgb(30, 41, 59);
    visuals.selection.bg_fill = Color32::from_rgb(37, 99, 235);
    visuals.panel_fill = Color32::from_rgb(8, 15, 28);
    visuals.window_fill = Color32::from_rgb(11, 18, 32);
    visuals.extreme_bg_color = Color32::from_rgb(3, 7, 18);
    visuals.faint_bg_color = Color32::from_rgb(20, 28, 45);
    visuals.code_bg_color = Color32::from_rgb(15, 23, 42);
    visuals.hyperlink_color = Color32::from_rgb(96, 165, 250);
    visuals.warn_fg_color = Color32::from_rgb(251, 191, 36);
    visuals.error_fg_color = Color32::from_rgb(248, 113, 113);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(12.0);
    style.visuals.window_rounding = egui::Rounding::same(10.0);
    style.visuals.menu_rounding = egui::Rounding::same(10.0);
    style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(10.0);
    style.visuals.widgets.inactive.rounding = egui::Rounding::same(10.0);
    style.visuals.widgets.hovered.rounding = egui::Rounding::same(10.0);
    style.visuals.widgets.active.rounding = egui::Rounding::same(10.0);
    style.visuals.widgets.open.rounding = egui::Rounding::same(10.0);
    ctx.set_style(style);
}
