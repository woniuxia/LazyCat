//! Redis 驱动：连接管理、SCAN 分批浏览、类型感知的 key 详情、受控写操作、
//! 命令控制台（黑名单 + 阻塞类拒绝 + 超时）。
//!
//! 与 SQL 引擎不同，Redis 走独立的 KV 语义，不复用 DbPool / DbConn。
//! 连接为 MultiplexedConnection（可 Clone），按 connectionId + db 编号缓存。

use std::time::Duration;

use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use serde_json::{json, Value};

use super::ConnectConfig;

/// 单值读取的展示截断上限（字节）。
const STRING_PREVIEW_LIMIT: usize = 64 * 1024;
/// 集合类型成员的单次加载上限。
const MEMBER_LIMIT: usize = 200;
/// 命令控制台执行超时。
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// 阻塞与订阅类命令：控制台不支持长连接语义，直接拒绝。
const BLOCKING_COMMANDS: &[&str] = &[
    "SUBSCRIBE", "PSUBSCRIBE", "UNSUBSCRIBE", "PUNSUBSCRIBE", "SSUBSCRIBE", "SUNSUBSCRIBE",
    "MONITOR", "BLPOP", "BRPOP", "BRPOPLPUSH", "BLMOVE", "BLMPOP", "BZPOPMIN", "BZPOPMAX",
    "BZMPOP", "WAIT", "WAITAOF",
];

/// 破坏性命令：需要用户在弹窗中手动输入命令名确认（confirmed 标志）。
const DANGEROUS_COMMANDS: &[&str] = &[
    "FLUSHALL", "FLUSHDB", "CONFIG", "SHUTDOWN", "DEBUG", "SAVE", "BGSAVE", "BGREWRITEAOF",
    "REPLICAOF", "SLAVEOF", "SWAPDB", "RESET", "FAILOVER", "CLUSTER", "SCRIPT", "FUNCTION",
    "MIGRATE", "RESTORE",
];

/// 只读连接允许的命令白名单（不在名单内的命令一律拒绝）。
const READONLY_COMMANDS: &[&str] = &[
    "GET", "MGET", "STRLEN", "GETRANGE", "EXISTS", "TYPE", "TTL", "PTTL", "SCAN", "KEYS",
    "RANDOMKEY", "DBSIZE", "HGET", "HGETALL", "HMGET", "HLEN", "HKEYS", "HVALS", "HSCAN",
    "HSTRLEN", "HEXISTS", "LRANGE", "LLEN", "LINDEX", "LPOS", "SCARD", "SMEMBERS", "SSCAN",
    "SISMEMBER", "SRANDMEMBER", "ZRANGE", "ZRANGEBYSCORE", "ZCARD", "ZSCORE", "ZSCAN", "ZCOUNT",
    "ZRANK", "ZREVRANK", "OBJECT", "MEMORY", "INFO", "PING", "ECHO", "TIME", "CLIENT", "SELECT",
    "DUMP", "BITCOUNT", "GETBIT", "SINTERCARD", "LCS", "XLEN", "XRANGE", "XINFO",
];

pub fn classify_command(verb: &str) -> CommandClass {
    let v = verb.to_ascii_uppercase();
    if BLOCKING_COMMANDS.contains(&v.as_str()) {
        CommandClass::Blocking
    } else if DANGEROUS_COMMANDS.contains(&v.as_str()) {
        CommandClass::Dangerous
    } else if READONLY_COMMANDS.contains(&v.as_str()) {
        CommandClass::Readonly
    } else {
        CommandClass::Write
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClass {
    Readonly,
    Write,
    Dangerous,
    Blocking,
}

/// 解析控制台命令行：按空白拆分，支持单/双引号包裹含空格参数。
pub fn parse_command_line(line: &str) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut has_token = false;
    for ch in line.chars() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    has_token = true;
                }
                c if c.is_whitespace() => {
                    if has_token {
                        args.push(std::mem::take(&mut current));
                        has_token = false;
                    }
                }
                c => {
                    current.push(c);
                    has_token = true;
                }
            },
        }
    }
    if quote.is_some() {
        return Err("命令中的引号未闭合".into());
    }
    if has_token {
        args.push(current);
    }
    if args.is_empty() {
        return Err("命令不能为空".into());
    }
    Ok(args)
}

pub async fn connect(cfg: &ConnectConfig, db_index: i64) -> Result<MultiplexedConnection, String> {
    let info = redis::ConnectionInfo {
        addr: redis::ConnectionAddr::Tcp(cfg.host.clone(), cfg.port),
        redis: redis::RedisConnectionInfo {
            db: db_index,
            username: if cfg.username.is_empty() { None } else { Some(cfg.username.clone()) },
            password: if cfg.password.is_empty() { None } else { Some(cfg.password.clone()) },
            protocol: redis::ProtocolVersion::RESP2,
        },
    };
    let client = redis::Client::open(info).map_err(|e| format!("连接配置无效: {e}"))?;
    tokio::time::timeout(
        Duration::from_secs(cfg.connect_timeout_secs),
        client.get_multiplexed_tokio_connection(),
    )
    .await
    .map_err(|_| format!("连接超时（{} 秒）", cfg.connect_timeout_secs))?
    .map_err(|e| format!("连接失败: {e}"))
}

pub async fn server_version(conn: &mut MultiplexedConnection) -> Result<String, String> {
    let info: String = redis::cmd("INFO")
        .arg("server")
        .query_async(conn)
        .await
        .map_err(|e| format!("查询版本失败: {e}"))?;
    let version = info
        .lines()
        .find_map(|l| l.strip_prefix("redis_version:"))
        .unwrap_or("unknown")
        .trim()
        .to_string();
    Ok(format!("Redis {version}"))
}

/// 逻辑库数量（CONFIG GET databases 可能被禁用，兜底 16）。
pub async fn database_count(conn: &mut MultiplexedConnection) -> usize {
    let result: Result<Vec<String>, _> = redis::cmd("CONFIG")
        .arg("GET")
        .arg("databases")
        .query_async(conn)
        .await;
    result
        .ok()
        .and_then(|v| v.get(1).and_then(|s| s.parse::<usize>().ok()))
        .unwrap_or(16)
}

/// SCAN 一批 key，并用 pipeline 批量取每个 key 的类型。
pub async fn scan_keys(
    conn: &mut MultiplexedConnection,
    cursor: u64,
    pattern: &str,
    count: usize,
) -> Result<(u64, Vec<(String, String)>), String> {
    let mut cmd = redis::cmd("SCAN");
    cmd.arg(cursor);
    if !pattern.is_empty() {
        cmd.arg("MATCH").arg(pattern);
    }
    cmd.arg("COUNT").arg(count);
    let (next, keys): (u64, Vec<String>) = cmd
        .query_async(conn)
        .await
        .map_err(|e| format!("SCAN 失败: {e}"))?;

    if keys.is_empty() {
        return Ok((next, Vec::new()));
    }
    let mut pipe = redis::pipe();
    for key in &keys {
        pipe.cmd("TYPE").arg(key);
    }
    let types: Vec<String> = pipe
        .query_async(conn)
        .await
        .map_err(|e| format!("查询类型失败: {e}"))?;
    Ok((next, keys.into_iter().zip(types).collect()))
}

pub async fn key_detail(conn: &mut MultiplexedConnection, key: &str) -> Result<Value, String> {
    let key_type: String = redis::cmd("TYPE")
        .arg(key)
        .query_async(conn)
        .await
        .map_err(|e| format!("查询类型失败: {e}"))?;
    if key_type == "none" {
        return Err("key 不存在（可能已过期或被删除）".into());
    }
    let ttl: i64 = conn.ttl(key).await.map_err(|e| format!("查询 TTL 失败: {e}"))?;
    let encoding: String = redis::cmd("OBJECT")
        .arg("ENCODING")
        .arg(key)
        .query_async(conn)
        .await
        .unwrap_or_else(|_| "unknown".to_string());
    let memory: Option<i64> = redis::cmd("MEMORY")
        .arg("USAGE")
        .arg(key)
        .query_async(conn)
        .await
        .ok();

    let (value, total, truncated) = match key_type.as_str() {
        "string" => {
            let len: usize = conn.strlen(key).await.unwrap_or(0);
            let raw: Vec<u8> = redis::cmd("GETRANGE")
                .arg(key)
                .arg(0)
                .arg(STRING_PREVIEW_LIMIT as isize - 1)
                .query_async(conn)
                .await
                .map_err(|e| format!("读取值失败: {e}"))?;
            let text = String::from_utf8_lossy(&raw).to_string();
            (json!(text), len as i64, len > STRING_PREVIEW_LIMIT)
        }
        "hash" => {
            let total: i64 = conn.hlen(key).await.unwrap_or(0);
            let (_, entries): (u64, Vec<(String, String)>) = redis::cmd("HSCAN")
                .arg(key)
                .arg(0)
                .arg("COUNT")
                .arg(MEMBER_LIMIT)
                .query_async(conn)
                .await
                .map_err(|e| format!("读取 hash 失败: {e}"))?;
            let capped: Vec<_> = entries
                .into_iter()
                .take(MEMBER_LIMIT)
                .map(|(f, v)| json!({ "field": f, "value": v }))
                .collect();
            let truncated = (capped.len() as i64) < total;
            (json!(capped), total, truncated)
        }
        "list" => {
            let total: i64 = conn.llen(key).await.unwrap_or(0);
            let items: Vec<String> = conn
                .lrange(key, 0, MEMBER_LIMIT as isize - 1)
                .await
                .map_err(|e| format!("读取 list 失败: {e}"))?;
            let truncated = (items.len() as i64) < total;
            (json!(items), total, truncated)
        }
        "set" => {
            let total: i64 = conn.scard(key).await.unwrap_or(0);
            let (_, members): (u64, Vec<String>) = redis::cmd("SSCAN")
                .arg(key)
                .arg(0)
                .arg("COUNT")
                .arg(MEMBER_LIMIT)
                .query_async(conn)
                .await
                .map_err(|e| format!("读取 set 失败: {e}"))?;
            let capped: Vec<String> = members.into_iter().take(MEMBER_LIMIT).collect();
            let truncated = (capped.len() as i64) < total;
            (json!(capped), total, truncated)
        }
        "zset" => {
            let total: i64 = conn.zcard(key).await.unwrap_or(0);
            let entries: Vec<(String, f64)> = conn
                .zrange_withscores(key, 0, MEMBER_LIMIT as isize - 1)
                .await
                .map_err(|e| format!("读取 zset 失败: {e}"))?;
            let truncated = (entries.len() as i64) < total;
            let items: Vec<_> = entries
                .into_iter()
                .map(|(m, s)| json!({ "member": m, "score": s }))
                .collect();
            (json!(items), total, truncated)
        }
        other => (json!(format!("（暂不支持预览的类型: {other}）")), 0, false),
    };

    Ok(json!({
        "key": key,
        "type": key_type,
        "ttl": ttl,
        "encoding": encoding,
        "memory": memory,
        "value": value,
        "total": total,
        "truncated": truncated,
    }))
}

/// 受控写操作。action 与参数由 db.rs 校验连接只读标记后转发。
pub async fn key_write(
    conn: &mut MultiplexedConnection,
    action: &str,
    key: &str,
    payload: &Value,
) -> Result<Value, String> {
    let arg = |name: &str| -> Result<String, String> {
        payload[name]
            .as_str()
            .map(String::from)
            .or_else(|| payload[name].as_i64().map(|v| v.to_string()))
            .or_else(|| payload[name].as_f64().map(|v| v.to_string()))
            .ok_or(format!("缺少参数 {name}"))
    };
    match action {
        "set_string" => {
            let value = arg("value")?;
            let _: () = conn.set(key, value).await.map_err(err("写入失败"))?;
        }
        "del" => {
            let _: () = conn.del(key).await.map_err(err("删除失败"))?;
        }
        "expire" => {
            let ttl = payload["ttlSecs"].as_i64().ok_or("缺少参数 ttlSecs")?;
            if ttl < 0 {
                let _: () = conn.persist(key).await.map_err(err("取消过期失败"))?;
            } else {
                let _: () = conn.expire(key, ttl).await.map_err(err("设置 TTL 失败"))?;
            }
        }
        "rename" => {
            let new_key = arg("newKey")?;
            let _: () = conn.rename(key, new_key).await.map_err(err("重命名失败"))?;
        }
        "hset" => {
            let _: () = conn
                .hset(key, arg("field")?, arg("value")?)
                .await
                .map_err(err("写入字段失败"))?;
        }
        "hdel" => {
            let _: () = conn.hdel(key, arg("field")?).await.map_err(err("删除字段失败"))?;
        }
        "lset" => {
            let index = payload["index"].as_i64().ok_or("缺少参数 index")?;
            let _: () = conn
                .lset(key, index as isize, arg("value")?)
                .await
                .map_err(err("修改列表元素失败"))?;
        }
        "rpush" => {
            let _: () = conn.rpush(key, arg("value")?).await.map_err(err("追加元素失败"))?;
        }
        "lrem" => {
            let _: () = conn
                .lrem(key, 1, arg("value")?)
                .await
                .map_err(err("移除元素失败"))?;
        }
        "sadd" => {
            let _: () = conn.sadd(key, arg("member")?).await.map_err(err("添加成员失败"))?;
        }
        "srem" => {
            let _: () = conn.srem(key, arg("member")?).await.map_err(err("移除成员失败"))?;
        }
        "zadd" => {
            let score = payload["score"].as_f64().ok_or("缺少参数 score")?;
            let _: () = conn
                .zadd(key, arg("member")?, score)
                .await
                .map_err(err("添加成员失败"))?;
        }
        "zrem" => {
            let _: () = conn.zrem(key, arg("member")?).await.map_err(err("移除成员失败"))?;
        }
        other => return Err(format!("未知写操作: {other}")),
    }
    Ok(json!({ "ok": true }))
}

fn err(prefix: &'static str) -> impl Fn(redis::RedisError) -> String {
    move |e| format!("{prefix}: {e}")
}

/// 执行控制台命令（分类校验由 db.rs 完成），带超时。
pub async fn run_command(
    conn: &mut MultiplexedConnection,
    args: &[String],
) -> Result<Value, String> {
    let mut cmd = redis::cmd(&args[0].to_ascii_uppercase());
    for a in &args[1..] {
        cmd.arg(a);
    }
    let result = tokio::time::timeout(COMMAND_TIMEOUT, cmd.query_async::<redis::Value>(conn))
        .await
        .map_err(|_| format!("命令超时（{} 秒）", COMMAND_TIMEOUT.as_secs()))?
        .map_err(|e| format!("{e}"))?;
    Ok(redis_value_to_json(&result))
}

fn redis_value_to_json(value: &redis::Value) -> Value {
    match value {
        redis::Value::Nil => Value::Null,
        redis::Value::Int(i) => json!(i),
        redis::Value::BulkString(bytes) => json!(String::from_utf8_lossy(bytes).to_string()),
        redis::Value::SimpleString(s) => json!(s),
        redis::Value::Okay => json!("OK"),
        redis::Value::Array(items) | redis::Value::Set(items) => {
            json!(items.iter().map(redis_value_to_json).collect::<Vec<_>>())
        }
        redis::Value::Map(pairs) => {
            let obj: Vec<Value> = pairs
                .iter()
                .map(|(k, v)| json!([redis_value_to_json(k), redis_value_to_json(v)]))
                .collect();
            json!(obj)
        }
        redis::Value::Double(d) => json!(d),
        redis::Value::Boolean(b) => json!(b),
        redis::Value::BigNumber(n) => json!(n.to_string()),
        redis::Value::VerbatimString { text, .. } => json!(text),
        other => json!(format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_line_basic() {
        assert_eq!(parse_command_line("GET foo").unwrap(), vec!["GET", "foo"]);
        assert_eq!(
            parse_command_line("SET key \"hello world\"").unwrap(),
            vec!["SET", "key", "hello world"]
        );
        assert_eq!(
            parse_command_line("SET k 'a b'  extra").unwrap(),
            vec!["SET", "k", "a b", "extra"]
        );
        // 空引号是合法的空参数
        assert_eq!(parse_command_line("SET k \"\"").unwrap(), vec!["SET", "k", ""]);
    }

    #[test]
    fn parse_command_line_rejects_invalid() {
        assert!(parse_command_line("").is_err());
        assert!(parse_command_line("   ").is_err());
        assert!(parse_command_line("SET k \"unclosed").is_err());
    }

    #[test]
    fn classify_commands() {
        assert_eq!(classify_command("get"), CommandClass::Readonly);
        assert_eq!(classify_command("SCAN"), CommandClass::Readonly);
        assert_eq!(classify_command("SET"), CommandClass::Write);
        assert_eq!(classify_command("DEL"), CommandClass::Write);
        assert_eq!(classify_command("FLUSHALL"), CommandClass::Dangerous);
        assert_eq!(classify_command("config"), CommandClass::Dangerous);
        assert_eq!(classify_command("SUBSCRIBE"), CommandClass::Blocking);
        assert_eq!(classify_command("BLPOP"), CommandClass::Blocking);
        assert_eq!(classify_command("MONITOR"), CommandClass::Blocking);
    }

    #[test]
    fn redis_value_to_json_variants() {
        assert_eq!(redis_value_to_json(&redis::Value::Nil), Value::Null);
        assert_eq!(redis_value_to_json(&redis::Value::Int(5)), json!(5));
        assert_eq!(redis_value_to_json(&redis::Value::Okay), json!("OK"));
        assert_eq!(
            redis_value_to_json(&redis::Value::BulkString(b"abc".to_vec())),
            json!("abc")
        );
        assert_eq!(
            redis_value_to_json(&redis::Value::Array(vec![
                redis::Value::Int(1),
                redis::Value::Nil
            ])),
            json!([1, null])
        );
    }
}
