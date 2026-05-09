//! 进程内挂件运行时状态
//!
//! 改造后只保留：暂停状态机、最近一次推送时间、错误信息、第三方引擎冲突标记、
//! 老板键注册结果、调度自动跳过原因。
//!
//! 旧 PNG 链路相关字段（last_rendered_path、burnout 计数、BaseCacheEntry 缓存）
//! 已删除——挂件不再合成图，没有"渲染失败连续 N 次重建 hidden window"的概念。

#![allow(dead_code)]

use std::sync::{LazyLock, RwLock};

use serde_json::{json, Value};

use crate::tools::wallpaper::config;

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
    /// 最近一次推送 dashboard-data 给挂件的本地 ISO 时间。
    pub last_rendered_at: Option<String>,
    pub last_error: Option<String>,
    pub spotlight_detected: bool,
    pub third_party_engine: Option<String>,
    /// design §9：老板键注册失败时的提示文案；setup 时写入，前端面板透出
    pub boss_key_error: Option<String>,
    /// 调度上轮 should_skip 命中的原因（lock / fullscreen）；
    /// 与 `paused` 互斥透出：未显式暂停时由此字段反映"自动跳过"。None = 未跳过
    pub auto_skip_reason: Option<&'static str>,
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

    // 敏感模式自动到期：privacy_mask_until 已过 → 视为关
    let mask_active = cfg.privacy_mask
        && cfg
            .privacy_mask_until
            .as_deref()
            .map(|until| !is_iso_past(until))
            .unwrap_or(true);

    json!({
        "enabled": cfg.enabled,
        "paused": st.paused,
        "pauseReason": st.pause_reason.map(|r| r.as_str()),
        "lastRenderedAt": st.last_rendered_at,
        "lastError": st.last_error,
        "spotlightDetected": st.spotlight_detected,
        "thirdPartyEngine": st.third_party_engine,
        "bossKeyError": st.boss_key_error,
        "privacyMaskActive": mask_active,
        "privacyMaskUntil": cfg.privacy_mask_until,
        "autoSkipReason": st.auto_skip_reason,
    })
}

/// ISO 8601 时间字符串是否已过当前时刻；解析失败 → false（保守起见视为未过期）。
fn is_iso_past(iso: &str) -> bool {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(iso) else { return false; };
    chrono::Utc::now() > dt.with_timezone(&chrono::Utc)
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
    }
}
