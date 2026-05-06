//! 全屏切净检测（plan §3.3 / design §9）
//!
//! 通过 [`SHQueryUserNotificationState`] 取系统当前通知态：
//! - `QUNS_BUSY` / `QUNS_RUNNING_D3D_FULL_SCREEN` / `QUNS_PRESENTATION_MODE` → 演示 / 录屏 / 全屏游戏
//! - 其他态（`QUNS_APP` / `QUNS_ACCEPTS_NOTIFICATIONS` / 等）→ 正常工作
//!
//! 相比设计文档完整版的「兜底全屏窗口检测 + 进程黑名单匹配」，此最小子集
//! 仅依赖系统通知态判定。Chrome / VLC 等日常应用全屏播放由系统 API 兜底，
//! 已避免 design §9 强调的「窗口化使用时长期误切净」。
//!
//! 黑名单匹配将在后续 Phase 接入：要 `GetForegroundWindow` +
//! `QueryFullProcessImageName`，整体代码量偏大，暂留扩展位。

#![allow(dead_code)] // is_fullscreen_busy 由 scheduler::should_skip 调用

#[cfg(windows)]
pub use imp::is_fullscreen_busy;

#[cfg(not(windows))]
pub fn is_fullscreen_busy() -> bool {
    false
}

#[cfg(windows)]
mod imp {
    use windows::Win32::UI::Shell::{
        SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
        QUNS_RUNNING_D3D_FULL_SCREEN,
    };

    /// 系统是否处于「不该被打扰」的态：演示 / 录屏 / 全屏 D3D 游戏。
    ///
    /// 任何调用失败 → 返回 false，避免误切净；与 [`crate::tools::wallpaper::lock`]
    /// 的失败-回退策略一致。
    pub fn is_fullscreen_busy() -> bool {
        unsafe {
            match SHQueryUserNotificationState() {
                Ok(state) => matches!(
                    state,
                    QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE
                ),
                Err(_) => false,
            }
        }
    }
}
