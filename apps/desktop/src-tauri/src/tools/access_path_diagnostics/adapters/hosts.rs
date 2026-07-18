use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::net::IpAddr;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostsAddressFamily {
    Ipv4,
    Ipv6,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostsSourceKind {
    SystemFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsSource {
    pub kind: HostsSourceKind,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsEntry {
    pub line_number: usize,
    pub address: String,
    pub address_family: HostsAddressFamily,
    pub hostnames: Vec<String>,
    pub matched_hostname: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsComment {
    pub line_number: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostsIssueKind {
    InvalidAddress,
    MissingHostname,
    InvalidHostname,
    InvisibleCharacter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsIssue {
    pub kind: HostsIssueKind,
    pub line_number: usize,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsDiagnostic {
    pub source: HostsSource,
    pub target_hostname: String,
    pub matched_entries: Vec<HostsEntry>,
    pub effective_addresses: Vec<String>,
    pub duplicate_mapping: bool,
    pub multiple_addresses: bool,
    pub mixed_address_families: bool,
    pub commented_entries: Vec<HostsEntry>,
    pub relevant_comments: Vec<HostsComment>,
    pub issues: Vec<HostsIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostsAdapterErrorKind {
    InvalidTarget,
    ReadFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsAdapterError {
    pub kind: HostsAdapterErrorKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

pub fn diagnose_system_hosts(target_hostname: &str) -> Result<HostsDiagnostic, HostsAdapterError> {
    diagnose_hosts_file(crate::tools::hosts::system_hosts_path(), target_hostname)
}

pub fn diagnose_hosts_file(
    path: impl AsRef<Path>,
    target_hostname: &str,
) -> Result<HostsDiagnostic, HostsAdapterError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|error| HostsAdapterError {
        kind: HostsAdapterErrorKind::ReadFailed,
        message: format!("read hosts file failed: {error}"),
        path: Some(path.to_string_lossy().into_owned()),
    })?;

    analyze_hosts_content(path, &content, target_hostname)
}

pub fn analyze_hosts_content(
    source_path: impl AsRef<Path>,
    content: &str,
    target_hostname: &str,
) -> Result<HostsDiagnostic, HostsAdapterError> {
    let target_hostname = normalize_hostname(target_hostname).ok_or_else(|| HostsAdapterError {
        kind: HostsAdapterErrorKind::InvalidTarget,
        message: "hosts target must be a valid hostname".into(),
        path: None,
    })?;
    let source_path = source_path.as_ref();
    let mut matched_entries = Vec::new();
    let mut commented_entries = Vec::new();
    let mut relevant_comments = Vec::new();
    let mut issues = Vec::new();

    for (index, raw_line) in content.lines().enumerate() {
        let line_number = index + 1;
        let line = if line_number == 1 {
            raw_line.strip_prefix('\u{feff}').unwrap_or(raw_line)
        } else {
            raw_line
        };

        collect_invisible_character_issues(line, line_number, &mut issues);

        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            let comment = trimmed.trim_start_matches('#').trim();
            if comment_mentions_target(comment, &target_hostname) {
                relevant_comments.push(HostsComment {
                    line_number,
                    text: comment.to_string(),
                });
            }
            if let Some(entry) = parse_mapping(
                comment,
                line_number,
                &target_hostname,
                None,
                false,
                &mut issues,
            ) {
                commented_entries.push(entry);
            }
            continue;
        }

        let (mapping, inline_comment) = match line.split_once('#') {
            Some((mapping, comment)) => (mapping, non_empty(comment.trim())),
            None => (line, None),
        };
        if let Some(entry) = parse_mapping(
            mapping,
            line_number,
            &target_hostname,
            inline_comment,
            true,
            &mut issues,
        ) {
            matched_entries.push(entry);
        }
    }

    let mut seen_addresses = HashSet::new();
    let effective_addresses: Vec<String> = matched_entries
        .iter()
        .filter_map(|entry| {
            seen_addresses
                .insert(entry.address.clone())
                .then(|| entry.address.clone())
        })
        .collect();
    let has_ipv4 = matched_entries
        .iter()
        .any(|entry| entry.address_family == HostsAddressFamily::Ipv4);
    let has_ipv6 = matched_entries
        .iter()
        .any(|entry| entry.address_family == HostsAddressFamily::Ipv6);

    Ok(HostsDiagnostic {
        source: HostsSource {
            kind: HostsSourceKind::SystemFile,
            path: source_path.to_string_lossy().into_owned(),
        },
        target_hostname,
        duplicate_mapping: matched_entries.len() > 1,
        multiple_addresses: effective_addresses.len() > 1,
        mixed_address_families: has_ipv4 && has_ipv6,
        matched_entries,
        effective_addresses,
        commented_entries,
        relevant_comments,
        issues,
    })
}

fn parse_mapping(
    mapping: &str,
    line_number: usize,
    target_hostname: &str,
    inline_comment: Option<String>,
    report_issues: bool,
    issues: &mut Vec<HostsIssue>,
) -> Option<HostsEntry> {
    let fields: Vec<&str> = mapping.split_whitespace().collect();
    if fields.is_empty() {
        return None;
    }

    let address = match fields[0].parse::<IpAddr>() {
        Ok(address) => address,
        Err(_) => {
            if report_issues {
                issues.push(HostsIssue {
                    kind: HostsIssueKind::InvalidAddress,
                    line_number,
                    message: "hosts mapping starts with an invalid IP address".into(),
                    value: Some(fields[0].to_string()),
                });
            }
            return None;
        }
    };
    if fields.len() == 1 {
        if report_issues {
            issues.push(HostsIssue {
                kind: HostsIssueKind::MissingHostname,
                line_number,
                message: "hosts mapping does not contain a hostname".into(),
                value: Some(fields[0].to_string()),
            });
        }
        return None;
    }

    let mut hostnames = Vec::with_capacity(fields.len() - 1);
    let mut matched_hostname = None;
    for hostname in &fields[1..] {
        match normalize_hostname(hostname) {
            Some(normalized) => {
                if normalized == target_hostname {
                    matched_hostname = Some((*hostname).to_string());
                }
                hostnames.push((*hostname).to_string());
            }
            None if report_issues => issues.push(HostsIssue {
                kind: HostsIssueKind::InvalidHostname,
                line_number,
                message: "hosts mapping contains an invalid hostname".into(),
                value: Some((*hostname).to_string()),
            }),
            None => {}
        }
    }

    Some(HostsEntry {
        line_number,
        address: address.to_string(),
        address_family: match address {
            IpAddr::V4(_) => HostsAddressFamily::Ipv4,
            IpAddr::V6(_) => HostsAddressFamily::Ipv6,
        },
        hostnames,
        matched_hostname: matched_hostname?,
        inline_comment,
    })
}

fn normalize_hostname(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches('.');
    if value.is_empty()
        || value.parse::<IpAddr>().is_ok()
        || value.starts_with('.')
        || value.contains("..")
        || !value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return None;
    }
    Some(value.to_lowercase())
}

fn comment_mentions_target(comment: &str, target_hostname: &str) -> bool {
    comment.split_whitespace().any(|word| {
        let candidate = word.trim_matches(|character: char| {
            !character.is_alphanumeric() && !matches!(character, '-' | '_' | '.')
        });
        normalize_hostname(candidate).as_deref() == Some(target_hostname)
    })
}

fn collect_invisible_character_issues(
    line: &str,
    line_number: usize,
    issues: &mut Vec<HostsIssue>,
) {
    for character in line
        .chars()
        .filter(|character| is_suspicious_invisible(*character))
    {
        issues.push(HostsIssue {
            kind: HostsIssueKind::InvisibleCharacter,
            line_number,
            message: format!(
                "hosts line contains invisible character U+{:04X}",
                character as u32
            ),
            value: Some(format!("U+{:04X}", character as u32)),
        });
    }
}

fn is_suspicious_invisible(character: char) -> bool {
    matches!(
        character,
        '\u{00a0}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2069}'
            | '\u{feff}'
    )
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn source() -> PathBuf {
        PathBuf::from(r"C:\Windows\System32\drivers\etc\hosts")
    }

    #[test]
    fn reports_target_mappings_conflicts_and_relevant_comments() {
        let content = concat!(
            "# 192.0.2.9 API.Example.test disabled mapping\n",
            "192.0.2.1 api.example.test alias.test # primary\n",
            "192.0.2.2 api.example.test\n",
            "2001:db8::1 API.EXAMPLE.TEST. # ipv6\n",
            "# api.example.test is managed manually\n",
        );

        let result = analyze_hosts_content(source(), content, "Api.Example.Test.").unwrap();

        assert_eq!(result.target_hostname, "api.example.test");
        assert_eq!(result.matched_entries.len(), 3);
        assert_eq!(
            result.effective_addresses,
            ["192.0.2.1", "192.0.2.2", "2001:db8::1"]
        );
        assert!(result.duplicate_mapping);
        assert!(result.multiple_addresses);
        assert!(result.mixed_address_families);
        assert_eq!(result.commented_entries.len(), 1);
        assert_eq!(result.relevant_comments.len(), 2);
        assert_eq!(
            result.matched_entries[0].inline_comment.as_deref(),
            Some("primary")
        );
        assert_eq!(result.source.kind, HostsSourceKind::SystemFile);
        assert_eq!(
            serde_json::to_value(&result).unwrap()["source"]["kind"],
            "system_file"
        );
    }

    #[test]
    fn reports_format_errors_and_invisible_characters() {
        let content = concat!(
            "not-an-ip example.test\n",
            "192.0.2.1\n",
            "192.0.2.2 bad!hostname\n",
            "192.0.2.3 example.test\u{200b}\n",
            "\u{feff}192.0.2.4 example.test\n",
        );

        let result = analyze_hosts_content(source(), content, "example.test").unwrap();

        assert!(result
            .issues
            .iter()
            .any(|issue| issue.kind == HostsIssueKind::InvalidAddress && issue.line_number == 1));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.kind == HostsIssueKind::MissingHostname && issue.line_number == 2));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.kind == HostsIssueKind::InvalidHostname && issue.line_number == 3));
        assert_eq!(
            result
                .issues
                .iter()
                .filter(|issue| issue.kind == HostsIssueKind::InvisibleCharacter)
                .count(),
            2
        );
    }

    #[test]
    fn accepts_utf8_bom_only_at_the_start_of_the_file() {
        let result =
            analyze_hosts_content(source(), "\u{feff}192.0.2.1 example.test\n", "example.test")
                .unwrap();

        assert_eq!(result.effective_addresses, ["192.0.2.1"]);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn returns_empty_match_without_inventing_a_failure() {
        let result =
            analyze_hosts_content(source(), "127.0.0.1 localhost\n", "example.test").unwrap();

        assert!(result.matched_entries.is_empty());
        assert!(result.effective_addresses.is_empty());
        assert!(!result.duplicate_mapping);
    }

    #[test]
    fn rejects_ip_address_as_hosts_hostname_target() {
        let error =
            analyze_hosts_content(source(), "127.0.0.1 localhost\n", "127.0.0.1").unwrap_err();

        assert_eq!(error.kind, HostsAdapterErrorKind::InvalidTarget);
    }

    #[test]
    fn reads_a_file_and_preserves_read_errors() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lazycat-hosts-{nonce}.txt"));
        fs::write(&path, "192.0.2.1 example.test\n").unwrap();

        let result = diagnose_hosts_file(&path, "example.test").unwrap();
        assert_eq!(result.effective_addresses, ["192.0.2.1"]);
        fs::remove_file(&path).unwrap();

        let error = diagnose_hosts_file(&path, "example.test").unwrap_err();
        assert_eq!(error.kind, HostsAdapterErrorKind::ReadFailed);
        assert_eq!(error.path.as_deref(), Some(path.to_string_lossy().as_ref()));
        assert!(error.message.contains("read hosts file failed"));
    }

    #[test]
    fn preserves_invalid_utf8_read_failure() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lazycat-hosts-invalid-{nonce}.txt"));
        fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();

        let error = diagnose_hosts_file(&path, "example.test").unwrap_err();
        fs::remove_file(&path).unwrap();

        assert_eq!(error.kind, HostsAdapterErrorKind::ReadFailed);
        assert_eq!(error.path.as_deref(), Some(path.to_string_lossy().as_ref()));
        assert!(error.message.contains("read hosts file failed"));
    }
}
