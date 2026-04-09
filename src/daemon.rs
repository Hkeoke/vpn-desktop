use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use tempfile::{NamedTempFile, TempDir};

use crate::ipc::{
    Ack, ClientMessage, ConnectRequest, ErrorCode, ErrorMessage, HealthStatus, HelloOk,
    HelperHealthItem, HelperHealthReport, LogEntry, LogStream, ServerMessage, SessionSnapshot,
    VpnPhase, DEFAULT_SOCKET_PATH, PROTOCOL_VERSION,
};

const LISTEN_FDS_START: RawFd = 3;
const MAX_LOG_LINES: usize = 1000;

struct SharedState {
    snapshot: Mutex<SessionSnapshot>,
    logs: Mutex<Vec<LogEntry>>,
    clients: Mutex<HashMap<u64, Sender<ServerMessage>>>,
    next_log_seq: AtomicU64,
}

impl SharedState {
    fn new() -> Self {
        Self {
            snapshot: Mutex::new(SessionSnapshot::idle()),
            logs: Mutex::new(Vec::new()),
            clients: Mutex::new(HashMap::new()),
            next_log_seq: AtomicU64::new(1),
        }
    }

    fn snapshot(&self) -> SessionSnapshot {
        self.snapshot
            .lock()
            .expect("snapshot mutex poisoned")
            .clone()
    }

    fn logs_tail(&self, requested: usize) -> Vec<LogEntry> {
        let logs = self.logs.lock().expect("logs mutex poisoned");
        let count = requested.min(logs.len());
        logs[logs.len().saturating_sub(count)..].to_vec()
    }

    fn register_client(&self, client_id: u64, tx: Sender<ServerMessage>) {
        self.clients
            .lock()
            .expect("clients mutex poisoned")
            .insert(client_id, tx);
    }

    fn unregister_client(&self, client_id: u64) {
        self.clients
            .lock()
            .expect("clients mutex poisoned")
            .remove(&client_id);
    }

    fn broadcast(&self, message: ServerMessage) {
        let clients: Vec<(u64, Sender<ServerMessage>)> = self
            .clients
            .lock()
            .expect("clients mutex poisoned")
            .iter()
            .map(|(id, tx)| (*id, tx.clone()))
            .collect();

        let mut stale = Vec::new();

        for (client_id, tx) in clients {
            if tx.send(message.clone()).is_err() {
                stale.push(client_id);
            }
        }

        if !stale.is_empty() {
            let mut guard = self.clients.lock().expect("clients mutex poisoned");
            for client_id in stale {
                guard.remove(&client_id);
            }
        }
    }

    fn set_snapshot(&self, snapshot: SessionSnapshot) {
        *self.snapshot.lock().expect("snapshot mutex poisoned") = snapshot;
    }

    fn set_phase(
        &self,
        phase: VpnPhase,
        active_profile_name: Option<String>,
        active_proxy_name: Option<String>,
        pid: Option<u32>,
        started_at_unix_ms: Option<u64>,
        last_error: Option<String>,
    ) {
        let snapshot = SessionSnapshot {
            phase,
            active_profile_name,
            active_proxy_name,
            pid,
            started_at_unix_ms,
            last_error,
            last_log_seq: self.last_log_seq(),
        };

        self.set_snapshot(snapshot.clone());
        self.broadcast(ServerMessage::StateChanged(snapshot));
    }

    fn append_log(&self, stream: LogStream, line: impl Into<String>) {
        let entry = LogEntry::new(self.next_log_seq(), stream, line);

        {
            let mut logs = self.logs.lock().expect("logs mutex poisoned");
            logs.push(entry.clone());
            if logs.len() > MAX_LOG_LINES {
                let overflow = logs.len() - MAX_LOG_LINES;
                logs.drain(0..overflow);
            }
        }

        {
            let mut snapshot = self.snapshot.lock().expect("snapshot mutex poisoned");
            snapshot.last_log_seq = entry.seq;
        }

        self.broadcast(ServerMessage::Log(entry));
    }

    fn next_log_seq(&self) -> u64 {
        self.next_log_seq.fetch_add(1, Ordering::Relaxed)
    }

    fn last_log_seq(&self) -> u64 {
        self.next_log_seq.load(Ordering::Relaxed).saturating_sub(1)
    }
}

struct SessionRuntime {
    child: Child,
    profile_name: String,
    proxy_name: Option<String>,
    pid: u32,
    started_at_unix_ms: u64,
    _runtime_dir: TempDir,
    _vpn_auth_file: NamedTempFile,
    _proxy_auth_file: Option<NamedTempFile>,
}

enum ManagerCommand {
    Connect(ConnectRequest),
    Disconnect,
    ProcessConnected,
    ProcessExited(Result<()>),
}

pub fn run() -> Result<()> {
    let shared = Arc::new(SharedState::new());
    let (manager_tx, manager_rx) = unbounded::<ManagerCommand>();

    {
        let shared = Arc::clone(&shared);
        let manager_tx = manager_tx.clone();
        thread::Builder::new()
            .name("vpn-desktopd-manager".into())
            .spawn(move || manager_loop(shared, manager_rx, manager_tx))
            .context("no se pudo arrancar el manager")?;
    }

    let listener = bind_listener().context("no se pudo preparar el socket del daemon")?;
    shared.append_log(
        LogStream::Internal,
        "vpn-desktopd listo para recibir conexiones",
    );

    let next_client_id = Arc::new(AtomicU64::new(1));

    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let client_id = next_client_id.fetch_add(1, Ordering::Relaxed);
                let shared = Arc::clone(&shared);
                let manager_tx = manager_tx.clone();

                thread::Builder::new()
                    .name(format!("vpn-desktopd-client-{client_id}"))
                    .spawn(move || {
                        if let Err(err) = handle_client(client_id, stream, shared, manager_tx) {
                            eprintln!("[vpn-desktopd] error en cliente {client_id}: {err:#}");
                        }
                    })
                    .ok();
            }
            Err(err) => {
                eprintln!("[vpn-desktopd] error aceptando conexión: {err}");
                thread::sleep(Duration::from_millis(200));
            }
        }
    }

    Ok(())
}

fn bind_listener() -> Result<UnixListener> {
    if let Some(listener) = try_listener_from_systemd_socket()? {
        return Ok(listener);
    }

    let socket_path = Path::new(DEFAULT_SOCKET_PATH);

    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("no se pudo crear '{}'", parent.display()))?;
    }

    if socket_path.exists() {
        fs::remove_file(socket_path)
            .with_context(|| format!("no se pudo eliminar '{}'", socket_path.display()))?;
    }

    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("no se pudo hacer bind en '{}'", socket_path.display()))?;

    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o660)).with_context(|| {
        format!(
            "no se pudieron ajustar permisos en '{}'",
            socket_path.display()
        )
    })?;

    Ok(listener)
}

fn try_listener_from_systemd_socket() -> Result<Option<UnixListener>> {
    let listen_pid = std::env::var("LISTEN_PID").ok();
    let listen_fds = std::env::var("LISTEN_FDS").ok();

    let current_pid = std::process::id().to_string();

    if listen_pid.as_deref() != Some(current_pid.as_str()) {
        return Ok(None);
    }

    let fds: i32 = match listen_fds {
        Some(value) => value
            .parse()
            .map_err(|e| anyhow!("LISTEN_FDS inválido: {e}"))?,
        None => return Ok(None),
    };

    if fds < 1 {
        return Ok(None);
    }

    let listener = unsafe { UnixListener::from_raw_fd(LISTEN_FDS_START) };
    Ok(Some(listener))
}

fn handle_client(
    client_id: u64,
    stream: UnixStream,
    shared: Arc<SharedState>,
    manager_tx: Sender<ManagerCommand>,
) -> Result<()> {
    let (server_tx, server_rx) = unbounded::<ServerMessage>();
    shared.register_client(client_id, server_tx.clone());

    let writer_stream = stream
        .try_clone()
        .context("no se pudo clonar el stream del cliente")?;

    let writer_handle = thread::Builder::new()
        .name(format!("vpn-desktopd-writer-{client_id}"))
        .spawn(move || client_writer_loop(writer_stream, server_rx))
        .context("no se pudo arrancar el writer del cliente")?;

    let mut reader = BufReader::new(stream);

    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .context("fallo leyendo del cliente")?;

        if bytes == 0 {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let message: ClientMessage =
            serde_json::from_str(line).with_context(|| format!("JSON inválido: {line}"))?;

        match message {
            ClientMessage::Hello(request) => {
                if request.protocol_version != PROTOCOL_VERSION {
                    let _ = server_tx.send(ServerMessage::Error(ErrorMessage {
                        code: ErrorCode::InvalidProtocol,
                        message: format!(
                            "Versión de protocolo no soportada: {} (esperada {})",
                            request.protocol_version, PROTOCOL_VERSION
                        ),
                    }));
                    continue;
                }

                let snapshot = shared.snapshot();
                let logs_tail = shared.logs_tail(request.want_logs_tail);

                let _ = server_tx.send(ServerMessage::HelloOk(HelloOk {
                    protocol_version: PROTOCOL_VERSION,
                    snapshot,
                    logs_tail,
                }));
            }
            ClientMessage::GetStatus => {
                let _ = server_tx.send(ServerMessage::Snapshot(shared.snapshot()));
            }
            ClientMessage::Connect(request) => {
                manager_tx
                    .send(ManagerCommand::Connect(request))
                    .context("no se pudo enviar connect al manager")?;

                let _ = server_tx.send(ServerMessage::Ack(Ack {
                    request: "connect".into(),
                    message: Some("Solicitud de conexión enviada".into()),
                }));
            }
            ClientMessage::Disconnect => {
                manager_tx
                    .send(ManagerCommand::Disconnect)
                    .context("no se pudo enviar disconnect al manager")?;

                let _ = server_tx.send(ServerMessage::Ack(Ack {
                    request: "disconnect".into(),
                    message: Some("Solicitud de desconexión enviada".into()),
                }));
            }
            ClientMessage::Ping => {
                let _ = server_tx.send(ServerMessage::Pong);
            }
            ClientMessage::GetHealth => {
                let _ = server_tx.send(ServerMessage::Health(collect_helper_health_report()));
            }
            ClientMessage::GetLogs { from_seq, limit } => {
                let logs = shared.logs_tail(limit.unwrap_or(200));
                let filtered = if let Some(seq) = from_seq {
                    logs.into_iter().filter(|entry| entry.seq >= seq).collect()
                } else {
                    logs
                };

                let _ = server_tx.send(ServerMessage::Logs { entries: filtered });
            }
        }
    }

    shared.unregister_client(client_id);
    drop(server_tx);
    let _ = writer_handle.join();
    Ok(())
}

fn client_writer_loop(stream: UnixStream, rx: Receiver<ServerMessage>) {
    let mut writer = stream;

    while let Ok(message) = rx.recv() {
        let serialized = match serde_json::to_string(&message) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if writer.write_all(serialized.as_bytes()).is_err() {
            break;
        }
        if writer.write_all(b"\n").is_err() {
            break;
        }
        if writer.flush().is_err() {
            break;
        }
    }
}

fn manager_loop(
    shared: Arc<SharedState>,
    cmd_rx: Receiver<ManagerCommand>,
    manager_tx: Sender<ManagerCommand>,
) {
    let mut session: Option<SessionRuntime> = None;

    loop {
        if let Some(runtime) = session.as_mut() {
            match runtime.child.try_wait() {
                Ok(Some(status)) => {
                    let result = if status.success() {
                        Ok(())
                    } else {
                        Err(anyhow!("openvpn terminó con estado {}", status))
                    };
                    let _ = manager_tx.send(ManagerCommand::ProcessExited(result));
                }
                Ok(None) => {}
                Err(err) => {
                    let _ = manager_tx.send(ManagerCommand::ProcessExited(Err(anyhow!(
                        "falló try_wait(): {err}"
                    ))));
                }
            }
        }

        match cmd_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(ManagerCommand::Connect(request)) => {
                if session.is_some() {
                    shared.append_log(
                        LogStream::Internal,
                        "Ya existe una sesión activa; ignorando nueva conexión",
                    );
                    shared.broadcast(ServerMessage::Error(ErrorMessage {
                        code: ErrorCode::Busy,
                        message: "Ya existe una VPN activa".into(),
                    }));
                    continue;
                }

                shared.append_log(
                    LogStream::Internal,
                    format!("Iniciando OpenVPN para '{}'", request.profile_name),
                );
                shared.set_phase(
                    VpnPhase::Connecting,
                    Some(request.profile_name.clone()),
                    request.proxy.as_ref().map(|p| p.name.clone()),
                    None,
                    None,
                    None,
                );

                match spawn_openvpn(&request, Arc::clone(&shared), manager_tx.clone()) {
                    Ok(runtime) => {
                        shared.set_phase(
                            VpnPhase::Connecting,
                            Some(runtime.profile_name.clone()),
                            runtime.proxy_name.clone(),
                            Some(runtime.pid),
                            Some(runtime.started_at_unix_ms),
                            None,
                        );
                        session = Some(runtime);
                    }
                    Err(err) => {
                        let msg = format!("No se pudo lanzar OpenVPN: {err:#}");
                        shared.append_log(LogStream::Internal, msg.clone());
                        shared.set_phase(VpnPhase::Failed, None, None, None, None, Some(msg));
                    }
                }
            }
            Ok(ManagerCommand::Disconnect) => {
                if let Some(mut runtime) = session.take() {
                    shared.append_log(LogStream::Internal, "Solicitando desconexión");
                    shared.set_phase(
                        VpnPhase::Disconnecting,
                        Some(runtime.profile_name.clone()),
                        runtime.proxy_name.clone(),
                        Some(runtime.pid),
                        Some(runtime.started_at_unix_ms),
                        None,
                    );

                    let stop_result =
                        stop_openvpn(&mut runtime.child, runtime.pid, Arc::clone(&shared));

                    match stop_result {
                        Ok(()) => {
                            shared.append_log(LogStream::Internal, "VPN desconectada");
                            shared.set_phase(VpnPhase::Idle, None, None, None, None, None);
                        }
                        Err(err) => {
                            let msg = format!("Error al detener OpenVPN: {err:#}");
                            shared.append_log(LogStream::Internal, msg.clone());
                            shared.set_phase(
                                VpnPhase::Failed,
                                Some(runtime.profile_name),
                                runtime.proxy_name,
                                None,
                                None,
                                Some(msg),
                            );
                        }
                    }
                } else {
                    shared.append_log(LogStream::Internal, "No hay VPN activa");
                    shared.broadcast(ServerMessage::Snapshot(shared.snapshot()));
                }
            }
            Ok(ManagerCommand::ProcessConnected) => {
                if let Some(runtime) = session.as_ref() {
                    shared.set_phase(
                        VpnPhase::Connected,
                        Some(runtime.profile_name.clone()),
                        runtime.proxy_name.clone(),
                        Some(runtime.pid),
                        Some(runtime.started_at_unix_ms),
                        None,
                    );
                }
            }
            Ok(ManagerCommand::ProcessExited(result)) => {
                let profile_name = session.as_ref().map(|s| s.profile_name.clone());
                let proxy_name = session.as_ref().and_then(|s| s.proxy_name.clone());
                session = None;

                match result {
                    Ok(()) => {
                        shared.append_log(LogStream::Internal, "OpenVPN terminó correctamente");
                        shared.set_phase(VpnPhase::Idle, None, None, None, None, None);
                    }
                    Err(err) => {
                        let msg = format!("OpenVPN terminó con error: {err:#}");
                        shared.append_log(LogStream::Internal, msg.clone());
                        shared.set_phase(
                            VpnPhase::Failed,
                            profile_name,
                            proxy_name,
                            None,
                            None,
                            Some(msg),
                        );
                    }
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }

        if let Some(runtime) = session.as_mut() {
            match runtime.child.try_wait() {
                Ok(Some(status)) => {
                    let result = if status.success() {
                        Ok(())
                    } else {
                        Err(anyhow!("openvpn terminó con estado {}", status))
                    };
                    let _ = manager_tx.send(ManagerCommand::ProcessExited(result));
                }
                Ok(None) => {}
                Err(err) => {
                    let _ = manager_tx.send(ManagerCommand::ProcessExited(Err(anyhow!(
                        "falló try_wait(): {err}"
                    ))));
                }
            }
        }
    }
}

fn collect_helper_health_report() -> HelperHealthReport {
    let socket_path = Path::new(DEFAULT_SOCKET_PATH);
    let socket_exists = socket_path.exists();
    let socket_metadata = fs::metadata(socket_path).ok();
    let socket_is_unix = socket_metadata
        .as_ref()
        .map(|metadata| metadata.file_type().is_socket())
        .unwrap_or(false);
    let socket_mode = socket_metadata
        .as_ref()
        .map(|metadata| format!("{:04o}", metadata.permissions().mode() & 0o7777));
    let socket_uid = socket_metadata.as_ref().map(|metadata| metadata.uid());
    let socket_gid = socket_metadata.as_ref().map(|metadata| metadata.gid());

    let socket_accessible = UnixStream::connect(DEFAULT_SOCKET_PATH).is_ok();

    let socket_summary = if socket_exists && socket_is_unix {
        "El socket del helper existe y tiene formato Unix.".to_string()
    } else if socket_exists {
        "La ruta del helper existe, pero no es un socket Unix.".to_string()
    } else {
        "No existe el socket esperado del helper.".to_string()
    };

    let socket_details = Some(match (socket_mode, socket_uid, socket_gid) {
        (Some(mode), Some(uid), Some(gid)) => {
            format!(
                "ruta={}; modo={}; uid={}; gid={}",
                DEFAULT_SOCKET_PATH, mode, uid, gid
            )
        }
        _ => format!("ruta={}; sin metadatos disponibles", DEFAULT_SOCKET_PATH),
    });

    let socket_status = if socket_exists && socket_is_unix {
        HealthStatus::Ok
    } else {
        HealthStatus::Error
    };

    let access_status = if socket_accessible {
        HealthStatus::Ok
    } else if socket_exists {
        HealthStatus::Warn
    } else {
        HealthStatus::Error
    };

    let access_summary = if socket_accessible {
        "El helper puede aceptar conexiones en el socket IPC.".to_string()
    } else if socket_exists {
        "El socket existe, pero esta comprobación no pudo abrir una conexión local.".to_string()
    } else {
        "No se puede abrir el socket porque no está presente.".to_string()
    };

    let openvpn_check = Command::new("openvpn").arg("--version").output();
    let (openvpn_status, openvpn_summary, openvpn_details) = match openvpn_check {
        Ok(output) if output.status.success() => {
            let first_line = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("Versión de OpenVPN detectada")
                .to_string();

            (
                HealthStatus::Ok,
                "OpenVPN está instalado y responde correctamente.".to_string(),
                Some(first_line),
            )
        }
        Ok(output) => (
            HealthStatus::Warn,
            "OpenVPN existe, pero devolvió un estado no exitoso.".to_string(),
            Some(format!("exit_status={}", output.status)),
        ),
        Err(err) => (
            HealthStatus::Error,
            "OpenVPN no está disponible o no puede ejecutarse.".to_string(),
            Some(err.to_string()),
        ),
    };

    let update_resolv_conf_path = "/etc/openvpn/update-resolv-conf";
    let update_resolv_conf_exists = Path::new(update_resolv_conf_path).exists();
    let update_resolv_conf_status = if update_resolv_conf_exists {
        HealthStatus::Ok
    } else {
        HealthStatus::Warn
    };
    let update_resolv_conf_summary = if update_resolv_conf_exists {
        "Se encontró el script update-resolv-conf.".to_string()
    } else {
        "No se encontró update-resolv-conf; algunos perfiles pueden no actualizar DNS.".to_string()
    };

    let socket_unit = "vpn-desktopd.socket";
    let service_unit = "vpn-desktopd.service";

    let socket_unit_active = systemd_unit_is_active(socket_unit);
    let service_unit_active = systemd_unit_is_active(service_unit);
    let socket_unit_enabled = systemd_unit_is_enabled(socket_unit);
    let service_unit_enabled = systemd_unit_is_enabled(service_unit);

    let systemd_summary = format!(
        "socket_active={:?}; socket_enabled={:?}; service_active={:?}; service_enabled={:?}",
        socket_unit_active, socket_unit_enabled, service_unit_active, service_unit_enabled
    );

    let systemd_status = if socket_unit_active == Some(true) || socket_unit_enabled == Some(true) {
        HealthStatus::Ok
    } else {
        HealthStatus::Warn
    };

    let helper_binary_path = std::env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "desconocida".to_string());

    let items = vec![
        HelperHealthItem {
            key: "socket".to_string(),
            status: socket_status,
            summary: socket_summary,
            details: socket_details,
        },
        HelperHealthItem {
            key: "socket_access".to_string(),
            status: access_status,
            summary: access_summary,
            details: Some(format!("ruta={}", DEFAULT_SOCKET_PATH)),
        },
        HelperHealthItem {
            key: "openvpn".to_string(),
            status: openvpn_status,
            summary: openvpn_summary,
            details: openvpn_details,
        },
        HelperHealthItem {
            key: "update_resolv_conf".to_string(),
            status: update_resolv_conf_status,
            summary: update_resolv_conf_summary,
            details: Some(update_resolv_conf_path.to_string()),
        },
        HelperHealthItem {
            key: "systemd".to_string(),
            status: systemd_status,
            summary: "Estado observado de las units de systemd del helper.".to_string(),
            details: Some(systemd_summary),
        },
        HelperHealthItem {
            key: "helper_binary".to_string(),
            status: HealthStatus::Ok,
            summary: "Ruta del binario actual del helper.".to_string(),
            details: Some(helper_binary_path),
        },
    ];

    HelperHealthReport {
        generated_at_unix_ms: crate::ipc::unix_time_ms_now(),
        items,
    }
}

fn systemd_unit_is_active(unit: &str) -> Option<bool> {
    Command::new("systemctl")
        .arg("is-active")
        .arg(unit)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .map(|status| status.success())
}

fn systemd_unit_is_enabled(unit: &str) -> Option<bool> {
    Command::new("systemctl")
        .arg("is-enabled")
        .arg(unit)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .map(|status| status.success())
}

fn spawn_openvpn(
    request: &ConnectRequest,
    shared: Arc<SharedState>,
    manager_tx: Sender<ManagerCommand>,
) -> Result<SessionRuntime> {
    let runtime_dir = TempDir::new().context("no se pudo crear directorio temporal")?;

    let mut vpn_auth_file = NamedTempFile::new_in(runtime_dir.path())
        .context("no se pudo crear fichero temporal de auth VPN")?;
    writeln!(vpn_auth_file, "{}", request.username)?;
    writeln!(vpn_auth_file, "{}", request.password)?;

    let mut proxy_auth_file = None;
    let mut args = vec![
        "--config".to_string(),
        request.config_path.display().to_string(),
        "--auth-user-pass".to_string(),
        vpn_auth_file.path().display().to_string(),
    ];

    if let Some(proxy) = &request.proxy {
        args.push("--http-proxy".into());
        args.push(proxy.host.clone());
        args.push(proxy.port.to_string());

        if let Some(method) = proxy.auth_method.as_openvpn_arg() {
            let mut file = NamedTempFile::new_in(runtime_dir.path())
                .context("no se pudo crear fichero temporal de auth proxy")?;
            writeln!(file, "{}", proxy.username)?;
            writeln!(file, "{}", proxy.password)?;
            args.push(file.path().display().to_string());
            args.push(method.to_string());
            proxy_auth_file = Some(file);
        }
    }

    if request.use_update_resolv_conf {
        args.push("--script-security".into());
        args.push("2".into());
        args.push("--up".into());
        args.push("/etc/openvpn/update-resolv-conf".into());
        args.push("--down".into());
        args.push("/etc/openvpn/update-resolv-conf".into());
    }

    shared.append_log(LogStream::Internal, format!("$ openvpn {}", args.join(" ")));

    let mut child = Command::new("openvpn")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("falló spawn de openvpn")?;

    let pid = child.id();
    let profile_name = request.profile_name.clone();
    let proxy_name = request.proxy.as_ref().map(|p| p.name.clone());
    let started_at_unix_ms = crate::ipc::unix_time_ms_now();

    if let Some(stdout) = child.stdout.take() {
        let shared = Arc::clone(&shared);
        let manager_tx = manager_tx.clone();
        thread::Builder::new()
            .name("vpn-desktopd-openvpn-stdout".into())
            .spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().flatten() {
                    if line.contains("Initialization Sequence Completed") {
                        let _ = manager_tx.send(ManagerCommand::ProcessConnected);
                    }
                    shared.append_log(LogStream::Stdout, line);
                }
            })
            .ok();
    }

    if let Some(stderr) = child.stderr.take() {
        let shared = Arc::clone(&shared);
        thread::Builder::new()
            .name("vpn-desktopd-openvpn-stderr".into())
            .spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    shared.append_log(LogStream::Stderr, line);
                }
            })
            .ok();
    }

    Ok(SessionRuntime {
        child,
        profile_name,
        proxy_name,
        pid,
        started_at_unix_ms,
        _runtime_dir: runtime_dir,
        _vpn_auth_file: vpn_auth_file,
        _proxy_auth_file: proxy_auth_file,
    })
}

fn stop_openvpn(child: &mut Child, pid: u32, shared: Arc<SharedState>) -> Result<()> {
    let term_status = Command::new("/bin/kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();

    match term_status {
        Ok(status) if status.success() => {
            shared.append_log(
                LogStream::Internal,
                format!("SIGTERM enviado a openvpn pid {pid}"),
            );
        }
        Ok(status) => {
            shared.append_log(
                LogStream::Internal,
                format!("kill -TERM devolvió estado no exitoso: {status}"),
            );
        }
        Err(err) => {
            shared.append_log(
                LogStream::Internal,
                format!("no se pudo ejecutar kill -TERM: {err}"),
            );
        }
    }

    for _ in 0..20 {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => thread::sleep(Duration::from_millis(250)),
            Err(err) => return Err(anyhow!("try_wait falló: {err}")),
        }
    }

    shared.append_log(
        LogStream::Internal,
        "OpenVPN no salió tras SIGTERM; forzando kill()",
    );
    child.kill().context("kill() falló")?;
    child.wait().context("wait() tras kill falló")?;
    Ok(())
}
