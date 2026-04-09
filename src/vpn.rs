use crossbeam_channel::{bounded, Receiver, Sender};
use std::time::{Duration, Instant};

use crate::config::{ProxyAuthMethod, ProxyConfig, VpnProfile};
use crate::ipc::{
    ConnectRequest, HealthStatus, HelperHealthReport, ProxyAuthMethod as IpcProxyAuthMethod,
    ProxyRuntime,
};
use crate::ipc_client::{GuiEvent, IpcClient};

#[derive(Debug, Clone, PartialEq)]
pub enum VpnStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

impl VpnStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, VpnStatus::Connecting | VpnStatus::Connected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelperDiagnosticKind {
    Unknown,
    Connected,
    MissingSocket,
    PermissionDenied,
    ConnectionLost,
    OpenVpnMissing,
    OpenVpnUnavailable,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperStatus {
    pub kind: HelperDiagnosticKind,
    pub message: String,
    pub details: Option<String>,
}

impl HelperStatus {
    pub fn unknown() -> Self {
        Self {
            kind: HelperDiagnosticKind::Unknown,
            message: "Comprobando disponibilidad del helper…".to_string(),
            details: None,
        }
    }

    pub fn connected() -> Self {
        Self {
            kind: HelperDiagnosticKind::Connected,
            message: "Helper root disponible.".to_string(),
            details: None,
        }
    }

    pub fn from_helper_error(message: impl Into<String>) -> Self {
        let message = message.into();
        let lower = message.to_ascii_lowercase();

        let kind = if lower.contains("no se encontró el socket del helper")
            || lower.contains("asegúrate de que el servicio root está instalado y activo")
        {
            HelperDiagnosticKind::MissingSocket
        } else if lower.contains("no tienes permisos para usar el helper")
            || lower.contains("revisa el grupo/permisos del socket")
        {
            HelperDiagnosticKind::PermissionDenied
        } else if lower.contains("se perdió la conexión con el helper")
            || lower.contains("se cerró la conexión con el helper")
        {
            HelperDiagnosticKind::ConnectionLost
        } else {
            HelperDiagnosticKind::Other
        };

        Self {
            kind,
            message,
            details: None,
        }
    }

    pub fn from_runtime_error(message: impl Into<String>) -> Self {
        let message = message.into();
        let lower = message.to_ascii_lowercase();

        let kind = if lower.contains("no se pudo lanzar openvpn")
            && (lower.contains("no such file")
                || lower.contains("not found")
                || lower.contains("falló spawn de openvpn"))
        {
            HelperDiagnosticKind::OpenVpnMissing
        } else {
            HelperDiagnosticKind::Other
        };

        Self {
            kind,
            message,
            details: None,
        }
    }

    pub fn from_health_report(report: &HelperHealthReport) -> Self {
        let has_error = report
            .items
            .iter()
            .any(|item| matches!(item.status, HealthStatus::Error));
        let has_warn = report
            .items
            .iter()
            .any(|item| matches!(item.status, HealthStatus::Warn));

        let openvpn_item = report.items.iter().find(|item| item.key == "openvpn");
        let socket_item = report.items.iter().find(|item| item.key == "socket");
        let details = Some(format_health_report_details(report));

        if let Some(item) = openvpn_item {
            if matches!(item.status, HealthStatus::Error) {
                return Self {
                    kind: HelperDiagnosticKind::OpenVpnUnavailable,
                    message: item.summary.clone(),
                    details,
                };
            }
        }

        if let Some(item) = socket_item {
            if matches!(item.status, HealthStatus::Error | HealthStatus::Warn) {
                let lower = item.summary.to_ascii_lowercase();
                let kind = if lower.contains("permiso") || lower.contains("grupo") {
                    HelperDiagnosticKind::PermissionDenied
                } else if lower.contains("no existe")
                    || lower.contains("ausente")
                    || lower.contains("socket")
                {
                    HelperDiagnosticKind::MissingSocket
                } else {
                    HelperDiagnosticKind::Other
                };

                return Self {
                    kind,
                    message: item.summary.clone(),
                    details,
                };
            }
        }

        if has_error {
            return Self {
                kind: HelperDiagnosticKind::Other,
                message: "El helper detectó errores en el entorno.".to_string(),
                details,
            };
        }

        if has_warn {
            return Self {
                kind: HelperDiagnosticKind::Other,
                message: "El helper detectó avisos en el entorno.".to_string(),
                details,
            };
        }

        Self {
            kind: HelperDiagnosticKind::Connected,
            message: "Helper root disponible y entorno básico listo.".to_string(),
            details,
        }
    }
}

#[derive(Debug)]
pub enum VpnEvent {
    Log(String),
    StatusChanged(VpnStatus),
    HelperStatusChanged(HelperStatus),
    HealthReportChanged(HelperHealthReport),
}

enum WorkerCmd {
    Connect {
        profile: VpnProfile,
        proxy: Option<ProxyConfig>,
    },
    Disconnect,
    Shutdown,
}

pub struct VpnManager {
    cmd_tx: Sender<WorkerCmd>,
    event_rx: Receiver<VpnEvent>,
}

impl VpnManager {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = bounded::<WorkerCmd>(8);
        let (event_tx, event_rx) = bounded::<VpnEvent>(4096);

        std::thread::Builder::new()
            .name("vpn-ipc-worker".into())
            .spawn(move || worker_loop(cmd_rx, event_tx))
            .expect("No se pudo crear el hilo del worker VPN");

        Self { cmd_tx, event_rx }
    }

    pub fn connect(&self, profile: VpnProfile, proxy: Option<ProxyConfig>) {
        let _ = self.cmd_tx.send(WorkerCmd::Connect { profile, proxy });
    }

    pub fn disconnect(&self) {
        let _ = self.cmd_tx.send(WorkerCmd::Disconnect);
    }

    pub fn poll(&self) -> Vec<VpnEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Drop for VpnManager {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(WorkerCmd::Shutdown);
    }
}

fn worker_loop(cmd_rx: Receiver<WorkerCmd>, event_tx: Sender<VpnEvent>) {
    let client = IpcClient::new();
    let mut helper_available = false;
    let mut last_helper_status = HelperStatus::unknown();

    send_helper_status(&event_tx, &mut last_helper_status, HelperStatus::unknown());
    client.request_status();
    let mut last_ping_at = Instant::now();

    loop {
        for event in client.poll() {
            match event {
                GuiEvent::ConnectedToHelper => {
                    helper_available = true;
                    send_helper_status(
                        &event_tx,
                        &mut last_helper_status,
                        HelperStatus::connected(),
                    );
                    send_log(&event_tx, "Conectado con el helper root de VPN.");
                    client.request_health();
                }
                GuiEvent::HelperUnavailable(message) => {
                    helper_available = false;
                    send_helper_status(
                        &event_tx,
                        &mut last_helper_status,
                        HelperStatus::from_helper_error(message.clone()),
                    );
                    send_log(&event_tx, &message);
                    send_status(&event_tx, VpnStatus::Failed(message));
                }
                GuiEvent::Snapshot(snapshot) => {
                    if let Some(profile_name) = snapshot.active_profile_name {
                        send_log(
                            &event_tx,
                            &format!("Estado actualizado para el perfil '{}'.", profile_name),
                        );
                    }
                }
                GuiEvent::Health(report) => {
                    send_helper_status(
                        &event_tx,
                        &mut last_helper_status,
                        HelperStatus::from_health_report(&report),
                    );
                    let _ = event_tx.send(VpnEvent::HealthReportChanged(report));
                }
                GuiEvent::StatusChanged(status) => {
                    send_status(&event_tx, status);
                }
                GuiEvent::Log(line) => {
                    if line
                        .to_ascii_lowercase()
                        .contains("no se pudo lanzar openvpn")
                    {
                        send_helper_status(
                            &event_tx,
                            &mut last_helper_status,
                            HelperStatus::from_runtime_error(line.clone()),
                        );
                    }
                    send_log(&event_tx, &line);
                }
                GuiEvent::Error(message) => {
                    send_helper_status(
                        &event_tx,
                        &mut last_helper_status,
                        HelperStatus::from_runtime_error(message.clone()),
                    );
                    send_log(&event_tx, &message);
                    send_status(&event_tx, VpnStatus::Failed(message));
                }
                GuiEvent::Pong => {
                    helper_available = true;
                    send_helper_status(
                        &event_tx,
                        &mut last_helper_status,
                        HelperStatus::connected(),
                    );
                }
                GuiEvent::Ack(message) => {
                    send_log(&event_tx, &message);
                }
            }
        }

        if last_ping_at.elapsed() >= Duration::from_secs(3) {
            client.ping();
            client.request_health();
            if !helper_available {
                client.request_status();
            }
            last_ping_at = Instant::now();
        }

        match cmd_rx.recv_timeout(Duration::from_millis(150)) {
            Ok(WorkerCmd::Connect { profile, proxy }) => {
                match build_connect_request(profile, proxy) {
                    Ok(request) => {
                        if !helper_available {
                            client.request_status();
                        }

                        send_log(&event_tx, "Solicitando conexión al helper root...");
                        send_status(&event_tx, VpnStatus::Connecting);
                        client.connect_vpn(request);
                    }
                    Err(message) => {
                        send_log(&event_tx, &message);
                        send_status(&event_tx, VpnStatus::Failed(message));
                    }
                }
            }
            Ok(WorkerCmd::Disconnect) => {
                send_log(&event_tx, "Solicitando desconexión al helper root...");
                client.disconnect_vpn();
            }
            Ok(WorkerCmd::Shutdown) => {
                break;
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn build_connect_request(
    profile: VpnProfile,
    proxy: Option<ProxyConfig>,
) -> Result<ConnectRequest, String> {
    if let Err(err) = profile.validate() {
        return Err(err);
    }

    let proxy_runtime = match proxy {
        Some(proxy_cfg) => {
            if let Err(err) = proxy_cfg.validate() {
                return Err(err);
            }

            Some(ProxyRuntime {
                name: proxy_cfg.name,
                host: proxy_cfg.host,
                port: proxy_cfg.port,
                auth_method: map_proxy_auth_method(&proxy_cfg.auth_method),
                username: proxy_cfg.username,
                password: proxy_cfg.password,
            })
        }
        None => None,
    };

    Ok(ConnectRequest {
        profile_name: profile.name,
        config_path: std::path::PathBuf::from(profile.config_file),
        username: profile.username,
        password: profile.password,
        proxy: proxy_runtime,
        use_update_resolv_conf: profile.use_update_resolv_conf,
    })
}

fn map_proxy_auth_method(method: &ProxyAuthMethod) -> IpcProxyAuthMethod {
    match method {
        ProxyAuthMethod::None => IpcProxyAuthMethod::None,
        ProxyAuthMethod::Basic => IpcProxyAuthMethod::Basic,
        ProxyAuthMethod::Ntlm => IpcProxyAuthMethod::Ntlm,
    }
}

#[inline]
fn send_log(tx: &Sender<VpnEvent>, msg: &str) {
    let _ = tx.send(VpnEvent::Log(msg.to_string()));
}

#[inline]
fn send_status(tx: &Sender<VpnEvent>, status: VpnStatus) {
    let _ = tx.send(VpnEvent::StatusChanged(status));
}

#[inline]
fn send_helper_status(tx: &Sender<VpnEvent>, last_status: &mut HelperStatus, status: HelperStatus) {
    if *last_status != status {
        *last_status = status.clone();
        let _ = tx.send(VpnEvent::HelperStatusChanged(status));
    }
}

fn format_health_report_details(report: &HelperHealthReport) -> String {
    report
        .items
        .iter()
        .map(|item| {
            let status = match item.status {
                HealthStatus::Ok => "ok",
                HealthStatus::Warn => "warn",
                HealthStatus::Error => "error",
            };

            match &item.details {
                Some(details) if !details.trim().is_empty() => {
                    format!("{}={} ({})", item.key, status, details)
                }
                _ => format!("{}={} ({})", item.key, status, item.summary),
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}
