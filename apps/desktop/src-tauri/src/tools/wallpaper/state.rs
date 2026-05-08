//! 进程内壁纸运行时状态
//!
//! 阶段 0：提供 status 快照所需的最小字段（基于配置 + 占位）。
//! 阶段 1.3：新增 base 图缓存（按 monitor_id 隔离 + mtime 失效）。
//! 阶段 1+：扩展 burnout 计数等。

#![allow(dead_code)] // Phase 0 骨架：PauseReason 变体与 burnout/write 由 Phase 3 接入

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::SystemTime;

use image::DynamicImage;
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
    /// design §9：老板键注册失败时的提示文案；setup 时写入，前端面板透出
    pub boss_key_error: Option<String>,
    /// 调度上轮 should_skip 命中的原因（lock / fullscreen）；
    /// 与 `paused` 互斥透出：未显式暂停时由此字段反映"自动跳过"。None = 未跳过
    pub auto_skip_reason: Option<&'static str>,
}

static STATE: LazyLock<RwLock<WallpaperState>> = LazyLock::new(|| RwLock::new(WallpaperState::default()));

// ── base 图缓存（plan §1.3） ─────────────────────────
//
// 与 WallpaperState 分离，避免 RwLock<...> 内含大对象引发的 clone 成本。
// 阶段 1 仅主屏 1 个 entry；阶段 3 多屏时按 monitor_id 隔离。

/// base 图缓存条目；保存原图 Arc 引用 + 文件元信息用于 mtime 失效。
#[derive(Debug, Clone)]
pub struct BaseCacheEntry {
    pub path: PathBuf,
    pub mtime: SystemTime,
    pub image: Arc<DynamicImage>,
}

static BASE_CACHE: LazyLock<RwLock<HashMap<String, BaseCacheEntry>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// 读取指定 monitor 的 base 缓存（存在则克隆 entry，含 Arc 引用计数 +1）。
pub fn read_base_cache(monitor_id: &str) -> Option<BaseCacheEntry> {
    BASE_CACHE.read().ok()?.get(monitor_id).cloned()
}

/// 覆盖写入指定 monitor 的 base 缓存。毒锁时静默回退。
pub fn write_base_cache(monitor_id: &str, entry: BaseCacheEntry) {
    if let Ok(mut g) = BASE_CACHE.write() {
        g.insert(monitor_id.to_string(), entry);
    }
}

/// 清空所有 monitor 的 base 缓存（用户手改壁纸触发 invalidate 时使用）。
pub fn clear_base_cache() {
    if let Ok(mut g) = BASE_CACHE.write() {
        g.clear();
    }
}

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
        "originalPath": original_path,
        "lastRenderedAt": st.last_rendered_at,
        "lastRenderedPath": st.last_rendered_path,
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
        assert_eq!(s.burnout, 0);
    }

    #[test]
    fn base_cache_round_trip() {
        clear_base_cache();
        let img = Arc::new(DynamicImage::new_rgba8(2, 2));
        write_base_cache(
            "primary",
            BaseCacheEntry {
                path: PathBuf::from("/tmp/x.png"),
                mtime: SystemTime::UNIX_EPOCH,
                image: img.clone(),
            },
        );
        let got = read_base_cache("primary").expect("entry exists");
        assert_eq!(got.path, PathBuf::from("/tmp/x.png"));
        assert_eq!(got.mtime, SystemTime::UNIX_EPOCH);
        assert_eq!(Arc::strong_count(&got.image), 3); // map + got + img
        clear_base_cache();
        assert!(read_base_cache("primary").is_none());
    }
}
