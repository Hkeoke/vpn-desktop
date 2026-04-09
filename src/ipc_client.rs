use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};

use crate::ipc::{
    Ack, ClientMessage, ConnectRequest, ErrorMessage, HelloOk, HelloRequest, HelperHealthReport,
    LogEntry, ServerMessage, SessionSnapshot, VpnPhase, DEFAULT_SOCKET_PATH,
};
use crate::vpn::VpnStatus;

#[derive(Debug, Clone)]
pub enum GuiCommand {
    Connect(ConnectRequest),
    Disconnect,
    RequestStatus,
    RequestHealth,
    Ping,
    RequestLogs {
        from_seq: Option<u64>,
        limit: Option<usize>,
    },
}

#[derive(Debug, Clone)]
pub enum GuiEvent {
    ConnectedToHelper,
    HelperUnavailable(String),
    Snapshot(SessionSnapshot),
    StatusChanged(VpnStatus),
    Log(String),
    Error(String),
    Pong,
    Health(HelperHealthReport),
    Ack(String),
}

pub struct IpcClient {
    cmd_tx: Sender<GuiCommand>,
    event_rx: Receiver<GuiEvent>,
}

impl IpcClient {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = unbounded::<GuiCommand>();
        let (event_tx, event_rx) = unbounded::<GuiEvent>();

        std::thread::Builder::new()
            .name("ipc-client".into())
            .spawn(move || worker_loop(cmd_rx, event_tx))
            .expect("No se pudo crear el hilo del cliente IPC");

        Self { cmd_tx, event_rx }
    }

    pub fn connect_vpn(&self, request: ConnectRequest) {
        let _ = self.cmd_tx.send(GuiCommand::Connect(request));
    }

    pub fn disconnect_vpn(&self) {
        let _ = self.cmd_tx.send(GuiCommand::Disconnect);
    }

    pub fn request_status(&self) {
        let _ = self.cmd_tx.send(GuiCommand::RequestStatus);
    }

    pub fn request_health(&self) {
        let _ = self.cmd_tx.send(GuiCommand::RequestHealth);
    }

    pub fn ping(&self) {
        let _ = self.cmd_tx.send(GuiCommand::Ping);
    }

    pub fn request_logs(&self, from_seq: Option<u64>, limit: Option<usize>) {
        let _ = self
            .cmd_tx
            .send(GuiCommand::RequestLogs { from_seq, limit });
    }

    pub fn poll(&self) -> Vec<GuiEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }
}

enum ConnectionState {
    Disconnected,
    Connected(ActiveConnection),
}

struct ActiveConnection {
    writer: Arc<Mutex<UnixStream>>,
}

fn worker_loop(cmd_rx: Receiver<GuiCommand>, event_tx: Sender<GuiEvent>) {
    let mut state = ConnectionState::Disconnected;

    loop {
        match &mut state {
            ConnectionState::Disconnected => match establish_connection(&event_tx) {
                Ok(active) => {
                    state = ConnectionState::Connected(active);
                }
                Err(msg) => {
                    let _ = event_tx.send(GuiEvent::HelperUnavailable(msg));

                    match cmd_rx.recv_timeout(Duration::from_secs(2)) {
                        Ok(GuiCommand::Connect(_)) | Ok(GuiCommand::RequestStatus) => {
                            continue;
                        }
                        Ok(_) => {}
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            },
            ConnectionState::Connected(active) => {
                match cmd_rx.recv_timeout(Duration::from_millis(150)) {
                    Ok(cmd) => {
                        if let Err(err) = send_command(&active.writer, cmd) {
                            let _ = event_tx.send(GuiEvent::HelperUnavailable(format!(
                                "Se perdió la conexión con el helper: {}",
                                err
                            )));
                            state = ConnectionState::Disconnected;
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        }
    }
}

fn establish_connection(event_tx: &Sender<GuiEvent>) -> Result<ActiveConnection, String> {
    let stream = UnixStream::connect(DEFAULT_SOCKET_PATH).map_err(map_connect_error)?;
    let reader_stream = stream
        .try_clone()
        .map_err(|e| format!("No se pudo clonar el socket Unix: {}", e))?;
    let writer = Arc::new(Mutex::new(stream));

    send_raw_message(
        &writer,
        &ClientMessage::Hello(HelloRequest::new(
            "vpn-desktop",
            Some(std::process::id()),
            200,
        )),
    )
    .map_err(|e| format!("No se pudo enviar hello al helper: {}", e))?;

    let tx = event_tx.clone();
    std::thread::Builder::new()
        .name("ipc-reader".into())
        .spawn(move || reader_loop(reader_stream, tx))
        .map_err(|e| format!("No se pudo crear el hilo lector IPC: {}", e))?;

    let _ = event_tx.send(GuiEvent::ConnectedToHelper);

    Ok(ActiveConnection { writer })
}

fn send_command(writer: &Arc<Mutex<UnixStream>>, cmd: GuiCommand) -> Result<(), String> {
    let msg = match cmd {
        GuiCommand::Connect(request) => ClientMessage::Connect(request),
        GuiCommand::Disconnect => ClientMessage::Disconnect,
        GuiCommand::RequestStatus => ClientMessage::GetStatus,
        GuiCommand::RequestHealth => ClientMessage::GetHealth,
        GuiCommand::Ping => ClientMessage::Ping,
        GuiCommand::RequestLogs { from_seq, limit } => ClientMessage::GetLogs { from_seq, limit },
    };

    send_raw_message(writer, &msg)
}

fn send_raw_message(writer: &Arc<Mutex<UnixStream>>, msg: &ClientMessage) -> Result<(), String> {
    let payload = serde_json::to_string(msg)
        .map_err(|e| format!("No se pudo serializar mensaje IPC: {}", e))?;

    let mut stream = writer
        .lock()
        .map_err(|_| "No se pudo obtener el lock del socket IPC".to_string())?;

    stream
        .write_all(payload.as_bytes())
        .map_err(|e| format!("No se pudo escribir en el socket IPC: {}", e))?;
    stream
        .write_all(b"\n")
        .map_err(|e| format!("No se pudo terminar el mensaje IPC: {}", e))?;
    stream
        .flush()
        .map_err(|e| format!("No se pudo enviar el mensaje IPC: {}", e))?;

    Ok(())
}

fn reader_loop(stream: UnixStream, event_tx: Sender<GuiEvent>) {
    let reader = BufReader::new(stream);

    for line in reader.lines() {
        match line {
            Ok(raw) => {
                let raw = raw.trim();
                if raw.is_empty() {
                    continue;
                }

                match serde_json::from_str::<ServerMessage>(raw) {
                    Ok(message) => handle_server_message(message, &event_tx),
                    Err(e) => {
                        let _ = event_tx.send(GuiEvent::Error(format!(
                            "Mensaje inválido del helper: {}",
                            e
                        )));
                    }
                }
            }
            Err(e) => {
                let _ = event_tx.send(GuiEvent::HelperUnavailable(format!(
                    "Se cerró la conexión con el helper: {}",
                    e
                )));
                break;
            }
        }
    }
}

fn handle_server_message(msg: ServerMessage, event_tx: &Sender<GuiEvent>) {
    match msg {
        ServerMessage::HelloOk(HelloOk {
            snapshot,
            logs_tail,
            ..
        }) => {
            emit_snapshot(snapshot, event_tx);
            for entry in logs_tail {
                let _ = event_tx.send(GuiEvent::Log(format_log_entry(&entry)));
            }
        }
        ServerMessage::Ack(Ack { request, message }) => {
            if let Some(text) = message {
                let _ = event_tx.send(GuiEvent::Ack(text));
            } else {
                let _ = event_tx.send(GuiEvent::Ack(format!(
                    "Operación '{}' aceptada por el helper",
                    request
                )));
            }
        }
        ServerMessage::Error(ErrorMessage { message, .. }) => {
            let _ = event_tx.send(GuiEvent::Error(message.clone()));
            let _ = event_tx.send(GuiEvent::StatusChanged(VpnStatus::Failed(message)));
        }
        ServerMessage::Snapshot(snapshot) => {
            emit_snapshot(snapshot, event_tx);
        }
        ServerMessage::StateChanged(snapshot) => {
            emit_snapshot(snapshot, event_tx);
        }
        ServerMessage::Log(entry) => {
            let _ = event_tx.send(GuiEvent::Log(format_log_entry(&entry)));
        }
        ServerMessage::Logs { entries } => {
            for entry in entries {
                let _ = event_tx.send(GuiEvent::Log(format_log_entry(&entry)));
            }
        }
        ServerMessage::Pong => {
            let _ = event_tx.send(GuiEvent::Pong);
        }
        ServerMessage::Health(health) => {
            let _ = event_tx.send(GuiEvent::Health(health));
        }
    }
}

fn emit_snapshot(snapshot: SessionSnapshot, event_tx: &Sender<GuiEvent>) {
    let status = map_phase_to_status(&snapshot.phase, snapshot.last_error.as_deref());
    let _ = event_tx.send(GuiEvent::Snapshot(snapshot));
    let _ = event_tx.send(GuiEvent::StatusChanged(status));
}

fn map_phase_to_status(phase: &VpnPhase, last_error: Option<&str>) -> VpnStatus {
    match phase {
        VpnPhase::Idle => VpnStatus::Disconnected,
        VpnPhase::Connecting => VpnStatus::Connecting,
        VpnPhase::Connected => VpnStatus::Connected,
        VpnPhase::Disconnecting => VpnStatus::Connecting,
        VpnPhase::Failed => {
            VpnStatus::Failed(last_error.unwrap_or("Error desconocido").to_string())
        }
    }
}

fn format_log_entry(entry: &LogEntry) -> String {
    match entry.stream {
        crate::ipc::LogStream::Stdout => entry.line.clone(),
        crate::ipc::LogStream::Stderr => format!("[stderr] {}", entry.line),
        crate::ipc::LogStream::Internal => format!("[helper] {}", entry.line),
    }
}

fn map_connect_error(err: std::io::Error) -> String {
    match err.kind() {
        ErrorKind::NotFound => format!(
            "No se encontró el socket del helper en '{}'. Asegúrate de que el servicio root está instalado y activo.",
            DEFAULT_SOCKET_PATH
        ),
        ErrorKind::PermissionDenied => format!(
            "No tienes permisos para usar el helper en '{}'. Revisa el grupo/permisos del socket.",
            DEFAULT_SOCKET_PATH
        ),
        _ => format!("No se pudo conectar con el helper: {}", err),
    }
}
