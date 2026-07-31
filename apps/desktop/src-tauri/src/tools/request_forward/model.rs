use serde::{Deserialize, Serialize};

pub(crate) const REQUEST_FORWARD_ERROR_MARKER: &str = "lazycat.request_forward.error";
pub(crate) const REQUEST_FORWARD_ERROR_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RequestForwardErrorCode {
    ListenerInUse,
    DnsFailed,
    TargetUnreachable,
    TlsFailed,
    SelfForward,
    InvalidConfig,
    LifecycleConflict,
    PersistenceFailed,
    Unknown,
}

impl RequestForwardErrorCode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ListenerInUse => "listener_in_use",
            Self::DnsFailed => "dns_failed",
            Self::TargetUnreachable => "target_unreachable",
            Self::TlsFailed => "tls_failed",
            Self::SelfForward => "self_forward",
            Self::InvalidConfig => "invalid_config",
            Self::LifecycleConflict => "lifecycle_conflict",
            Self::PersistenceFailed => "persistence_failed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Serialize)]
struct RequestForwardErrorEnvelope<'a> {
    marker: &'static str,
    version: u8,
    code: RequestForwardErrorCode,
    message: &'a str,
    state: &'a str,
}

#[derive(Deserialize)]
struct DecodedRequestForwardErrorEnvelope {
    marker: String,
    version: u8,
    code: RequestForwardErrorCode,
    message: String,
    #[serde(rename = "state")]
    _state: String,
}

pub(crate) struct RequestForwardActionError {
    pub(crate) result_code: String,
    pub(crate) message: String,
}

pub(crate) fn encode_request_forward_error(message: &str, state: &str) -> String {
    encode_request_forward_error_with_code(message, state, classify_request_forward_error(message))
}

pub(crate) fn encode_request_forward_error_with_code(
    message: &str,
    state: &str,
    code: RequestForwardErrorCode,
) -> String {
    let envelope = RequestForwardErrorEnvelope {
        marker: REQUEST_FORWARD_ERROR_MARKER,
        version: REQUEST_FORWARD_ERROR_VERSION,
        code,
        message,
        state,
    };
    serde_json::to_string(&envelope).unwrap_or_else(|_| message.to_string())
}

pub(crate) fn decode_request_forward_error(encoded: &str) -> Option<RequestForwardActionError> {
    let envelope: DecodedRequestForwardErrorEnvelope = serde_json::from_str(encoded).ok()?;
    if envelope.marker != REQUEST_FORWARD_ERROR_MARKER
        || envelope.version != REQUEST_FORWARD_ERROR_VERSION
    {
        return None;
    }
    Some(RequestForwardActionError {
        result_code: envelope.code.as_str().to_string(),
        message: envelope.message,
    })
}

pub(crate) fn classify_request_forward_error(message: &str) -> RequestForwardErrorCode {
    let normalized = message.to_lowercase();

    if normalized.contains("不能直接转发到自身") || normalized.contains("self-forward") {
        return RequestForwardErrorCode::SelfForward;
    }
    if (normalized.contains("监听绑定失败") || normalized.contains("listener bind failed"))
        && (normalized.contains("address already in use")
            || normalized.contains("addrinuse")
            || normalized.contains("10048")
            || normalized.contains("only one usage of each socket address"))
    {
        return RequestForwardErrorCode::ListenerInUse;
    }
    if normalized.contains("解析目标地址")
        || normalized.contains("解析下游")
        || normalized.contains("读取系统 dns 配置失败")
        || normalized.contains("创建 dns 预检 runtime 失败")
        || normalized.contains("无法启动目标地址解析线程")
        || normalized.contains("未解析到可尝试的目标地址")
        || normalized.contains("未解析到地址")
    {
        return RequestForwardErrorCode::DnsFailed;
    }
    if normalized.contains("tls handshake")
        || normalized.contains("tls 握手")
        || normalized.contains("tls 根证书")
        || normalized.contains("tls 主机名")
        || normalized.contains("tls socket")
        || normalized.contains("tls 预检 runtime")
        || normalized.contains("invalid peer")
        || normalized.contains("peer certificate")
        || normalized.contains("certificate not valid")
        || normalized.contains("certificate verify")
        || normalized.contains("not valid for name")
    {
        return RequestForwardErrorCode::TlsFailed;
    }
    if normalized.contains("dns")
        || normalized.contains("name or service not known")
        || normalized.contains("no such host")
        || normalized.contains("nodename nor servname")
        || normalized.contains("temporary failure in name resolution")
    {
        return RequestForwardErrorCode::DnsFailed;
    }
    if normalized.contains("连接下游")
        || normalized.contains("target unreachable")
        || normalized.contains("connection refused")
        || normalized.contains("network is unreachable")
        || normalized.contains("host is unreachable")
        || normalized.contains("connect timed out")
        || normalized.contains("connection timed out")
    {
        return RequestForwardErrorCode::TargetUnreachable;
    }
    if normalized.contains("应用正在退出")
        || normalized.contains("不能修改或删除")
        || normalized.contains("starting")
        || normalized.contains("stopping")
        || normalized.contains("lifecycle")
    {
        return RequestForwardErrorCode::LifecycleConflict;
    }
    if normalized.contains("database")
        || normalized.contains("数据库")
        || normalized.contains("persist")
        || normalized.contains("持久化")
        || normalized.contains("保存停止意图失败")
        || normalized.contains("查询转发")
        || normalized.contains("读取转发")
        || normalized.contains("创建转发规则失败")
        || normalized.contains("更新转发规则失败")
        || normalized.contains("删除转发规则失败")
        || normalized.contains("启动意图失败")
        || normalized.contains("重置转发统计失败")
        || normalized.contains("清空转发日志失败")
        || normalized.contains("提交转发")
    {
        return RequestForwardErrorCode::PersistenceFailed;
    }
    if normalized.contains("参数无效")
        || normalized.contains("配置")
        || normalized.contains("格式不正确")
        || normalized.contains("目标 url 不能包含 query 或 fragment")
        || normalized.contains("已保存规则不能修改协议")
        || normalized.contains("必须")
        || normalized.contains("缺少")
        || normalized.contains("不能为空")
        || normalized.contains("仅支持")
    {
        return RequestForwardErrorCode::InvalidConfig;
    }
    RequestForwardErrorCode::Unknown
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForwardProtocol {
    Http,
    Tcp,
    Udp,
}

impl ForwardProtocol {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }

    pub(crate) fn from_db(value: &str) -> Option<Self> {
        match value {
            "http" => Some(Self::Http),
            "tcp" => Some(Self::Tcp),
            "udp" => Some(Self::Udp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleWriteInput {
    pub name: String,
    pub protocol: ForwardProtocol,
    pub bind_host: String,
    pub listen_port: u16,
    pub target_url: Option<String>,
    pub target_host: Option<String>,
    pub target_port: Option<u16>,
    pub capture_http_headers: bool,
    pub capture_http_body: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForwardRule {
    pub id: i64,
    pub name: String,
    pub protocol: ForwardProtocol,
    pub bind_host: String,
    pub listen_port: u16,
    pub target_url: Option<String>,
    pub target_host: Option<String>,
    pub target_port: Option<u16>,
    pub capture_http_headers: bool,
    pub capture_http_body: bool,
    pub auto_start: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl ForwardRule {
    pub(crate) fn write_input(&self) -> RuleWriteInput {
        RuleWriteInput {
            name: self.name.clone(),
            protocol: self.protocol,
            bind_host: self.bind_host.clone(),
            listen_port: self.listen_port,
            target_url: self.target_url.clone(),
            target_host: self.target_host.clone(),
            target_port: self.target_port,
            capture_http_headers: self.capture_http_headers,
            capture_http_body: self.capture_http_body,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ValidatedRuleWriteInput {
    pub name: String,
    pub protocol: ForwardProtocol,
    pub bind_host: String,
    pub listen_port: u16,
    pub target_url: Option<String>,
    pub target_host: Option<String>,
    pub target_port: Option<u16>,
    pub capture_http_headers: bool,
    pub capture_http_body: bool,
}
