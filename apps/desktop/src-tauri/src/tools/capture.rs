use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tauri::ipc::Channel;

#[cfg(feature = "capture")]
use pcap::{Capture, Device, Savefile};

#[cfg(feature = "capture")]
use etherparse::SlicedPacket;

/// Maximum raw packet buffer size (100 MB)
#[cfg(feature = "capture")]
const MAX_BUFFER_BYTES: u64 = 100 * 1024 * 1024;

/// Batch send interval in milliseconds
#[cfg(feature = "capture")]
const BATCH_INTERVAL_MS: u64 = 100;

/// Batch send packet count threshold
#[cfg(feature = "capture")]
const BATCH_COUNT: usize = 50;

// ─── Data Structures ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceInfo {
    pub name: String,
    pub description: String,
    pub addresses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacketInfo {
    pub index: u64,
    pub timestamp: f64,
    pub src: String,
    pub dst: String,
    pub protocol: String,
    pub length: u32,
    pub info: String,
    pub raw_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStats {
    pub total_packets: u64,
    pub duration_secs: f64,
    pub bytes_captured: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum CaptureEvent {
    #[serde(rename_all = "camelCase")]
    Packets { items: Vec<PacketInfo> },
    #[serde(rename_all = "camelCase")]
    Error { message: String },
    #[serde(rename_all = "camelCase")]
    Stats {
        total_packets: u64,
        duration_secs: f64,
        bytes_captured: u64,
    },
}

// ─── Session Management ────────────────────────────────────────

struct CaptureSession {
    running: Arc<AtomicBool>,
    start_time: Instant,
    total_packets: Arc<AtomicU64>,
    bytes_captured: Arc<AtomicU64>,
    /// Raw packet data for pcap export: (timestamp_us, raw_bytes)
    #[cfg(feature = "capture")]
    raw_packets: Arc<Mutex<Vec<(i64, Vec<u8>)>>>,
    /// The link type from the capture (needed for pcap export header)
    #[cfg(feature = "capture")]
    link_type: Arc<Mutex<pcap::Linktype>>,
}

static SESSIONS: OnceLock<Mutex<HashMap<String, CaptureSession>>> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<String, CaptureSession>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

// ─── Npcap Detection ───────────────────────────────────────────

/// Check if Npcap (or WinPcap) is installed on Windows.
#[cfg(windows)]
pub fn check_npcap() -> bool {
    // Npcap installs wpcap.dll in System32 or its own directory
    let sys32 = std::env::var("SYSTEMROOT")
        .map(|r| std::path::PathBuf::from(r).join("System32").join("wpcap.dll"))
        .ok();
    let npcap_dir = std::env::var("SYSTEMROOT")
        .map(|r| {
            std::path::PathBuf::from(r)
                .join("System32")
                .join("Npcap")
                .join("wpcap.dll")
        })
        .ok();
    sys32.as_ref().is_some_and(|p| p.exists()) || npcap_dir.as_ref().is_some_and(|p| p.exists())
}

#[cfg(not(windows))]
pub fn check_npcap() -> bool {
    // On non-Windows, libpcap is typically available
    true
}

// ─── Interface Listing ─────────────────────────────────────────

#[cfg(feature = "capture")]
pub fn list_interfaces() -> Result<Vec<InterfaceInfo>, String> {
    let devices = Device::list().map_err(|e| format!("列举网卡失败: {e}"))?;
    let result: Vec<InterfaceInfo> = devices
        .into_iter()
        .map(|d| InterfaceInfo {
            name: d.name.clone(),
            description: d.desc.unwrap_or_default(),
            addresses: d
                .addresses
                .iter()
                .map(|a| a.addr.to_string())
                .collect(),
        })
        .collect();
    Ok(result)
}

#[cfg(not(feature = "capture"))]
pub fn list_interfaces() -> Result<Vec<InterfaceInfo>, String> {
    Err("抓包功能未启用（需要 capture feature）".into())
}

// ─── Packet Parsing ────────────────────────────────────────────

#[cfg(feature = "capture")]
fn parse_packet(data: &[u8], index: u64, elapsed_secs: f64) -> PacketInfo {
    let raw_hex = {
        let limit = data.len().min(256);
        data[..limit]
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    };

    match SlicedPacket::from_ethernet(data) {
        Ok(pkt) => {
            let (src_ip, dst_ip) = match &pkt.ip {
                Some(etherparse::InternetSlice::Ipv4(h, _)) => (
                    format!("{}", h.source_addr()),
                    format!("{}", h.destination_addr()),
                ),
                Some(etherparse::InternetSlice::Ipv6(h, _)) => (
                    format!("{}", h.source_addr()),
                    format!("{}", h.destination_addr()),
                ),
                None => ("--".into(), "--".into()),
            };

            let (protocol, info) = match &pkt.transport {
                Some(etherparse::TransportSlice::Tcp(tcp)) => {
                    let mut flags = Vec::new();
                    if tcp.syn() {
                        flags.push("SYN");
                    }
                    if tcp.ack() {
                        flags.push("ACK");
                    }
                    if tcp.fin() {
                        flags.push("FIN");
                    }
                    if tcp.rst() {
                        flags.push("RST");
                    }
                    if tcp.psh() {
                        flags.push("PSH");
                    }
                    let flag_str = if flags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", flags.join(","))
                    };
                    let sport = tcp.source_port();
                    let dport = tcp.destination_port();
                    let payload_len = pkt.payload.len();

                    // Detect DNS over TCP port 53
                    let proto = if sport == 53 || dport == 53 {
                        "DNS"
                    } else if sport == 80 || dport == 80 {
                        "HTTP"
                    } else if sport == 443 || dport == 443 {
                        "HTTPS"
                    } else {
                        "TCP"
                    };

                    (
                        proto.into(),
                        format!(
                            "{sport} -> {dport}{flag_str} Seq={} Ack={} Len={payload_len}",
                            tcp.sequence_number(),
                            tcp.acknowledgment_number()
                        ),
                    )
                }
                Some(etherparse::TransportSlice::Udp(udp)) => {
                    let sport = udp.source_port();
                    let dport = udp.destination_port();
                    let proto = if sport == 53 || dport == 53 {
                        "DNS"
                    } else if sport == 67 || dport == 67 || sport == 68 || dport == 68 {
                        "DHCP"
                    } else {
                        "UDP"
                    };
                    (
                        proto.into(),
                        format!("{sport} -> {dport} Len={}", udp.length()),
                    )
                }
                Some(etherparse::TransportSlice::Icmpv4(_)) => {
                    ("ICMPv4".into(), "ICMP message".into())
                }
                Some(etherparse::TransportSlice::Icmpv6(_)) => {
                    ("ICMPv6".into(), "ICMPv6 message".into())
                }
                None => {
                    // Check for ARP (EtherType 0x0806) by examining raw bytes
                    if data.len() >= 14 && data[12] == 0x08 && data[13] == 0x06 {
                        ("ARP".into(), "ARP packet".into())
                    } else {
                        ("Other".into(), String::new())
                    }
                }
                _ => ("Other".into(), String::new()),
            };

            PacketInfo {
                index,
                timestamp: elapsed_secs,
                src: src_ip,
                dst: dst_ip,
                protocol,
                length: data.len() as u32,
                info,
                raw_hex,
            }
        }
        Err(_) => {
            // ARP or unparseable packet
            if data.len() >= 14 && data[12] == 0x08 && data[13] == 0x06 {
                PacketInfo {
                    index,
                    timestamp: elapsed_secs,
                    src: "--".into(),
                    dst: "--".into(),
                    protocol: "ARP".into(),
                    length: data.len() as u32,
                    info: "ARP packet".into(),
                    raw_hex,
                }
            } else {
                PacketInfo {
                    index,
                    timestamp: elapsed_secs,
                    src: "--".into(),
                    dst: "--".into(),
                    protocol: "Unknown".into(),
                    length: data.len() as u32,
                    info: String::new(),
                    raw_hex,
                }
            }
        }
    }
}

// ─── Capture Control ───────────────────────────────────────────

#[cfg(feature = "capture")]
pub fn start_capture(
    session_id: String,
    interface: String,
    filter: String,
    on_packet: Channel<CaptureEvent>,
) -> Result<(), String> {
    // Check if session already exists
    {
        let sessions = sessions().lock().map_err(|e| format!("锁定会话失败: {e}"))?;
        if sessions.contains_key(&session_id) {
            return Err("会话已存在，请先停止当前捕获".into());
        }
    }

    // Open capture device
    let mut cap = Capture::from_device(interface.as_str())
        .map_err(|e| format!("打开网卡失败: {e}"))?
        .promisc(true)
        .snaplen(65535)
        .timeout(100) // 100ms read timeout for responsive stopping
        .open()
        .map_err(|e| format!("启动捕获失败: {e}"))?;

    // Apply BPF filter if provided
    if !filter.trim().is_empty() {
        cap.filter(filter.trim(), true)
            .map_err(|e| format!("BPF 过滤器错误: {e}"))?;
    }

    let link_type = cap.get_datalink();

    let running = Arc::new(AtomicBool::new(true));
    let total_packets = Arc::new(AtomicU64::new(0));
    let bytes_captured = Arc::new(AtomicU64::new(0));
    let raw_packets: Arc<Mutex<Vec<(i64, Vec<u8>)>>> = Arc::new(Mutex::new(Vec::new()));

    let session = CaptureSession {
        running: Arc::clone(&running),
        start_time: Instant::now(),
        total_packets: Arc::clone(&total_packets),
        bytes_captured: Arc::clone(&bytes_captured),
        raw_packets: Arc::clone(&raw_packets),
        link_type: Arc::new(Mutex::new(link_type)),
    };

    {
        let mut sessions = sessions().lock().map_err(|e| format!("锁定会话失败: {e}"))?;
        sessions.insert(session_id.clone(), session);
    }

    // Spawn capture thread
    let running_clone = Arc::clone(&running);
    let total_packets_clone = Arc::clone(&total_packets);
    let bytes_captured_clone = Arc::clone(&bytes_captured);
    let raw_packets_clone = Arc::clone(&raw_packets);

    std::thread::spawn(move || {
        let start = Instant::now();
        let mut batch: Vec<PacketInfo> = Vec::with_capacity(BATCH_COUNT);
        let mut last_flush = Instant::now();
        let mut pkt_index: u64 = 0;

        while running_clone.load(Ordering::Relaxed) {
            match cap.next_packet() {
                Ok(packet) => {
                    let elapsed = start.elapsed().as_secs_f64();
                    let data = packet.data;
                    let data_len = data.len() as u64;

                    // Check buffer limit
                    let current_bytes = bytes_captured_clone.load(Ordering::Relaxed);
                    if current_bytes + data_len > MAX_BUFFER_BYTES {
                        let _ = on_packet.send(CaptureEvent::Error {
                            message: format!(
                                "已达到缓冲上限 ({} MB)，请停止捕获或清空数据",
                                MAX_BUFFER_BYTES / 1024 / 1024
                            ),
                        });
                        running_clone.store(false, Ordering::Relaxed);
                        break;
                    }

                    // Store raw packet for pcap export
                    let timestamp_us = (elapsed * 1_000_000.0) as i64;
                    if let Ok(mut rp) = raw_packets_clone.lock() {
                        rp.push((timestamp_us, data.to_vec()));
                    }

                    pkt_index += 1;
                    total_packets_clone.fetch_add(1, Ordering::Relaxed);
                    bytes_captured_clone.fetch_add(data_len, Ordering::Relaxed);

                    let info = parse_packet(data, pkt_index, elapsed);
                    batch.push(info);

                    // Flush batch if threshold reached
                    if batch.len() >= BATCH_COUNT
                        || last_flush.elapsed().as_millis() >= BATCH_INTERVAL_MS as u128
                    {
                        let items = std::mem::take(&mut batch);
                        let _ = on_packet.send(CaptureEvent::Packets { items });
                        last_flush = Instant::now();
                    }
                }
                Err(pcap::Error::TimeoutExpired) => {
                    // Timeout - flush any pending batch
                    if !batch.is_empty()
                        && last_flush.elapsed().as_millis() >= BATCH_INTERVAL_MS as u128
                    {
                        let items = std::mem::take(&mut batch);
                        let _ = on_packet.send(CaptureEvent::Packets { items });
                        last_flush = Instant::now();
                    }
                    continue;
                }
                Err(e) => {
                    let _ = on_packet.send(CaptureEvent::Error {
                        message: format!("捕获错误: {e}"),
                    });
                    break;
                }
            }
        }

        // Flush remaining batch
        if !batch.is_empty() {
            let _ = on_packet.send(CaptureEvent::Packets { items: batch });
        }

        // Send final stats
        let _ = on_packet.send(CaptureEvent::Stats {
            total_packets: total_packets_clone.load(Ordering::Relaxed),
            duration_secs: start.elapsed().as_secs_f64(),
            bytes_captured: bytes_captured_clone.load(Ordering::Relaxed),
        });
    });

    Ok(())
}

#[cfg(not(feature = "capture"))]
pub fn start_capture(
    _session_id: String,
    _interface: String,
    _filter: String,
    _on_packet: Channel<CaptureEvent>,
) -> Result<(), String> {
    Err("抓包功能未启用（需要 capture feature）".into())
}

pub fn stop_capture(session_id: &str) -> Result<CaptureStats, String> {
    let sessions = sessions()
        .lock()
        .map_err(|e| format!("锁定会话失败: {e}"))?;
    let session = sessions
        .get(session_id)
        .ok_or_else(|| "会话不存在".to_string())?;

    // Signal stop
    session.running.store(false, Ordering::Relaxed);

    let stats = CaptureStats {
        total_packets: session.total_packets.load(Ordering::Relaxed),
        duration_secs: session.start_time.elapsed().as_secs_f64(),
        bytes_captured: session.bytes_captured.load(Ordering::Relaxed),
    };

    // Don't remove the session yet - it's needed for pcap export
    // It will be removed when a new capture starts or explicitly cleared
    Ok(stats)
}

pub fn clear_session(session_id: &str) -> Result<(), String> {
    let mut sessions = sessions()
        .lock()
        .map_err(|e| format!("锁定会话失败: {e}"))?;
    sessions.remove(session_id);
    Ok(())
}

// ─── PCAP Export ───────────────────────────────────────────────

#[cfg(feature = "capture")]
pub fn export_pcap(session_id: &str, path: &str) -> Result<(), String> {
    let sessions = sessions()
        .lock()
        .map_err(|e| format!("锁定会话失败: {e}"))?;
    let session = sessions
        .get(session_id)
        .ok_or_else(|| "会话不存在，无法导出".to_string())?;

    let link_type = session
        .link_type
        .lock()
        .map_err(|e| format!("获取链路类型失败: {e}"))?;
    let raw_packets = session
        .raw_packets
        .lock()
        .map_err(|e| format!("获取数据包失败: {e}"))?;

    if raw_packets.is_empty() {
        return Err("没有捕获到数据包".into());
    }

    // Create a dead capture with the correct link type for writing the pcap header
    let dead = Capture::dead(*link_type).map_err(|e| format!("创建 pcap 写入器失败: {e}"))?;
    let mut savefile: Savefile = dead
        .savefile(path)
        .map_err(|e| format!("创建 pcap 文件失败: {e}"))?;

    for (timestamp_us, data) in raw_packets.iter() {
        let ts_sec = *timestamp_us / 1_000_000;
        let ts_usec = *timestamp_us % 1_000_000;
        let header = pcap::PacketHeader {
            ts: libc::timeval {
                tv_sec: ts_sec as libc::time_t,
                tv_usec: ts_usec as libc::suseconds_t,
            },
            caplen: data.len() as u32,
            len: data.len() as u32,
        };
        savefile.write(&pcap::Packet {
            header: &header,
            data,
        });
    }

    savefile.flush().map_err(|e| format!("写入 pcap 文件失败: {e}"))?;

    Ok(())
}

#[cfg(not(feature = "capture"))]
pub fn export_pcap(_session_id: &str, _path: &str) -> Result<(), String> {
    Err("抓包功能未启用（需要 capture feature）".into())
}
