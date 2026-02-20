use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::process::Command;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortUsageEntry {
    protocol: String,
    local_address: String,
    remote_address: String,
    state: Option<String>,
    pid: u32,
    process_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortProcessSummary {
    pid: u32,
    process_name: String,
    listening_ports: Vec<String>,
    connection_count: usize,
}

fn parse_netstat_entries(raw: &str) -> Vec<PortUsageEntry> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts = trimmed.split_whitespace().collect::<Vec<_>>();
        if parts.is_empty() {
            continue;
        }
        let proto = parts[0].to_ascii_uppercase();
        if proto != "TCP" && proto != "UDP" {
            continue;
        }
        if proto == "TCP" {
            if parts.len() < 5 {
                continue;
            }
            let pid = parts[parts.len() - 1].parse::<u32>().unwrap_or(0);
            let state = parts[parts.len() - 2].to_string();
            let remote = parts[parts.len() - 3].to_string();
            let local = parts[parts.len() - 4].to_string();
            out.push(PortUsageEntry {
                protocol: "TCP".to_string(),
                local_address: local,
                remote_address: remote,
                state: Some(state),
                pid,
                process_name: String::new(),
            });
            continue;
        }
        if parts.len() < 4 {
            continue;
        }
        let pid = parts[parts.len() - 1].parse::<u32>().unwrap_or(0);
        let remote = parts[parts.len() - 2].to_string();
        let local = parts[parts.len() - 3].to_string();
        out.push(PortUsageEntry {
            protocol: "UDP".to_string(),
            local_address: local,
            remote_address: remote,
            state: None,
            pid,
            process_name: String::new(),
        });
    }
    out
}

fn list_process_names() -> HashMap<u32, String> {
    let mut out = HashMap::new();
    let output = match Command::new("tasklist").args(["/FO", "CSV", "/NH"]).output() {
        Ok(v) => v,
        Err(_) => return out,
    };
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());
    for rec in rdr.records().flatten() {
        if rec.len() < 2 {
            continue;
        }
        let name = rec.get(0).unwrap_or("UNKNOWN").trim().to_string();
        let pid = rec
            .get(1)
            .unwrap_or_default()
            .replace(',', "")
            .trim()
            .parse::<u32>()
            .unwrap_or(0);
        if pid > 0 && !name.is_empty() {
            out.insert(pid, name);
        }
    }
    out
}

fn extract_port(local_address: &str) -> Option<String> {
    let port = local_address.rsplit(':').next().unwrap_or_default().trim();
    if port.is_empty() || port == "*" {
        return None;
    }
    if port.chars().all(|c| c.is_ascii_digit()) {
        return Some(port.to_string());
    }
    None
}

fn build_process_summaries(entries: &[PortUsageEntry]) -> Vec<PortProcessSummary> {
    let mut grouped: HashMap<u32, (String, BTreeSet<String>, usize)> = HashMap::new();
    for item in entries {
        let entry = grouped
            .entry(item.pid)
            .or_insert_with(|| (item.process_name.clone(), BTreeSet::new(), 0usize));
        entry.2 += 1;
        let is_listening = item
            .state
            .as_ref()
            .map(|s| s.eq_ignore_ascii_case("LISTENING"))
            .unwrap_or(false)
            || (item.protocol == "UDP" && item.remote_address == "*:*");
        if is_listening {
            if let Some(port) = extract_port(&item.local_address) {
                entry.1.insert(port);
            }
        }
    }
    let mut out = grouped
        .into_iter()
        .map(|(pid, (process_name, listening_ports, connection_count))| PortProcessSummary {
            pid,
            process_name,
            listening_ports: listening_ports.into_iter().collect(),
            connection_count,
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| {
        b.connection_count
            .cmp(&a.connection_count)
            .then_with(|| a.process_name.cmp(&b.process_name))
    });
    out
}

pub fn execute(action: &str, _payload: &Value) -> Result<Value, String> {
    match action {
        "usage" => {
            let output = Command::new("netstat")
                .arg("-ano")
                .output()
                .map_err(|e| format!("netstat failed: {e}"))?;
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            let mut entries = parse_netstat_entries(&text);
            let proc_names = list_process_names();
            for item in &mut entries {
                item.process_name = proc_names
                    .get(&item.pid)
                    .cloned()
                    .unwrap_or_else(|| "UNKNOWN".to_string());
            }
            let mut state_counts: HashMap<String, usize> = HashMap::new();
            let mut tcp_count = 0usize;
            let mut udp_count = 0usize;
            for item in &entries {
                match item.protocol.as_str() {
                    "TCP" => tcp_count += 1,
                    "UDP" => udp_count += 1,
                    _ => {}
                }
                if let Some(state) = &item.state {
                    *state_counts.entry(state.clone()).or_insert(0) += 1;
                }
            }
            let process_summaries = build_process_summaries(&entries);
            Ok(json!({
                "summary": {
                    "total": entries.len(),
                    "tcp": tcp_count,
                    "udp": udp_count
                },
                "stateCounts": state_counts,
                "processSummaries": process_summaries,
                "connections": entries
            }))
        }
        _ => Err(format!("unsupported port action: {action}")),
    }
}
