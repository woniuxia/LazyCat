//! `tool:widget:apply` —— 推数据给挂件
//!
//! 重构后职责精简：
//! 1. 拉 dashboard_data
//! 2. 注入 privacyMask 标记
//! 3. 算内容 hash（force=false 命中跳过）
//! 4. ensure 挂件存在（失败不阻塞）
//! 5. emit `widget://color-mode` + `widget://dashboard-data`
//! 6. 写 state.last_rendered_at
//!
//! 不再做：CapturePreview / 图像合成 / canvas-ready 握手 / hidden WebView 重建。

#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::tools::widget::{config, dashboard_logic, data, state, widget};

/// 上一次推送的输入 hash（dashboard 内容 + privacy_mask）。
/// 0 = 失效；调用 [`invalidate_input_hash`] 强制下一轮真正推一次。
static LAST_INPUT_HASH: AtomicU64 = AtomicU64::new(0);

/// 重置内容 hash；enable / 老板键恢复 / 用户交互后调用。
pub fn invalidate_input_hash() {
    LAST_INPUT_HASH.store(0, Ordering::SeqCst);
}

/// `tool:widget:apply` 入口；force=true 跳过 hash 去重。
pub fn apply(app: &AppHandle) -> Result<Value, String> {
    apply_with_force(app, true)
}

/// 调度 / 事件驱动入口；force=false 启用内容 hash 去重。
pub fn apply_with_force(app: &AppHandle, force: bool) -> Result<Value, String> {
    let start = Instant::now();
    eprintln!("[widget] apply: enter (force={force})");

    // 0. 快速路径：已禁用时跳过，防御 TOCTOU 竞态
    //    （scheduler / events 在 should_skip 之后到进入本函数之间，用户可能已 disable）
    let cfg = config::read_config();
    if !cfg.enabled {
        eprintln!("[widget] apply: skipped (disabled)");
        return Ok(json!({
            "ok": true,
            "skipped": true,
            "reason": "disabled",
            "elapsedMs": start.elapsed().as_millis() as u64,
        }));
    }

    // 1. 数据
    let dashboard = data::dashboard_data(&Value::Null)?;

    // 2. 解析 privacy 状态（含到期清零）
    let privacy_mask = resolve_privacy_mask(&cfg);

    // 3. 内容 hash
    let overview_value = dashboard.get("overview").cloned().unwrap_or(Value::Null);
    let todo_list_value: Vec<Value> = dashboard
        .get("todoList")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let hot_tools_value: Vec<Value> = dashboard
        .get("hotTools")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let input_hash = compute_input_hash(&overview_value, &todo_list_value, &hot_tools_value, privacy_mask);
    let prev_hash = LAST_INPUT_HASH.load(Ordering::SeqCst);
    if !force && input_hash != 0 && input_hash == prev_hash {
        eprintln!("[widget] apply: skipped (no-change, hash={input_hash:#x})");
        return Ok(json!({
            "ok": true,
            "skipped": true,
            "reason": "no-change",
            "elapsedMs": start.elapsed().as_millis() as u64,
        }));
    }

    // 4. 确保挂件存在（失败仅 log，不阻塞数据推送——下次 apply 自然重试）
    if let Err(e) = widget::ensure(app) {
        eprintln!("[widget] apply: widget ensure failed: {e}");
    }

    // 5. 注入 privacyMask 标记
    let mut dashboard_emit = dashboard;
    if let Some(obj) = dashboard_emit.as_object_mut() {
        obj.insert("privacyMask".into(), Value::Bool(privacy_mask));
    }

    // 6. 推送
    //    color-mode 固定 "dark"（浅玻璃蒙层 + 深色字）：挂件背景透明，
    //    深色字在多数壁纸上对比度都可接受，且不依赖 base 壁纸采样。
    //    未来可让用户在面板里切换 light / dark / auto。
    if let Err(e) = app.emit("widget://color-mode", "dark") {
        eprintln!("[widget] apply: emit color-mode failed: {e}");
    }
    app.emit("widget://dashboard-data", &dashboard_emit)
        .map_err(|e| format!("emit dashboard-data failed: {e}"))?;

    // 7. 写状态
    state::write(|s| {
        s.last_rendered_at = Some(now_iso());
        s.last_error = None;
    });

    if input_hash != 0 {
        LAST_INPUT_HASH.store(input_hash, Ordering::SeqCst);
    }

    let elapsed = start.elapsed().as_millis() as u64;
    eprintln!("[widget] apply: done in {elapsed}ms");
    Ok(json!({
        "ok": true,
        "skipped": false,
        "elapsedMs": elapsed,
        "privacyMask": privacy_mask,
    }))
}

// ── 内部 ──────────────────────────────────────────

fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

/// 判定当前是否处于敏感模式；过期则同步写回 widget.privacy_mask=false +
/// 清空 widget.privacy_mask_until。
fn resolve_privacy_mask(cfg: &config::WidgetConfig) -> bool {
    if !cfg.privacy_mask {
        return false;
    }
    let Some(until) = cfg.privacy_mask_until.as_deref() else {
        return true;
    };
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(until) else {
        return true;
    };
    if chrono::Utc::now() <= dt.with_timezone(&chrono::Utc) {
        return true;
    }
    if let Err(e) = config::set_string(config::KEY_PRIVACY_MASK, "false") {
        eprintln!("[widget] auto-clear privacy_mask failed: {e}");
    }
    if let Err(e) = config::set_string(config::KEY_PRIVACY_MASK_UNTIL, "") {
        eprintln!("[widget] auto-clear privacy_mask_until failed: {e}");
    }
    false
}

/// 内容哈希：dashboard（去掉 generatedAt 时间戳）+ hotTools + privacy_mask。
/// 0 → 1 避免与 sentinel 冲突。
fn compute_input_hash(overview: &Value, todo_list: &[Value], hot_tools: &[Value], privacy_mask: bool) -> u64 {
    let dashboard_hex = dashboard_logic::compute_dashboard_hash(overview, todo_list);

    let mut hasher = blake3::Hasher::new();
    hasher.update(dashboard_hex.as_bytes());
    hasher.update(b"|");
    hasher.update(if privacy_mask { b"1" } else { b"0" });
    hasher.update(b"|");
    // 序列化 hotTools 纳入 hash，确保工具推荐变化时能触发推送
    hasher.update(serde_json::to_string(hot_tools).unwrap_or_default().as_bytes());

    let bytes = hasher.finalize();
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes.as_bytes()[..8]);
    let result = u64::from_le_bytes(buf);
    if result == 0 {
        1
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_stable_for_same_input() {
        let overview = json!({ "completedToday": 1, "totalToday": 5 });
        let todos = vec![json!({ "id": "pm:1", "title": "x" })];
        let hot = vec![json!({ "id": "pm", "count": 3 })];
        let a = compute_input_hash(&overview, &todos, &hot, false);
        let b = compute_input_hash(&overview, &todos, &hot, false);
        assert_eq!(a, b);
    }

    #[test]
    fn hash_changes_with_todo_list() {
        let h1 = compute_input_hash(&json!({}), &[json!({ "id": "a" })], &[], false);
        let h2 = compute_input_hash(&json!({}), &[json!({ "id": "b" })], &[], false);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_changes_with_overview() {
        let h1 = compute_input_hash(&json!({ "p0Pending": 0 }), &[], &[], false);
        let h2 = compute_input_hash(&json!({ "p0Pending": 1 }), &[], &[], false);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_changes_with_privacy_mask() {
        let h1 = compute_input_hash(&json!({}), &[], &[], false);
        let h2 = compute_input_hash(&json!({}), &[], &[], true);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_changes_with_hot_tools() {
        let h1 = compute_input_hash(&json!({}), &[], &[json!({ "id": "pm" })], false);
        let h2 = compute_input_hash(&json!({}), &[], &[json!({ "id": "inbox" })], false);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_never_returns_sentinel_zero() {
        let h = compute_input_hash(&json!({}), &[], &[], false);
        assert_ne!(h, 0);
    }
}