//! 事件驱动立刷（plan §3.2 / design §8）
//!
//! ## 设计要点
//!
//! - PM / Todo 的 CRUD 副作用调 [`notify_data_changed`]，发一条触发到内部 channel
//! - 后台 debounce 线程实现 trailing-edge 5s 静默节流：用户连按 3 个完成后
//!   等 5s 静默期再触发一次 [`apply::apply_with_force`]（force=false，仍走 hash 去重）
//! - 仅在 [`start`] 被调用过后才有 sender；调用前的事件被静默丢弃，不报错
//!
//! ## 与 plan §3.2 的差异
//!
//! - plan 用 `tokio::sync::Notify`；本实现走 `std::sync::mpsc::SyncSender`
//!   保持与 scheduler / clipboard_monitor 的 std::thread 架构一致，不引入 tokio
//! - 0 点跨日触发由 scheduler 心跳兜底（间隔 ≤ 60min，最迟 60min 内能反映跨日变化）

#![allow(dead_code)] // hook 由 tools/mod.rs 接入；start 由 main.rs setup 接入

use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Listener};

use crate::tools::wallpaper::{apply, config, fullscreen, lock, state};

/// Trailing-edge debounce 静默期；最后一条事件 5s 内无新事件即触发 apply。
const DEBOUNCE_WINDOW: Duration = Duration::from_secs(5);

/// 内部 channel buffer；防止 PM/Todo 频繁写入时阻塞业务流。
/// 16 足以覆盖正常使用；超出则 try_send 静默丢弃（debounce 后只 fire 一次反正等价）。
const CHANNEL_CAPACITY: usize = 16;

/// 进程内单例 sender；start 后被填充，notify_data_changed 通过它发事件。
static EVENT_TX: OnceLock<mpsc::SyncSender<&'static str>> = OnceLock::new();

/// 启动 debounce 后台线程；幂等，多次调用只生效一次。
///
/// 线程接收事件后用 trailing-edge 策略：每个新事件重置截止时间到 now+5s，
/// 直到没有新事件流入才真正触发 apply。
///
/// 同时启动 midnight 调度线程：每天本地 00:00:00 触发一次 apply（design §8 跨日立刷），
/// 让用户凌晨 0:01 看到的"今日 X/Y" 立即反映新一天的统计。
pub fn start(app: AppHandle) {
    let (tx, rx) = mpsc::sync_channel::<&'static str>(CHANNEL_CAPACITY);
    if EVENT_TX.set(tx).is_err() {
        // 已经启动过，直接返回
        return;
    }

    let app_for_midnight = app.clone();
    std::thread::spawn(move || midnight_loop(app_for_midnight));

    // 监听前端挂件交互（v2 新增）：用户点击 todo checkbox 等动作 → 立即推新数据。
    // 走 debounce 通道避免连点抖动；前端 emit `wallpaper://canvas-action`。
    app.listen("wallpaper://canvas-action", |evt| {
        eprintln!("[wallpaper] canvas-action received: {}", evt.payload());
        apply::invalidate_input_hash();
        notify_data_changed("widget");
    });

    std::thread::spawn(move || {
        loop {
            // 等首条事件（无限阻塞）
            let _first = match rx.recv() {
                Ok(reason) => reason,
                Err(_) => return, // 所有 sender drop（进程退出）
            };

            // 滚动 deadline：每来一条新事件就把 deadline 顺延 DEBOUNCE_WINDOW
            let mut deadline = Instant::now() + DEBOUNCE_WINDOW;
            loop {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                match rx.recv_timeout(deadline - now) {
                    Ok(_) => deadline = Instant::now() + DEBOUNCE_WINDOW,
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }

            // 静默期到，触发 apply（force=false → 内容 hash 去重仍生效）
            if should_skip(&app) {
                continue;
            }
            match apply::apply_with_force(&app, false) {
                Ok(_) => {}
                Err(e) => eprintln!("[wallpaper] event-driven apply failed: {e}"),
            }
        }
    });
}

/// 每天本地 00:00:00 触发一次 apply，让仪表盘"今日"统计立即跟上新一天。
///
/// - 不依赖 tokio：用 chrono 算下次 midnight 偏移 + std::thread::sleep
/// - 跳过判定与 debounce loop 同口径（禁用 / 暂停 / 锁屏 / 全屏期间不执行）
/// - force=true：跨日时其它字段可能未变（hash 命中），但"今日"统计已变，必须真合成
fn midnight_loop(app: AppHandle) {
    use chrono::{Duration as ChronoDuration, Local, NaiveTime, TimeZone};

    loop {
        let now = Local::now();
        let tomorrow = now.date_naive().succ_opt().unwrap_or(now.date_naive());
        let midnight_naive = tomorrow.and_time(NaiveTime::MIN);
        let midnight = match Local.from_local_datetime(&midnight_naive).earliest() {
            Some(dt) => dt,
            None => now + ChronoDuration::hours(24), // 极端 DST 边界回退
        };
        let wait_ms = (midnight - now).num_milliseconds().max(60_000) as u64;

        std::thread::sleep(Duration::from_millis(wait_ms));

        if should_skip(&app) {
            continue;
        }
        if let Err(e) = apply::apply_with_force(&app, true) {
            eprintln!("[wallpaper] midnight apply failed: {e}");
        }
    }
}

/// PM / Todo 副作用末尾调用；reason 用 `'static str`（"pm" / "todo"）便于日志。
///
/// 调用方不感知 channel 满 / 未启动；任何错误都静默丢弃。
pub fn notify_data_changed(reason: &'static str) {
    if let Some(tx) = EVENT_TX.get() {
        let _ = tx.try_send(reason);
    }
}

/// 与 scheduler.should_skip 同口径；事件驱动也需要尊重禁用 / 暂停 / 锁屏 / 全屏。
fn should_skip(_app: &AppHandle) -> bool {
    let cfg = config::read_config();
    if !cfg.enabled {
        return true;
    }
    if state::snapshot().paused {
        return true;
    }
    if lock::is_locked() {
        return true;
    }
    if fullscreen::is_fullscreen_busy() {
        return true;
    }
    false
}
