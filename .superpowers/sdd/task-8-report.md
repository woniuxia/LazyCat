# Task 8 Report: 持久化转发日志与统计

## 范围

- 仅在 `request_forward` 域增加日志、统计持久化和 action；未修改 `main.rs`、UI 或数据库 schema。
- HTTP 仍为 HTTP-only；HTTPS 下游继续在启动时明确返回“当前版本暂不支持 HTTPS 下游”。
- TCP/UDP/HTTP 转发协议行为保持不变；HTTP 只补充请求级元数据和响应完成时的观测事件收口。

## RED 证据

先写测试并确认失败：

- `cargo test request_forward::repository::tests -- --nocapture`
  - 初始因日志/统计 repository API 不存在而失败（预期 E0432）。
- `cargo test request_forward::observability::tests -- --nocapture`
  - 初始因 `ObservationCursor` / `batch_since` 不存在而失败。
- `cargo test observability_write_failure_keeps_forwarding_and_exposes_last_error -- --nocapture`
  - 初始因 runtime persistence 注入接口不存在而失败。
- `cargo test http_sensitive_headers_and_64k_truncation_reach_database -- --nocapture`
  - 增加 baseline 已推进后的请求级字节断言后，初始日志字节为 `(0, 0)`，证明事件不能依赖批次 delta。
- `cargo test stats_get_repeated_flush_and_active_reset_do_not_double_count -- --nocapture`
  - 初始 `stats_reset` 会把未 flush 日志一起跳过：实际 1 条、预期 2 条。

## 实现模型

### Repository

- `persist_observability_with_conn` 在单事务内累加统计、插入日志并按 `rule_id` 保留最新 1000 条（`created_at DESC, id DESC`）。
- `list_logs_with_conn` 先执行规则/关键词/成功或错误过滤，再稳定排序和分页。
- `log_clear` 只删选定规则日志；`stats_reset` 只归零统计。
- HTTP header 以脱敏后的 JSON 写入，body preview 保留 64 KiB 和 truncation flags。

### Snapshot / delta / flush

- 每个协议观测维护累计 `StatsDelta` 和单调 event sequence；`batch_since(cursor)` 返回 delta、事件、下一 cursor 和环形缓冲 gap。
- `stats_get` 读取持久化总量并叠加当前 cursor 后的内存 delta；读取不会推进 cursor，因此重复读取不重复计数。
- cadence worker 每秒尝试 flush；仅 SQLite 事务成功后推进 cursor。失败只更新单条有界 `lastObservabilityError`，不停止转发、不排队重试。
- 规则停止和 worker completion 会发送停止信号并执行最终 flush。
- event sequence 出现环形缓冲缺口时，仍持久化可见事件和统计，同时保留显式 gap 错误，避免伪称全部日志已落库。
- `stats_reset` 在规则锁内归零持久化统计，只更新 cursor 的 totals baseline，保留 event sequence，因而不会丢弃尚未 flush 的日志；日志表不清空。

### Actions / status

- 接入 `log_list`、`log_clear`、`stats_get`、`stats_reset`，payload 错误和缺失规则显式返回错误。
- runtime status/list 输出新增独立 `lastObservabilityError`，不覆盖 runtime `lastError`，也不把 DB 写失败标记为 stopped/failed。

## GREEN / verification

串行执行（避免 Cargo 测试资源互相干扰）：

```text
cargo test request_forward::repository::tests -- --nocapture
4 passed, 0 failed

cargo test request_forward::observability::tests -- --nocapture
7 passed, 0 failed

cargo test request_forward -- --nocapture
58 passed, 0 failed

cargo check
Finished successfully, no request_forward warnings

git diff --check
passed
```

额外覆盖：三协议 event_count 持久化语义、HTTP 敏感 header 脱敏、64 KiB 截断、重复 stats_get/flush、active stats_reset、camelCase action/status serialization、DB failure forwarding degradation、HTTPS 明确拒绝。

## Concern

累计统计始终精确；TCP/UDP 在同一 cadence 批次包含多个事件时，日志行的字节字段按批次聚合写入首个可见事件，整体统计字节不丢失，但单行到单连接/单数据报的字节归属不是逐事件精确拆分。后续若需要逐连接/逐数据报审计，应在协议事件记录中增加 per-event byte counters；本任务未改变转发语义，避免扩大到协议生命周期重构。
