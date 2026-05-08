//! 壁纸刷新心跳调度（plan §3.1 / design §8）
//!
//! - 后台线程按 `wallpaper.refresh_interval_min` 心跳触发 [`apply::apply_with_force`]
//! - 启动时若 `enabled=true`，先立即 apply 一次（避免用户开启后等 15min 才看到效果）
//! - 跳过条件：未启用 / 暂停态 / 锁屏 / 全屏切净（lock + fullscreen 模块）
//! - 空闲降频：5min 无输入降到 60min sleep；用户回来立即 break sleep
//! - 连续渲染失败 ≥ [`hidden::BURNOUT_REBUILD_THRESHOLD`] 时销毁 hidden window，
//!   下一轮重建，避免 WebView 内存泄漏 / 黑帧（design §7.5）
//!
//! ## 后续 Phase 3.x（暂不接入）
//!
//! - 全屏黑名单进程匹配（GetForegroundWindow + QueryFullProcessImageName）
//! - Spotlight 检测（design §9 第三方引擎兜底）

#![allow(dead_code)] // Phase 3.x 增量接入

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::AppHandle;

use crate::tools::wallpaper::{apply, config, conflicts, fullscreen, hidden, idle, lock, state};

/// 5min 无输入即视为空闲，sleep 间隔降为 [`IDLE_INTERVAL_SECS`]。
const IDLE_THRESHOLD_SECS: u32 = 300;
/// 空闲降频后的固定间隔（60min），覆盖用户离开期间。
const IDLE_INTERVAL_SECS: u64 = 60 * 60;
/// 用户「刚回来」判定阈值：本周期 < 30s 输入间隔即视为活跃。
const ACTIVE_THRESHOLD_SECS: u32 = 30;
/// 分块睡眠粒度；越小响应越快，越大 CPU 越省。30s 在两者间取折中。
const SLEEP_CHUNK_SECS: u64 = 30;

/// 进程内仅启动一次的守门员；防止 main.rs 误重复调 [`start`]。
static SCHEDULER_RUNNING: AtomicBool = AtomicBool::new(false);

/// 启动后台心跳线程；幂等。
///
/// 主线程在 `tauri::Builder::setup` 阶段调用一次即可。
pub fn start(app: AppHandle) {
    if SCHEDULER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    // 启动时立即探测一次 Spotlight / 第三方引擎（design §13.4）
    conflicts::refresh();

    std::thread::spawn(move || {
        let mut last_conflict_check = std::time::Instant::now();
        loop {
            // 每 10min 重新探测一次冲突
            if last_conflict_check.elapsed() >= std::time::Duration::from_secs(600) {
                conflicts::refresh();
                last_conflict_check = std::time::Instant::now();
            }
            // 1. 跳过判定先行（避免在禁用状态下也走完一次 apply）
            if !should_skip(&app) {
                // 心跳走 force=false：相同输入的合成结果会被 hash 去重跳过
                match apply::apply_with_force(&app, false) {
                    Ok(_) => clear_burnout(),
                    Err(e) => handle_apply_error(&app, &e),
                }
            }

            // 2. 计算本轮 sleep 目标：空闲态 60min，活跃态走配置间隔
            let cfg_interval_min = config::read_config().refresh_interval_min.max(1) as u64;
            let target_secs = if idle::seconds_idle() >= IDLE_THRESHOLD_SECS {
                IDLE_INTERVAL_SECS
            } else {
                cfg_interval_min * 60
            };

            // 3. 分块睡眠 + 空闲恢复立刷探测
            sleep_with_idle_check(target_secs);
        }
    });
}

/// 分块 sleep；进入时若已空闲则记录，期间检测「刚回来」立即 break。
///
/// plan §3.1 v0.5：上一周期 idle ≥ 5min、本周期 < 30s → 用户刚回来，
/// 不必等 sleep 走完，立刻进入下一轮 apply 让用户看到最新数据。
fn sleep_with_idle_check(target_secs: u64) {
    let was_idle_at_start = idle::seconds_idle() >= IDLE_THRESHOLD_SECS;
    let mut elapsed = 0u64;

    while elapsed < target_secs {
        let chunk = (target_secs - elapsed).min(SLEEP_CHUNK_SECS);
        std::thread::sleep(Duration::from_secs(chunk));
        elapsed += chunk;

        // 仅在「刚才空闲、现在活跃」的边沿触发；连续活跃不重复 break
        if was_idle_at_start && idle::seconds_idle() < ACTIVE_THRESHOLD_SECS {
            return;
        }
    }
}

/// 跳过判定：禁用 / 暂停态 / 锁屏 / 屏保 / 全屏切净 都跳过。
///
/// 不写 state.paused：用户锁屏 / 全屏期间也看不到面板，恢复后下一轮自动接上。
/// 但写 state.auto_skip_reason 让前端状态卡片显示"⏸ 已自动跳过：锁屏/全屏"，
/// 避免用户看到"运行中"但桌面不刷新而困惑。
fn should_skip(_app: &AppHandle) -> bool {
    let cfg = config::read_config();
    if !cfg.enabled {
        // 未启用不属于"自动跳过"，不写 reason 避免与"未启用"重复透出
        state::write(|s| s.auto_skip_reason = None);
        return true;
    }

    let s = state::snapshot();
    if s.paused {
        // 显式暂停由 pauseReason 透出，不再叠加 auto_skip_reason
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

    // 路径正常 → 清掉上轮跳过原因
    state::write(|s| s.auto_skip_reason = None);
    false
}

/// 写入失败状态 + burnout 累计；超阈值触发 hidden window 重建。
fn handle_apply_error(app: &AppHandle, err: &str) {
    eprintln!("[wallpaper] scheduled apply failed: {err}");

    // burnout 累加；写完后查 snapshot，避免读写竞态
    state::write(|s| {
        s.last_error = Some(err.to_string());
        s.burnout = s.burnout.saturating_add(1);
    });

    let burnout = state::snapshot().burnout as u32;
    if burnout >= hidden::BURNOUT_REBUILD_THRESHOLD {
        if let Err(e) = hidden::rebuild_on_burnout(app) {
            eprintln!("[wallpaper] burnout rebuild failed: {e}");
        }
        state::write(|s| s.burnout = 0);
    }
}

/// 成功路径：apply 内部已清 burnout/last_error；此处保留为扩展钩子。
fn clear_burnout() {
    // apply::apply 已在成功分支写过 last_error=None / burnout=0；
    // 留空以便未来增加调度层成功统计（last_scheduled_at 等）
}
