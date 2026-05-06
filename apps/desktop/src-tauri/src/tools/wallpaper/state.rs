//! 进程内壁纸运行时状态
//!
//! 阶段 0：仅提供 status 快照所需的最小字段（基于配置 + 占位）。
//! 阶段 1+：扩展 base 图缓存、recently_completed、burnout 计数等。

#![allow(dead_code)] // Phase 0 骨架：PauseReason 变体与 burnout/write 由 Phase 3 接入

use std::sync::{LazyLock, RwLock};

use serde_json::{json, Value};

use crate::tools::wallpaper::config::{self, KEY_ORIGINAL_PATH};
use crate::tools::helpers::db_conn;

/// 暂停原因，与前端 `WallpaperPauseReason` 对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseReason {
    BossKey,
    Fullscreen,
    Lock,
    Manual,
}

impl PauseReason {
    pub fn as_str(self) -> &'static str {
        match self {
            PauseReason::BossKey => "boss_key",
            PauseReason::Fullscreen => "fullscreen",
            PauseReason::Lock => "lock",
            PauseReason::Manual => "manual",
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct WallpaperState {
    pub paused: bool,
    pub pause_reason: Option<PauseReason>,
    pub last_rendered_at: Option<String>, // ISO 时间字符串（避免 Instant 跨边界序列化）
    pub last_rendered_path: Option<String>,
    pub last_error: Option<String>,
    pub spotlight_detected: bool,
    pub third_party_engine: Option<String>,
    pub burnout: u8,
}

static STATE: LazyLock<RwLock<WallpaperState>> = LazyLock::new(|| RwLock::new(WallpaperState::default()));

/// 通用读访问（拿快照副本）
pub fn snapshot() -> WallpaperState {
    STATE.read().map(|g| g.clone()).unwrap_or_default()
}

/// 通用写访问（毒锁时静默回退）
pub fn write<F: FnOnce(&mut WallpaperState)>(f: F) {
    if let Ok(mut g) = STATE.write() {
        f(&mut g);
    }
}

/// status 通道返回值；前端 `WallpaperStatus` 直接消费。
pub fn status_snapshot() -> Value {
    let cfg = config::read_config();
    let st = snapshot();

    let original_path = db_conn()
        .ok()
        .and_then(|conn| config::read_string(&conn, KEY_ORIGINAL_PATH))
        .filter(|s| !s.is_empty());

    json!({
        "enabled": cfg.enabled,
        "paused": st.paused,
        "pauseReason": st.pause_reason.map(|r| r.as_str()),
        "originalPath": original_path,
        "lastRenderedAt": st.last_rendered_at,
        "lastRenderedPath": st.last_rendered_path,
        "lastError": st.last_error,
        "spotlightDetected": st.spotlight_detected,
        "thirdPartyEngine": st.third_party_engine,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_reason_str_round_trip() {
        assert_eq!(PauseReason::BossKey.as_str(), "boss_key");
        assert_eq!(PauseReason::Fullscreen.as_str(), "fullscreen");
        assert_eq!(PauseReason::Lock.as_str(), "lock");
        assert_eq!(PauseReason::Manual.as_str(), "manual");
    }

    #[test]
    fn default_state_is_empty() {
        let s = WallpaperState::default();
        assert!(!s.paused);
        assert!(s.pause_reason.is_none());
        assert!(s.last_rendered_at.is_none());
        assert_eq!(s.burnout, 0);
    }
}
