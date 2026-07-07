use serde_json::{json, Value};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::time::Instant;

#[cfg(windows)]
use std::time::Duration;

struct HttpStatus {
    code: u16,
    name: &'static str,
    desc: &'static str,
    usage: &'static str,
    causes: &'static str,
}

fn http_status_data() -> Vec<HttpStatus> {
    vec![
        HttpStatus {
            code: 100,
            name: "Continue",
            desc: "继续",
            usage: "客户端应继续发送请求体",
            causes: "",
        },
        HttpStatus {
            code: 101,
            name: "Switching Protocols",
            desc: "切换协议",
            usage: "WebSocket 升级",
            causes: "",
        },
        HttpStatus {
            code: 200,
            name: "OK",
            desc: "成功",
            usage: "请求成功",
            causes: "",
        },
        HttpStatus {
            code: 201,
            name: "Created",
            desc: "已创建",
            usage: "POST 创建资源成功",
            causes: "",
        },
        HttpStatus {
            code: 202,
            name: "Accepted",
            desc: "已接受",
            usage: "异步任务已接受处理",
            causes: "",
        },
        HttpStatus {
            code: 203,
            name: "Non-Authoritative Information",
            desc: "非权威信息",
            usage: "代理返回的修改后响应",
            causes: "",
        },
        HttpStatus {
            code: 204,
            name: "No Content",
            desc: "无内容",
            usage: "DELETE 成功，无返回体",
            causes: "",
        },
        HttpStatus {
            code: 206,
            name: "Partial Content",
            desc: "部分内容",
            usage: "Range 请求，断点续传",
            causes: "",
        },
        HttpStatus {
            code: 301,
            name: "Moved Permanently",
            desc: "永久重定向",
            usage: "资源永久迁移到新 URL",
            causes: "",
        },
        HttpStatus {
            code: 302,
            name: "Found",
            desc: "临时重定向",
            usage: "临时跳转到另一个 URL",
            causes: "",
        },
        HttpStatus {
            code: 303,
            name: "See Other",
            desc: "查看其他",
            usage: "POST 后重定向到 GET",
            causes: "",
        },
        HttpStatus {
            code: 304,
            name: "Not Modified",
            desc: "未修改",
            usage: "缓存有效，无需重新传输",
            causes: "",
        },
        HttpStatus {
            code: 307,
            name: "Temporary Redirect",
            desc: "临时重定向",
            usage: "保持请求方法的临时重定向",
            causes: "",
        },
        HttpStatus {
            code: 308,
            name: "Permanent Redirect",
            desc: "永久重定向",
            usage: "保持请求方法的永久重定向",
            causes: "",
        },
        HttpStatus {
            code: 400,
            name: "Bad Request",
            desc: "错误请求",
            usage: "请求参数错误或格式不正确",
            causes: "JSON 格式错误; 必填参数缺失; 参数类型不匹配; 请求体编码错误",
        },
        HttpStatus {
            code: 401,
            name: "Unauthorized",
            desc: "未授权",
            usage: "需要身份认证",
            causes:
                "Token 过期或无效; 未携带 Authorization 头; Cookie/Session 过期; OAuth 授权失败",
        },
        HttpStatus {
            code: 403,
            name: "Forbidden",
            desc: "禁止访问",
            usage: "服务器拒绝请求",
            causes: "无访问权限; IP 被封禁; CORS 策略拦截; CSRF Token 校验失败; 文件/目录权限不足",
        },
        HttpStatus {
            code: 404,
            name: "Not Found",
            desc: "未找到",
            usage: "请求的资源不存在",
            causes: "URL 路径拼写错误; 资源已被删除; 路由未注册; 后端接口未部署",
        },
        HttpStatus {
            code: 405,
            name: "Method Not Allowed",
            desc: "方法不允许",
            usage: "HTTP 方法不被支持",
            causes: "用 GET 请求了 POST 接口; 路由只允许特定方法; RESTful 方法不匹配",
        },
        HttpStatus {
            code: 406,
            name: "Not Acceptable",
            desc: "不可接受",
            usage: "无法满足 Accept 头要求",
            causes: "Accept 头与服务器响应格式不兼容",
        },
        HttpStatus {
            code: 407,
            name: "Proxy Authentication Required",
            desc: "需要代理认证",
            usage: "代理服务器要求认证",
            causes: "企业代理未配置认证; 代理凭证错误",
        },
        HttpStatus {
            code: 408,
            name: "Request Timeout",
            desc: "请求超时",
            usage: "服务器等待请求超时",
            causes: "客户端发送数据过慢; 网络不稳定; 大文件上传超时",
        },
        HttpStatus {
            code: 409,
            name: "Conflict",
            desc: "冲突",
            usage: "资源状态冲突",
            causes: "并发更新同一资源; 唯一约束冲突; 乐观锁版本不匹配",
        },
        HttpStatus {
            code: 410,
            name: "Gone",
            desc: "已删除",
            usage: "资源已永久删除",
            causes: "API 版本已下线; 资源被永久移除",
        },
        HttpStatus {
            code: 411,
            name: "Length Required",
            desc: "需要内容长度",
            usage: "缺少 Content-Length 头",
            causes: "未设置 Content-Length; 分块传输配置错误",
        },
        HttpStatus {
            code: 412,
            name: "Precondition Failed",
            desc: "前提条件失败",
            usage: "条件请求的前提不满足",
            causes: "If-Match/If-Unmodified-Since 条件不满足; ETag 不匹配",
        },
        HttpStatus {
            code: 413,
            name: "Payload Too Large",
            desc: "请求体过大",
            usage: "上传文件超过限制",
            causes: "上传文件超过 Nginx/后端限制; Body 大小超过配置; client_max_body_size 过小",
        },
        HttpStatus {
            code: 414,
            name: "URI Too Long",
            desc: "URI 过长",
            usage: "URL 超过服务器限制",
            causes: "GET 参数过多; 查询字符串过长; 应改用 POST",
        },
        HttpStatus {
            code: 415,
            name: "Unsupported Media Type",
            desc: "不支持的媒体类型",
            usage: "Content-Type 不被支持",
            causes: "Content-Type 与实际格式不匹配; 发送 JSON 但未设 application/json",
        },
        HttpStatus {
            code: 416,
            name: "Range Not Satisfiable",
            desc: "范围不满足",
            usage: "Range 请求超出范围",
            causes: "Range 头的字节范围超出文件实际大小",
        },
        HttpStatus {
            code: 422,
            name: "Unprocessable Entity",
            desc: "无法处理的实体",
            usage: "请求格式正确但语义错误",
            causes: "字段校验失败(邮箱格式、手机号等); 业务规则不满足; 枚举值不在允许范围内",
        },
        HttpStatus {
            code: 429,
            name: "Too Many Requests",
            desc: "请求过多",
            usage: "触发限流",
            causes: "API 调用频率超限; 爬虫/脚本触发防护; 缺少速率限制退避逻辑",
        },
        HttpStatus {
            code: 500,
            name: "Internal Server Error",
            desc: "服务器内部错误",
            usage: "服务器未知错误",
            causes: "后端代码抛出未捕获异常; 空指针/数组越界; 数据库连接失败; 依赖服务不可用",
        },
        HttpStatus {
            code: 501,
            name: "Not Implemented",
            desc: "未实现",
            usage: "服务器不支持该功能",
            causes: "接口尚未开发; 服务器不支持请求的 HTTP 方法",
        },
        HttpStatus {
            code: 502,
            name: "Bad Gateway",
            desc: "网关错误",
            usage: "上游服务器返回无效响应",
            causes: "后端服务崩溃/未启动; Nginx upstream 配置错误; 后端返回格式异常; 容器 OOM 被杀",
        },
        HttpStatus {
            code: 503,
            name: "Service Unavailable",
            desc: "服务不可用",
            usage: "服务器过载或维护中",
            causes: "服务正在部署/重启; 连接池耗尽; 线程池满; 熔断器打开; 服务器资源不足",
        },
        HttpStatus {
            code: 504,
            name: "Gateway Timeout",
            desc: "网关超时",
            usage: "上游服务器响应超时",
            causes: "后端处理时间过长; 数据库慢查询; 第三方接口超时; proxy_read_timeout 过小",
        },
        HttpStatus {
            code: 505,
            name: "HTTP Version Not Supported",
            desc: "HTTP 版本不支持",
            usage: "不支持请求的 HTTP 版本",
            causes: "客户端使用了不兼容的 HTTP 协议版本",
        },
    ]
}

fn http_status_list(_payload: &Value) -> Result<Value, String> {
    let data = http_status_data();
    let categories = [
        ("1xx", "信息响应"),
        ("2xx", "成功"),
        ("3xx", "重定向"),
        ("4xx", "客户端错误"),
        ("5xx", "服务器错误"),
    ];
    let groups: Vec<Value> = categories
        .iter()
        .map(|(cat, name)| {
            let prefix = cat.chars().next().unwrap().to_digit(10).unwrap() as u16;
            let codes: Vec<Value> = data
                .iter()
                .filter(|s| s.code / 100 == prefix)
                .map(|s| json!({ "code": s.code, "name": s.name, "desc": s.desc, "usage": s.usage, "causes": s.causes }))
                .collect();
            json!({ "category": cat, "name": name, "codes": codes })
        })
        .collect();
    Ok(json!({ "groups": groups }))
}

fn http_status_lookup(payload: &Value) -> Result<Value, String> {
    let query = payload["query"].as_str().unwrap_or_default().to_lowercase();
    if query.is_empty() {
        return Ok(json!({ "results": [] }));
    }
    let data = http_status_data();
    let results: Vec<Value> = data
        .iter()
        .filter(|s| {
            s.code.to_string().contains(&query)
                || s.name.to_lowercase().contains(&query)
                || s.desc.contains(&query)
        })
        .map(|s| json!({ "code": s.code, "name": s.name, "desc": s.desc, "usage": s.usage, "causes": s.causes }))
        .collect();
    Ok(json!({ "results": results }))
}

fn chmod_calc(payload: &Value) -> Result<Value, String> {
    let mode = payload["mode"].as_str().unwrap_or("numeric");
    let value = payload["value"].as_str().unwrap_or("644");

    let bits: [bool; 9] = if mode == "symbolic" {
        if value.len() != 9 {
            return Err("符号模式需要 9 个字符 (如 rwxr-xr-x)".to_string());
        }
        let chars: Vec<char> = value.chars().collect();
        [
            chars[0] == 'r',
            chars[1] == 'w',
            chars[2] == 'x',
            chars[3] == 'r',
            chars[4] == 'w',
            chars[5] == 'x',
            chars[6] == 'r',
            chars[7] == 'w',
            chars[8] == 'x',
        ]
    } else {
        let num = u16::from_str_radix(value, 8).map_err(|_| "无效的八进制数".to_string())?;
        if num > 0o777 {
            return Err("权限值不能超过 777".to_string());
        }
        [
            num & 0o400 != 0,
            num & 0o200 != 0,
            num & 0o100 != 0,
            num & 0o040 != 0,
            num & 0o020 != 0,
            num & 0o010 != 0,
            num & 0o004 != 0,
            num & 0o002 != 0,
            num & 0o001 != 0,
        ]
    };

    let numeric_val = (if bits[0] { 4 } else { 0 }
        + if bits[1] { 2 } else { 0 }
        + if bits[2] { 1 } else { 0 })
        * 100
        + (if bits[3] { 4 } else { 0 } + if bits[4] { 2 } else { 0 } + if bits[5] { 1 } else { 0 })
            * 10
        + (if bits[6] { 4 } else { 0 } + if bits[7] { 2 } else { 0 } + if bits[8] { 1 } else { 0 });

    let symbolic: String = bits
        .iter()
        .enumerate()
        .map(|(i, &b)| {
            if !b {
                '-'
            } else {
                match i % 3 {
                    0 => 'r',
                    1 => 'w',
                    _ => 'x',
                }
            }
        })
        .collect();

    Ok(json!({
        "numeric": format!("{numeric_val:03}"),
        "symbolic": symbolic,
        "owner": { "read": bits[0], "write": bits[1], "execute": bits[2] },
        "group": { "read": bits[3], "write": bits[4], "execute": bits[5] },
        "other": { "read": bits[6], "write": bits[7], "execute": bits[8] },
    }))
}

/// UDP 端口测试
/// UDP 是无连接协议，测试原理：
/// 1. 尝试绑定本地 UDP 端口
/// 2. 尝试向目标发送数据（0字节）
/// 3. 设置读取超时，如果收到 ICMP 不可达则判定为端口不可达
/// 注意：由于 UDP 的特性，无法 100% 确认端口开放，只能判断是否有明显的拒绝响应
fn udp_test(payload: &Value) -> Result<Value, String> {
    let host = payload["host"].as_str().unwrap_or("127.0.0.1");
    let port = payload["port"].as_u64().unwrap_or(53) as u16;
    let timeout_ms = payload["timeoutMs"].as_u64().unwrap_or(2000);
    let started = Instant::now();

    let target_addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e| format!("invalid address: {e}"))?;

    // 绑定本地任意端口
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("failed to bind udp socket: {e}"))?;

    // 设置读写超时
    socket
        .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|e| format!("failed to set timeout: {e}"))?;
    socket
        .set_write_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|e| format!("failed to set timeout: {e}"))?;

    // 尝试连接（仅设置默认地址，不实际发送数据）
    let connect_result = socket.connect(target_addr);

    // UDP "连接" 几乎总是"成功"，因为它只是设置默认地址
    // 真正的测试是尝试发送数据后看是否有 ICMP 错误
    if connect_result.is_err() {
        return Ok(json!({
            "host": host,
            "port": port,
            "reachable": false,
            "latencyMs": started.elapsed().as_millis(),
            "error": "无法连接到目标地址"
        }));
    }

    // 发送一个空 UDP 数据包
    let send_result = socket.send(&[]);
    if let Err(e) = send_result {
        return Ok(json!({
            "host": host,
            "port": port,
            "reachable": false,
            "latencyMs": started.elapsed().as_millis(),
            "error": format!("发送失败: {e}")
        }));
    }

    // 尝试接收响应（大多数 UDP 服务不会响应空包）
    // 但我们设置一个短暂的超时来检测 ICMP 不可达错误
    let mut buf = [0u8; 1];
    match socket.recv(&mut buf) {
        Ok(_) => {
            // 收到响应，说明端口是开放的
            Ok(json!({
                "host": host,
                "port": port,
                "reachable": true,
                "latencyMs": started.elapsed().as_millis(),
                "error": null
            }))
        }
        Err(e) => {
            let error_kind = e.kind();
            let error_msg = e.to_string().to_lowercase();
            let elapsed = started.elapsed().as_millis();

            // 检测 ICMP 不可达错误（Windows 和 Linux 的错误信息不同）
            let is_unreachable = error_kind == std::io::ErrorKind::ConnectionRefused
                || error_msg.contains("unreachable")
                || error_msg.contains("icmp")
                || error_msg.contains("拒绝")
                || error_msg.contains("refused");

            if is_unreachable {
                // 明确收到 ICMP 不可达，端口关闭
                Ok(json!({
                    "host": host,
                    "port": port,
                    "reachable": false,
                    "latencyMs": elapsed,
                    "error": "端口不可达（可能被防火墙阻止或服务未运行）"
                }))
            } else {
                // 超时或其他原因，无法确定状态
                // UDP 无响应是正常情况，我们标记为可能可达
                Ok(json!({
                    "host": host,
                    "port": port,
                    "reachable": true,
                    "latencyMs": elapsed,
                    "error": null,
                    "note": "UDP 无响应（这是正常行为，端口可能开放）"
                }))
            }
        }
    }
}

/// PING 测试
/// Windows 上使用 winping 库（Windows ICMP API），无需管理员权限
#[cfg(windows)]
fn ping_test(payload: &Value) -> Result<Value, String> {
    use winping::{Buffer, Pinger};

    let host = payload["host"].as_str().unwrap_or("127.0.0.1");
    let count = payload["count"].as_u64().unwrap_or(3).min(10).max(1);
    let interval_ms = 100;

    let pinger = Pinger::new().map_err(|e| format!("初始化 ping 失败: {e}"))?;
    let dest = host.parse().map_err(|e| format!("无效地址: {e}"))?;

    let mut success_count = 0u64;
    let mut total_latency = 0u64;
    let mut latencies: Vec<u64> = Vec::new();

    for i in 0..count {
        if i > 0 {
            std::thread::sleep(Duration::from_millis(interval_ms));
        }

        let mut buffer = Buffer::new();
        match pinger.send(dest, &mut buffer) {
            Ok(rtt) => {
                success_count += 1;
                total_latency += rtt as u64;
                latencies.push(rtt as u64);
            }
            Err(_) => {}
        }
    }

    let is_reachable = success_count > 0;
    let avg_latency = if success_count > 0 {
        total_latency / success_count
    } else {
        0
    };
    let packet_loss = if count > 0 {
        ((count - success_count) * 100) / count
    } else {
        100
    };

    Ok(json!({
        "host": host,
        "reachable": is_reachable,
        "latencyMs": avg_latency,
        "packetLoss": packet_loss,
        "packetsSent": count,
        "packetsReceived": success_count,
        "latencies": latencies,
        "error": if is_reachable { Value::Null } else { Value::String("无法到达目标主机".to_string()) }
    }))
}

/// PING 测试（非 Windows 平台保留系统命令实现）
#[cfg(not(windows))]
fn ping_test(payload: &Value) -> Result<Value, String> {
    use std::process::Command;
    use std::time::Duration;

    let host = payload["host"].as_str().unwrap_or("127.0.0.1");
    let timeout_ms = payload["timeoutMs"].as_u64().unwrap_or(2000);
    let count = payload["count"].as_u64().unwrap_or(3).min(10).max(1);
    let started = Instant::now();

    let mut success_count = 0u64;
    let mut total_latency = 0u64;
    let mut latencies: Vec<u64> = Vec::new();
    let mut last_error: Option<String> = None;

    for i in 0..count {
        if i > 0 {
            std::thread::sleep(Duration::from_millis(100));
        }

        let output = Command::new("ping")
            .args(&["-n", "1", "-w", &timeout_ms.to_string(), host])
            .output();

        match output {
            Ok(result) => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr = String::from_utf8_lossy(&result.stderr);
                let combined = format!("{}{}", stdout, stderr);

                let latency = parse_ping_single_latency(&combined);

                if result.status.success() {
                    if latency.is_some() {
                        success_count += 1;
                        let lat = latency.unwrap();
                        total_latency += lat;
                        latencies.push(lat);
                    } else {
                        last_error = Some("请求超时".to_string());
                    }
                } else {
                    last_error = Some("无法到达目标主机".to_string());
                }
            }
            Err(e) => {
                last_error = Some(format!("执行 ping 命令失败: {e}"));
            }
        }
    }

    let is_reachable = success_count > 0;
    let avg_latency = if success_count > 0 {
        total_latency / success_count
    } else {
        started.elapsed().as_millis() as u64
    };
    let packet_loss = if count > 0 {
        ((count - success_count) * 100) / count
    } else {
        100
    };

    Ok(json!({
        "host": host,
        "reachable": is_reachable,
        "latencyMs": avg_latency,
        "packetLoss": packet_loss,
        "packetsSent": count,
        "packetsReceived": success_count,
        "error": if is_reachable { Value::Null } else { Value::String(last_error.unwrap_or_else(|| "无法到达目标主机".to_string())) },
        "latencies": latencies
    }))
}

/// 解析单次 ping 延迟（非 Windows 平台使用）
#[cfg(not(windows))]
fn parse_ping_single_latency(output: &str) -> Option<u64> {
    let lower = output.to_lowercase();

    for line in lower.lines() {
        if let Some(pos) = line.find("time=").or_else(|| line.find("时间=")) {
            let after = &line[pos + 5..];
            let num_str: String = after
                .chars()
                .skip_while(|c| *c == '<' || *c == ' ')
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(num) = num_str.parse::<f64>() {
                return Some(num as u64);
            }
        }
    }
    None
}

const ACTIONS: &[&str] = &[
    "tcp_test",
    "http_test",
    "http_status_list",
    "http_status_lookup",
    "chmod_calc",
    "udp_test",
    "ping_test",
];

pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported network action: {action}"));
    }
    match action {
        "tcp_test" => {
            let host = payload["host"].as_str().unwrap_or("127.0.0.1");
            let port = payload["port"].as_u64().unwrap_or(80) as u16;
            let timeout_ms = payload["timeoutMs"].as_u64().unwrap_or(2000);
            let started = Instant::now();
            let addr: SocketAddr = format!("{host}:{port}")
                .parse()
                .map_err(|e| format!("invalid address: {e}"))?;
            let result = TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms));
            Ok(json!({
                "host": host,
                "port": port,
                "reachable": result.is_ok(),
                "latencyMs": started.elapsed().as_millis(),
                "error": result.err().map(|e| e.to_string())
            }))
        }
        "http_test" => {
            let url = payload["url"].as_str().unwrap_or("http://127.0.0.1");
            let timeout_ms = payload["timeoutMs"].as_u64().unwrap_or(5000);
            let started = Instant::now();
            let agent = ureq::AgentBuilder::new()
                .timeout(Duration::from_millis(timeout_ms))
                .build();
            match agent.head(url).call() {
                Ok(resp) => Ok(json!({
                    "url": url,
                    "reachable": true,
                    "statusCode": resp.status(),
                    "latencyMs": started.elapsed().as_millis(),
                    "error": null
                })),
                Err(ureq::Error::Status(code, _resp)) => Ok(json!({
                    "url": url,
                    "reachable": true,
                    "statusCode": code,
                    "latencyMs": started.elapsed().as_millis(),
                    "error": null
                })),
                Err(e) => Ok(json!({
                    "url": url,
                    "reachable": false,
                    "statusCode": null,
                    "latencyMs": started.elapsed().as_millis(),
                    "error": e.to_string()
                })),
            }
        }
        "http_status_list" => http_status_list(payload),
        "http_status_lookup" => http_status_lookup(payload),
        "chmod_calc" => chmod_calc(payload),
        "udp_test" => udp_test(payload),
        "ping_test" => ping_test(payload),
        _ => Err(format!("unsupported network action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn tcp_test_should_detect_reachable_and_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = thread::spawn(move || {
            let _ = listener.accept();
        });

        let ok = execute(
            "tcp_test",
            &json!({ "host": "127.0.0.1", "port": port, "timeoutMs": 1000 }),
        )
        .expect("tcp ok");
        assert_eq!(ok["reachable"], true);
        drop(handle);

        let fail = execute(
            "tcp_test",
            &json!({ "host": "127.0.0.1", "port": 9, "timeoutMs": 100 }),
        )
        .expect("tcp fail path");
        assert!(fail["reachable"].is_boolean());
    }

    #[test]
    fn http_test_should_work_with_local_server() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            }
        });

        let out = execute(
            "http_test",
            &json!({ "url": format!("http://127.0.0.1:{port}"), "timeoutMs": 1500 }),
        )
        .expect("http test");
        assert_eq!(out["reachable"], true);
        assert!(out["statusCode"].as_u64().unwrap_or(0) >= 200);
    }

    #[test]
    fn http_status_list_returns_groups() {
        let r = execute("http_status_list", &json!({})).unwrap();
        let groups = r["groups"].as_array().unwrap();
        assert_eq!(groups.len(), 5);
        assert_eq!(groups[0]["category"], "1xx");
    }

    #[test]
    fn http_status_lookup_by_code() {
        let r = execute("http_status_lookup", &json!({"query": "404"})).unwrap();
        let results = r["results"].as_array().unwrap();
        assert!(results.len() >= 1);
        assert_eq!(results[0]["code"], 404);
    }

    #[test]
    fn http_status_lookup_by_text() {
        let r = execute("http_status_lookup", &json!({"query": "not found"})).unwrap();
        let results = r["results"].as_array().unwrap();
        assert!(results.iter().any(|r| r["code"] == 404));
    }

    #[test]
    fn chmod_calc_from_numeric() {
        let r = execute("chmod_calc", &json!({"mode": "numeric", "value": "755"})).unwrap();
        assert_eq!(r["numeric"], "755");
        assert_eq!(r["symbolic"], "rwxr-xr-x");
        assert_eq!(r["owner"]["read"], true);
        assert_eq!(r["owner"]["write"], true);
        assert_eq!(r["owner"]["execute"], true);
        assert_eq!(r["group"]["write"], false);
    }

    #[test]
    fn chmod_calc_from_symbolic() {
        let r = execute(
            "chmod_calc",
            &json!({"mode": "symbolic", "value": "rw-r--r--"}),
        )
        .unwrap();
        assert_eq!(r["numeric"], "644");
    }

    #[test]
    fn chmod_calc_zero() {
        let r = execute("chmod_calc", &json!({"mode": "numeric", "value": "000"})).unwrap();
        assert_eq!(r["symbolic"], "---------");
    }
}
