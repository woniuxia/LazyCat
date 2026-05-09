//! 挂件刷新心跳调度（v2 挂件版）
//!
//! - 后台线程按 `widget.refresh_interval_min` 心跳触发 [`apply::apply_with_force`]
//! - 启动时若 `enabled=true`，先 ensure widget + 立即 apply 一次
//! - 跳过条件：未启用 / 暂停态 / 锁屏 / 全屏切净（lock + fullscreen 模块）
//! - **挂件状态联动**：should_skip 命中 → widget::set_state(Hidden)；
//!   未命中且当前是 Hidden（自动跳过 reason 释放）→ set_state(Peek)
//! - 空闲降频：5min 无输入降到 60min sleep；用户回来立即 break sleep

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::AppHandle;

use crate::tools::widget::{apply, conflicts, config, fullscreen, idle, lock, state, widget};

/// 5min 无输入即视为空闲，sleep 间隔降为 [`IDLE_INTERVAL_SECS`]。
const IDLE_THRESHOLD_SECS: u32 = 300;
/// 空闲降频后的固定间隔（60min）。
const IDLE_INTERVAL_SECS: u64 = 60 * 60;
/// 用户「刚回来」判定阈值：本周期 < 30s 输入间隔即视为活跃。
const ACTIVE_THRESHOLD_SECS: u32 = 30;
/// 分块睡眠粒度。
const SLEEP_CHUNK_SECS: u64 = 30;

static SCHEDULER_RUNNING: AtomicBool = AtomicBool::new(false);

/// 启动后台心跳线程；幂等。
pub fn start(app: AppHandle) {
    if SCHEDULER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    conflicts::refresh();

    std::thread::spawn(move || {
        let mut last_conflict_check = std::time::Instant::now();
        loop {
            // 每 10min 重新探测一次冲突
            if last_conflict_check.elapsed() >= std::time::Duration::from_secs(600) {
                conflicts::refresh();
                last_conflict_check = std::time::Instant::now();
            }

            let skip = should_skip();
            sync_widget_visibility(&app, skip);

            if !skip {
                if let Err(e) = apply::apply_with_force(&app, false) {
                    handle_apply_error(&e);
                }
            }

            let cfg_interval_min = config::read_config().refresh_interval_min.max(1) as u64;
            let target_secs = if idle::seconds_idle() >= IDLE_THRESHOLD_SECS {
                IDLE_INTERVAL_SECS
            } else {
                cfg_interval_min * 60
            };

            sleep_with_idle_check(target_secs);
        }
    });
}

/// 分块 sleep；进入时若已空闲则记录，期间检测「刚回来」立即 break。
fn sleep_with_idle_check(target_secs: u64) {
    let was_idle_at_start = idle::seconds_idle() >= IDLE_THRESHOLD_SECS;
    let mut elapsed = 0u64;

    while elapsed < target_secs {
        let chunk = (target_secs - elapsed).min(SLEEP_CHUNK_SECS);
        std::thread::sleep(Duration::from_secs(chunk));
        elapsed += chunk;

        if was_idle_at_start && idle::seconds_idle() < ACTIVE_THRESHOLD_SECS {
            return;
        }
    }
}

/// 跳过判定：禁用 / 显式暂停 / 锁屏 / 全屏切净 都跳过。
///
/// 写 `auto_skip_reason` 给前端透出"已自动跳过"原因，避免用户看到"运行中"
/// 但桌面不刷新而困惑；显式 paused 由 pauseReason 透出，不重复写。
fn should_skip() -> bool {
    let cfg = config::read_config();
    if !cfg.enabled {
        state::write(|s| s.auto_skip_reason = None);
        return true;
    }

    let s = state::snapshot();
    if s.paused {
        state::write(|s| s.auto_skip_reason = None);
        return true;
    }

    if lock::is_locked() {
        state::write(|s| s.auto_skip_reason = Some("lock"));
        return true;
    }

    if fullscreen::is_fullscreen_busy() {
        state::write(|s| s.auto_skip_reason = Some("fullscreen"));
        return true;
    }

    state::write(|s| s.auto_skip_reason = None);
    false
}

/// 把跳过状态映射到挂件可见性：跳过 → Hidden；正常 → 当前若 Hidden 切回 Peek。
///
/// 显式 paused（manual / boss_key）不在这里处理：manual 由用户自行决定是否
/// 隐藏挂件（默认保留 peek 条让其知道工具仍在）；boss_key 由 boss_key.rs 自己 set Hidden。
fn sync_widget_visibility(app: &AppHandle, skip: bool) {
    if !widget::is_open(app) {
        return;
    }
    let cur = widget::snapshot_state();

    let s = state::snapshot();
    // 显式暂停 (manual/boss_key) 不动挂件可见性
    if s.paused
        && !matches!(
            s.pause_reason,
            Some(state::PauseReason::Lock) | Some(state::PauseReason::Fullscreen)
        )
    {
        return;
    }

    if skip {
        if cur != widget::VisualState::Hidden {
            let _ = widget::set_state(app, widget::VisualState::Hidden);
        }
    } else if cur == widget::VisualState::Hidden {
        let _ = widget::set_state(app, widget::VisualState::Peek);
    }
}

/// 写入失败状态；不再有 burnout / hidden window 重建逻辑。
fn handle_apply_error(err: &str) {
    eprintln!("[widget] scheduled apply failed: {err}");
    state::write(|s| s.last_error = Some(err.to_string()));
}
