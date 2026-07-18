use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessProtocol {
    Http,
    Https,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessPathTargetKind {
    Hostname,
    Ipv4,
    Ipv6,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedAccessPathTarget {
    pub raw_input: String,
    pub protocol: AccessProtocol,
    pub hostname: String,
    pub target_kind: AccessPathTargetKind,
    pub port: u16,
    pub path: String,
    pub url: String,
    pub sni: Option<String>,
    #[serde(default)]
    pub verify_hostname: Option<String>,
    pub http_host: String,
    pub connection_ip: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepId {
    Proxy,
    Hosts,
    Dns,
    Tcp,
    Tls,
    Http,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepLifecycle {
    Pending,
    Running,
    Completed,
    Blocked,
    Skipped,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepOutcome {
    Success,
    Warning,
    Failed,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    #[serde(default)]
    pub retriable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub id: String,
    pub step_id: StepId,
    pub kind: String,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conclusion {
    pub id: String,
    pub severity: ConclusionSeverity,
    pub message: String,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    #[serde(default)]
    pub recommendation_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConclusionSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub id: String,
    pub title: String,
    pub action: String,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticStep {
    pub id: StepId,
    pub lifecycle: StepLifecycle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<StepOutcome>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<DiagnosticError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisReport {
    pub schema_version: u32,
    pub report_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub input: NormalizedAccessPathTarget,
    #[serde(default)]
    pub steps: Vec<DiagnosticStep>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub conclusions: Vec<Conclusion>,
    #[serde(default)]
    pub recommendations: Vec<Recommendation>,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn target() -> NormalizedAccessPathTarget {
        NormalizedAccessPathTarget {
            raw_input: "https://[2001:db8::1]:8443/health?x=1".into(),
            protocol: AccessProtocol::Https,
            hostname: "2001:db8::1".into(),
            target_kind: AccessPathTargetKind::Ipv6,
            port: 8443,
            path: "/health?x=1".into(),
            url: "https://[2001:db8::1]:8443/health?x=1".into(),
            sni: Some("api.example.test".into()),
            verify_hostname: Some("api.example.test".into()),
            http_host: "api.example.test".into(),
            connection_ip: Some("2001:db8::1".into()),
        }
    }

    #[test]
    fn serializes_wire_names_and_enum_values() {
        let report = DiagnosisReport {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: "report-1".into(),
            run_id: Some("run-1".into()),
            input: target(),
            steps: vec![DiagnosticStep {
                id: StepId::Tls,
                lifecycle: StepLifecycle::Completed,
                outcome: Some(StepOutcome::Unverified),
                evidence_ids: vec!["ev-1".into()],
                error: Some(DiagnosticError {
                    code: "certificate_unverified".into(),
                    message: "无法在线确认吊销状态".into(),
                    details: Some(json!({ "source": "offline" })),
                    retriable: false,
                }),
                started_at: None,
                finished_at: None,
            }],
            evidence: vec![Evidence {
                id: "ev-1".into(),
                step_id: StepId::Tls,
                kind: "certificate_chain".into(),
                value: json!({ "connectionIp": "2001:db8::1" }),
                observed_at: None,
            }],
            conclusions: vec![Conclusion {
                id: "c-1".into(),
                severity: ConclusionSeverity::Warning,
                message: "证书信任状态无法验证".into(),
                evidence_ids: vec!["ev-1".into()],
                recommendation_ids: vec!["r-1".into()],
            }],
            recommendations: vec![Recommendation {
                id: "r-1".into(),
                title: "检查本机证书链".into(),
                action: "确认离线根证书和吊销策略".into(),
                evidence_ids: vec!["ev-1".into()],
            }],
            started_at: "2026-07-18T00:00:00Z".into(),
            finished_at: None,
        };

        let value = serde_json::to_value(&report).expect("serialize report");
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["input"]["targetKind"], "ipv6");
        assert_eq!(value["input"]["httpHost"], "api.example.test");
        assert_eq!(value["steps"][0]["lifecycle"], "completed");
        assert_eq!(value["steps"][0]["outcome"], "unverified");
        assert_eq!(value["steps"][0]["error"]["retriable"], false);
        assert_eq!(value["conclusions"][0]["recommendationIds"][0], "r-1");
    }

    #[test]
    fn round_trip_preserves_optional_target_fields() {
        let input = json!({
            "rawInput": "example.test",
            "protocol": "http",
            "hostname": "example.test",
            "targetKind": "hostname",
            "port": 80,
            "path": "",
            "url": "http://example.test",
            "sni": null,
            "verifyHostname": "example.test",
            "httpHost": "example.test",
            "connectionIp": null
        });
        let parsed: NormalizedAccessPathTarget =
            serde_json::from_value(input).expect("deserialize target");
        assert_eq!(parsed.target_kind, AccessPathTargetKind::Hostname);
        assert_eq!(parsed.port, 80);
        assert_eq!(parsed.verify_hostname.as_deref(), Some("example.test"));
        assert_eq!(
            serde_json::to_value(parsed).unwrap()["hostname"],
            "example.test"
        );
    }

    #[test]
    fn accepts_legacy_target_without_verify_hostname() {
        let input = json!({
            "rawInput": "example.test",
            "protocol": "https",
            "hostname": "example.test",
            "targetKind": "hostname",
            "port": 443,
            "path": "/",
            "url": "https://example.test/",
            "sni": "routing.example.test",
            "httpHost": "example.test",
            "connectionIp": null
        });
        let parsed: NormalizedAccessPathTarget =
            serde_json::from_value(input).expect("deserialize legacy target");
        assert!(parsed.verify_hostname.is_none());
    }
}
