use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Deserialize)]
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
