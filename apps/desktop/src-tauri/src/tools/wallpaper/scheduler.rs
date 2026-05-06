//! 壁纸刷新心跳调度（plan §3.1 / design §8）
//!
//! ## Phase 3.1 范围（最小可用）
//!
//! - 后台线程按 `wallpaper.refresh_interval_min` 心跳触发 [`apply::apply`]
//! - 启动时若 `enabled=true`，先立即 apply 一次（避免用户开启后等 15min 才看到效果）
//! - 跳过条件：未启用 / 暂停态
//! - 连续渲染失败 ≥ [`hidden::BURNOUT_REBUILD_THRESHOLD`] 时销毁 hidden window，
//!   下一轮重建，避免 WebView 内存泄漏 / 黑帧（design §7.5）
//!
//! ## 后续 Phase 3.x（暂不接入）
//!
//! - 空闲降频：`GetLastInputInfo` 检测，5min 无操作降到 60min
//! - 锁屏暂停：`WTSRegisterSessionNotification` 监听
//! - 全屏切净：`SHQueryUserNotificationState`
//! - 事件驱动立刷：PM/Todo 副作用 → trailing-edge debounce 5s
//! - 内容 hash 去重：跳过未变化的合成

#![allow(dead_code)] // Phase 3.x 增量接入

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::AppHandle;

use crate::tools::wallpaper::{apply, config, hidden, lock, state};

/// 进程内仅启动一次的守门员；防止 main.rs 误重复调 [`start`]。
static SCHEDULER_RUNNING: AtomicBool = AtomicBool::new(false);

/// 启动后台心跳线程；幂等。
///
/// 主线程在 `tauri::Builder::setup` 阶段调用一次即可。
pub fn start(app: AppHandle) {
    if SCHEDULER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(move || {
        loop {
            // 1. 跳过判定先行（避免在禁用状态下也走完一次 apply）
            if !should_skip(&app) {
                match apply::apply(&app) {
                    Ok(_) => clear_burnout(),
                    Err(e) => handle_apply_error(&app, &e),
                }
            }

            // 2. 配置可能在运行期改动；每轮重新读
            let interval_min = config::read_config().refresh_interval_min.max(1) as u64;
            std::thread::sleep(Duration::from_secs(interval_min * 60));
        }
    });
}

/// 跳过判定：禁用 / 暂停态 / 锁屏 / 屏保 / 用户切换 都跳过。
///
/// 锁屏判定走 [`lock::is_locked`] 同步轮询，不写状态：用户解锁后下一轮
/// 心跳自动恢复；状态卡片不会出现 "锁屏暂停" 字样（用户锁屏期间也看不到）。
///
/// 为后续 Phase 3.x 留扩展点：空闲、全屏、Spotlight 检测都汇入此函数。
fn should_skip(_app: &AppHandle) -> bool {
    let cfg = config::read_config();
    if !cfg.enabled {
        return true;
    }

    let s = state::snapshot();
    if s.paused {
        return true;
    }

    if lock::is_locked() {
        return true;
    }

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
