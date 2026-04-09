use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_SOCKET_PATH: &str = "/run/vpn-desktopd.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectRequest {
    pub profile_name: String,
    pub config_path: PathBuf,
    pub username: String,
    pub password: String,
    pub proxy: Option<ProxyRuntime>,
    pub use_update_resolv_conf: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRuntime {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub auth_method: ProxyAuthMethod,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyAuthMethod {
    None,
    Basic,
    Ntlm,
}

impl ProxyAuthMethod {
    pub fn as_openvpn_arg(self) -> Option<&'static str> {
        match self {
            ProxyAuthMethod::None => None,
            ProxyAuthMethod::Basic => Some("basic"),
            ProxyAuthMethod::Ntlm => Some("ntlm"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VpnPhase {
    Idle,
    Connecting,
    Connected,
    Disconnecting,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    Stdout,
    Stderr,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub seq: u64,
    pub stream: LogStream,
    pub line: String,
    pub ts_unix_ms: u64,
}

impl LogEntry {
    pub fn new(seq: u64, stream: LogStream, line: impl Into<String>) -> Self {
        Self {
            seq,
            stream,
            line: line.into(),
            ts_unix_ms: unix_time_ms_now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub phase: VpnPhase,
    pub active_profile_name: Option<String>,
    pub active_proxy_name: Option<String>,
    pub pid: Option<u32>,
    pub started_at_unix_ms: Option<u64>,
    pub last_error: Option<String>,
    pub last_log_seq: u64,
}

impl SessionSnapshot {
    pub fn idle() -> Self {
        Self {
            phase: VpnPhase::Idle,
            active_profile_name: None,
            active_proxy_name: None,
            pid: None,
            started_at_unix_ms: None,
            last_error: None,
            last_log_seq: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloRequest {
    pub protocol_version: u32,
    pub client_name: String,
    pub client_pid: Option<u32>,
    pub want_logs_tail: usize,
}

impl HelloRequest {
    pub fn new(
        client_name: impl Into<String>,
        client_pid: Option<u32>,
        want_logs_tail: usize,
    ) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            client_name: client_name.into(),
            client_pid,
            want_logs_tail,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloOk {
    pub protocol_version: u32,
    pub snapshot: SessionSnapshot,
    pub logs_tail: Vec<LogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ack {
    pub request: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperHealthItem {
    pub key: String,
    pub status: HealthStatus,
    pub summary: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperHealthReport {
    pub generated_at_unix_ms: u64,
    pub items: Vec<HelperHealthItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidProtocol,
    InvalidRequest,
    Busy,
    Unauthorized,
    NotConnected,
    AlreadyConnected,
    OpenVpnStartFailed,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Hello(HelloRequest),
    GetStatus,
    GetHealth,
    Connect(ConnectRequest),
    Disconnect,
    Ping,
    GetLogs {
        from_seq: Option<u64>,
        limit: Option<usize>,
    },
}

impl ClientMessage {
    pub fn kind(&self) -> &'static str {
        match self {
            ClientMessage::Hello(_) => "hello",
            ClientMessage::GetStatus => "get_status",
            ClientMessage::GetHealth => "get_health",
            ClientMessage::Connect(_) => "connect",
            ClientMessage::Disconnect => "disconnect",
            ClientMessage::Ping => "ping",
            ClientMessage::GetLogs { .. } => "get_logs",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    HelloOk(HelloOk),
    Ack(Ack),
    Error(ErrorMessage),
    Snapshot(SessionSnapshot),
    Health(HelperHealthReport),
    StateChanged(SessionSnapshot),
    Log(LogEntry),
    Logs { entries: Vec<LogEntry> },
    Pong,
}

pub fn unix_time_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
