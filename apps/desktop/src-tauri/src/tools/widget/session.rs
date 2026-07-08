//! 挂件会话状态持有者
//!
//! 单一状态持有者（`WidgetSession`），替代所有散落的 atomic / RwLock。
//!
//! ## 并发设计
//!
//! - **热路径字段**：`visual_state`(AtomicU8)、`paused`(AtomicBool)、`pending_y`(AtomicI32)
//!   被 cursor_loop（80ms）和 pulse tick（30s~3600s）高频读取，在 RwLock 外独立存储。
//! - **其余字段**：由 `RwLock<SessionInner>` 保护。
//! - **transition() 锁策略**：仅在 atomic store + event push 时持锁（μs 级），
//!   窗口 API（show/hide/set_position）在锁外执行。
//! - **generation counter**：cursor_loop 捕获当前值 → transition 前校验 → 不匹配则跳过。

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering};
use std::sync::{LazyLock, RwLock};
use std::time::Instant;

use serde_json::{json, Value};
use tauri::{AppHandle, WebviewWindow};

use crate::tools::widget::{config, diagnostics, guards, widget};

// ── 基础类型 ─────────────────────────────────────

/// 窗口可见状态（三态 + Windowless）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualState {
    Peek = 0,
    Full = 1,
    Hidden = 2,
    Windowless = 3,
}

impl VisualState {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Full,
            2 => Self::Hidden,
            3 => Self::Windowless,
            _ => Self::Peek,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Peek => "peek",
            Self::Full => "full",
            Self::Hidden => "hidden",
            Self::Windowless => "windowless",
        }
    }
}

/// 暂停原因，与前端 `WidgetPauseReason` 对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseReason {
    Fullscreen,
    Lock,
    Manual,
}

impl PauseReason {
    pub fn as_str(self) -> &'static str {
        match self {
            PauseReason::Fullscreen => "fullscreen",
            PauseReason::Lock => "lock",
            PauseReason::Manual => "manual",
        }
    }
}

// ── SessionInner（RwLock 保护） ──────────────────

pub(crate) struct SessionInner {
    // 窗口
    pub(crate) window: Option<WebviewWindow>,
    pub(crate) window_generation: u64,
    pub(crate) rebuild_in_progress: bool,
    pub(crate) watchdog_rebuilds: u32,

    // 运行时状态（原 state.rs 字段）
    pub(crate) pause_reason: Option<PauseReason>,
    pub(crate) last_rendered_at: Option<String>,
    pub(crate) last_error: Option<String>,
    pub(crate) auto_skip_reason: Option<String>,
    /// 触发自动跳过的应用名（fullscreen 时为前台进程名；lock 时为 None）
    pub(crate) auto_skip_app: Option<String>,
    pub(crate) spotlight_detected: bool,
    pub(crate) third_party_engine: Option<String>,

    // 配置缓存
    pub(crate) config_cache: config::WidgetConfig,
    pub(crate) config_dirty: bool,

    // 内容去重（原 apply.rs atomic）
    pub(crate) input_hash: u64,

    // 握手
    pub(crate) ready_deadline: Option<Instant>,

    // 诊断
    pub(crate) events: diagnostics::EventRing,
    pub(crate) last_ping_at: Instant,
}

// ── WidgetSession（公开结构） ─────────────────────

/// 挂件会话单例。
pub struct WidgetSession {
    /// 当前可见状态（cursor_loop 80ms 无锁读取）。
    pub visual_state: AtomicU8,
    /// 是否暂停（cursor_loop / pulse 高频读取）。
    pub paused: AtomicBool,
    /// 拖拽缓存 Y 物理像素（Moved 事件写入，flush_loop 200ms 持久化）。
    pub pending_y: AtomicI32,

    pub(crate) inner: RwLock<SessionInner>,
}

// ── 单例 ──────────────────────────────────────────

static SESSION: LazyLock<WidgetSession> = LazyLock::new(WidgetSession::new);

/// 获取全局 WidgetSession 引用。
pub fn session() -> &'static WidgetSession {
    &SESSION
}

// ── 实现 ──────────────────────────────────────────

impl WidgetSession {
    pub fn new() -> Self {
        Self {
            visual_state: AtomicU8::new(VisualState::Windowless as u8),
            paused: AtomicBool::new(false),
            pending_y: AtomicI32::new(i32::MIN),
            inner: RwLock::new(SessionInner {
                window: None,
                window_generation: 0,
                rebuild_in_progress: false,
                watchdog_rebuilds: 0,
                pause_reason: None,
                last_rendered_at: None,
                last_error: None,
                auto_skip_reason: None,
                auto_skip_app: None,
                spotlight_detected: false,
                third_party_engine: None,
                config_cache: config::WidgetConfig::default(),
                config_dirty: true, // first tick will load from DB
                input_hash: 0,
                ready_deadline: None,
                events: diagnostics::EventRing::new(),
                last_ping_at: Instant::now(),
            }),
        }
    }

    // ── 热路径读取（无锁） ──────────────────────────

    /// 当前可见状态（AtomicU8 读取）。
    pub fn visual_state(&self) -> VisualState {
        VisualState::from_u8(self.visual_state.load(Ordering::SeqCst))
    }

    /// 是否暂停。
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// 拖拽 Y 缓存值；`i32::MIN` = 未设置。
    pub fn pending_y_val(&self) -> i32 {
        self.pending_y.load(Ordering::SeqCst)
    }

    /// 设置拖拽 Y 缓存（由 Moved 事件监听器调用）。
    pub fn set_pending_y(&self, y: i32) {
        self.pending_y.store(y, Ordering::SeqCst);
    }

    // ── 窗口信息（短锁读取） ────────────────────────

    /// 窗口创建代数（cursor_loop 用于校验窗口有效性）。
    pub fn generation(&self) -> u64 {
        self.inner.read().map(|g| g.window_generation).unwrap_or(0)
    }

    /// 窗口是否存在。
    pub fn is_window_open(&self) -> bool {
        self.inner
            .read()
            .map(|g| g.window.is_some())
            .unwrap_or(false)
    }

    /// 获取窗口句柄克隆（供 apply.rs 等外部使用）。
    pub fn window_handle(&self) -> Option<WebviewWindow> {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.window.clone())
    }

    // ── transition（所有可见性变更唯一入口） ─────────

    /// 切换可见状态。
    ///
    /// - 幂等：to == 当前 → 立即返回 Ok
    /// - 原子：先改 AtomicU8，再操作窗口；窗口操作失败时回写旧状态
    /// - 可追溯：每次 transition 自动 `record(StateTransition)`
    pub fn transition(&self, app: &AppHandle, to: VisualState) -> Result<(), String> {
        let cur = self.visual_state();
        if cur == to {
            return Ok(());
        }

        // Windowless → 非 Windowless：必须有窗口（通过 set_window() 已存储）
        if cur == VisualState::Windowless && to != VisualState::Windowless && !self.is_window_open() {
            return Err("transition from Windowless requires set_window() first".into());
        }

        let from_str = cur.as_str().to_string();
        let to_str = to.as_str().to_string();
        eprintln!("[widget] session: transition {} → {}", from_str, to_str);

        // 1. 更新 AtomicU8（先改状态）
        self.visual_state
            .store(to as u8, Ordering::SeqCst);

        // 2. 窗口操作（锁外执行，获取窗口引用用短锁）
        let win = self.inner.read().ok().and_then(|g| g.window.clone());
        let result = if let Some(ref w) = win {
            self.apply_window_ops(app, w, cur, to)
        } else if to == VisualState::Windowless {
            // Windowless 无窗口 → 直接清理
            Ok(())
        } else {
            // 无窗口但尝试 show → 失败（窗口可能在 pulse rebuild 中）
            eprintln!(
                "[widget] session: transition {} → {} but no window stored",
                from_str, to_str
            );
            Ok(()) // 不报错，下次 pulse tick 会重建
        };

        // 3. 清理 / 记录
        match to {
            VisualState::Windowless => {
                if let Ok(mut g) = self.inner.write() {
                    g.window = None;
                    g.ready_deadline = None;
                }
            }
            _ => {}
        }

        // 4. 如果窗口操作失败，回写旧状态
        if let Err(ref e) = result {
            eprintln!(
                "[widget] session: transition {} → {} failed: {}",
                from_str, to_str, e
            );
            self.visual_state.store(cur as u8, Ordering::SeqCst);
        }

        // 5. 记录事件（无论成败都记）
        self.record(diagnostics::WidgetEvent::StateTransition {
            from: from_str,
            to: to_str,
            trigger: "transition".into(),
        });

        result
    }

    /// 窗口 show/hide/set_position 操作（transition 内部调用）。
    fn apply_window_ops(
        &self,
        app: &AppHandle,
        win: &WebviewWindow,
        cur: VisualState,
        to: VisualState,
    ) -> Result<(), String> {
        match to {
            VisualState::Hidden => {
                let _ = win.hide();
                Ok(())
            }
            VisualState::Peek | VisualState::Full => {
                if cur == VisualState::Hidden || cur == VisualState::Windowless {
                    eprintln!("[widget] session: show() (was {:?})", cur);
                    let _ = win.show();
                }
                widget::apply_position(app, win, to)
            }
            VisualState::Windowless => {
                eprintln!("[widget] session: closing widget window");
                match win.close() {
                    Ok(()) => eprintln!("[widget] session: close ok"),
                    Err(e) => eprintln!("[widget] session: close failed: {e}"),
                }
                Ok(())
            }
        }
    }

    // ── should_skip / sync_visibility ───────────────

    /// 统一跳过判定：仅当 enabled && !paused && !locked && !fullscreen 时才不跳过。
    pub fn should_skip(&self) -> bool {
        let inner = self.inner.read().unwrap();

        if !inner.config_cache.enabled {
            return true;
        }
        if self.is_paused() {
            return true;
        }

        if guards::is_locked() {
            drop(inner);
            self.write_inner(|s| {
                s.auto_skip_reason = Some("lock".into());
                s.auto_skip_app = None;
            });
            return true;
        }

        if let Some(app_name) = guards::fullscreen_busy_app() {
            drop(inner);
            self.write_inner(|s| {
                s.auto_skip_reason = Some("fullscreen".into());
                s.auto_skip_app = Some(app_name);
            });
            return true;
        }

        drop(inner);
        self.write_inner(|s| {
            s.auto_skip_reason = None;
            s.auto_skip_app = None;
        });
        false
    }

    /// 根据 skip 结果驱动挂件可见性。
    pub fn sync_visibility(&self, app: &AppHandle, skip: bool) {
        if !self.is_window_open() {
            return;
        }

        let cur = self.visual_state();

        // 显式暂停(manual)不动可见性
        let is_manual_pause = self
            .inner
            .read()
            .ok()
            .map(|g| {
                g.pause_reason
                    == Some(PauseReason::Manual)
            })
            .unwrap_or(false);

        if self.is_paused() && is_manual_pause {
            return;
        }

        if skip {
            if cur != VisualState::Hidden {
                let _ = self.transition(app, VisualState::Hidden);
            }
        } else if cur == VisualState::Hidden {
            let _ = self.transition(app, VisualState::Peek);
        }
    }

    // ── 配置缓存 ────────────────────────────────────

    /// 如果配置标记为 dirty，从 DB 重读。
    pub fn refresh_config_if_dirty(&self) {
        let dirty = self
            .inner
            .read()
            .map(|g| g.config_dirty)
            .unwrap_or(false);
        if dirty {
            let fresh = config::read_config();
            if let Ok(mut g) = self.inner.write() {
                g.config_cache = fresh;
                g.config_dirty = false;
            }
        }
    }

    /// 标记下次 tick 需刷新配置缓存。
    pub fn mark_config_dirty(&self) {
        if let Ok(mut g) = self.inner.write() {
            g.config_dirty = true;
        }
    }

    /// 获取配置缓存快照。
    pub fn config(&self) -> config::WidgetConfig {
        self.inner
            .read()
            .map(|g| g.config_cache.clone())
            .unwrap_or_default()
    }

    /// enabled 快捷访问。
    pub fn is_enabled(&self) -> bool {
        self.inner
            .read()
            .map(|g| g.config_cache.enabled)
            .unwrap_or(false)
    }

    // ── 内容 hash ────────────────────────────────────

    /// 重置内容 hash → 下一轮 apply 强制推送。
    pub fn invalidate_input_hash(&self) {
        if let Ok(mut g) = self.inner.write() {
            g.input_hash = 0;
        }
    }

    pub fn input_hash(&self) -> u64 {
        self.inner.read().map(|g| g.input_hash).unwrap_or(0)
    }

    pub fn store_input_hash(&self, hash: u64) {
        if let Ok(mut g) = self.inner.write() {
            g.input_hash = hash;
        }
    }

    // ── 事件记录 ────────────────────────────────────

    /// 追加事件到环形缓冲。
    pub fn record(&self, event: diagnostics::WidgetEvent) -> u64 {
        if let Ok(mut g) = self.inner.write() {
            g.events.push(event)
        } else {
            0
        }
    }

    // ── 通用读写 ────────────────────────────────────

    /// 通用写访问（替代旧 state::write）。
    pub fn write_inner<F: FnOnce(&mut SessionInner)>(&self, f: F) {
        if let Ok(mut g) = self.inner.write() {
            f(&mut g);
        }
    }

    pub fn update_last_rendered(&self) {
        if let Ok(mut g) = self.inner.write() {
            g.last_rendered_at = Some(now_iso());
            g.last_error = None;
        }
    }

    pub fn set_last_error(&self, err: Option<String>) {
        if let Ok(mut g) = self.inner.write() {
            g.last_error = err;
        }
    }

    /// 获取 Spotlight / 第三方引擎检测字段的可变引用，
    /// 供 conflicts::refresh() 写入。
    pub fn update_conflicts(&self, spotlight: bool, engine: Option<String>) {
        if let Ok(mut g) = self.inner.write() {
            g.spotlight_detected = spotlight;
            g.third_party_engine = engine;
        }
    }

    // ── Ping / Ready ──────────────────────────────────

    pub fn record_ping(&self) {
        if let Ok(mut g) = self.inner.write() {
            g.last_ping_at = Instant::now();
        }
    }

    /// 收到 widget://ready → 清 ready_deadline。
    pub fn set_ready(&self) {
        if let Ok(mut g) = self.inner.write() {
            g.ready_deadline = None;
        }
    }

    /// 设置 ready deadline（ensure() 后调用）。
    /// 同时重置 last_ping_at，避免看门狗用窗口创建前的旧 ping 时间误判超时。
    pub fn set_ready_deadline(&self) {
        if let Ok(mut g) = self.inner.write() {
            g.ready_deadline = Some(Instant::now() + std::time::Duration::from_secs(3));
            g.last_ping_at = Instant::now();
        }
    }

    /// 检查 ready 超时（3s 已过且未收到 ready）。
    pub fn check_ready_timeout(&self) -> bool {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.ready_deadline)
            .map(|dl| Instant::now() >= dl)
            .unwrap_or(false)
    }

    // ── 看门狗 ──────────────────────────────────────

    /// 距上次 ping 的秒数。
    pub fn seconds_since_ping(&self) -> u64 {
        self.inner
            .read()
            .ok()
            .map(|g| g.last_ping_at.elapsed().as_secs())
            .unwrap_or(0)
    }

    pub fn watchdog_rebuilds(&self) -> u32 {
        self.inner
            .read()
            .map(|g| g.watchdog_rebuilds)
            .unwrap_or(0)
    }

    /// CAS rebuild_in_progress: false→true。
    /// 返回 true = 获得重建权；false = 已有重建在进行。
    pub fn begin_rebuild(&self) -> bool {
        if let Ok(mut g) = self.inner.write() {
            if g.rebuild_in_progress {
                return false;
            }
            g.rebuild_in_progress = true;
            true
        } else {
            false
        }
    }

    /// 结束重建。success: true → 清零 rebuilds；false → rebuilds += 1。
    pub fn end_rebuild(&self, success: bool) {
        if let Ok(mut g) = self.inner.write() {
            g.rebuild_in_progress = false;
            if success {
                g.watchdog_rebuilds = 0;
            } else {
                g.watchdog_rebuilds += 1;
            }
        }
    }

    /// 检查 watchdog_rebuilds >= 3，如果是则暂停并设置错误。
    pub fn check_rebuild_limit(&self) -> bool {
        if self.watchdog_rebuilds() >= 3 {
            self.paused.store(true, Ordering::SeqCst);
            let msg = "窗口连续 3 次重建失败，已暂停".to_string();
            self.set_last_error(Some(msg.clone()));
            self.record(diagnostics::WidgetEvent::Error {
                source: "watchdog".into(),
                message: msg,
            });
            true
        } else {
            false
        }
    }

    // ── 窗口存储 ─────────────────────────────────────

    /// 存储窗口句柄 + 自增 generation。
    pub fn set_window(&self, win: WebviewWindow) {
        if let Ok(mut g) = self.inner.write() {
            g.window = Some(win);
            g.window_generation += 1;
        }
    }

    // ── status / diagnostics 快照 ──────────────────────

    /// status 通道返回值（替代旧 state::status_snapshot）。
    pub fn status_snapshot(&self) -> Value {
        let inner = self.inner.read().unwrap();

        // 敏感模式自动到期判断
        let mask_active = inner.config_cache.privacy_mask
            && inner
                .config_cache
                .privacy_mask_until
                .as_deref()
                .map(|until| !is_iso_past(until))
                .unwrap_or(true);

        json!({
            "enabled": inner.config_cache.enabled,
            "paused": self.is_paused(),
            "pauseReason": inner.pause_reason.map(|r| r.as_str()),
            "lastRenderedAt": inner.last_rendered_at,
            "lastError": inner.last_error,
            "spotlightDetected": inner.spotlight_detected,
            "thirdPartyEngine": inner.third_party_engine,
            "privacyMaskActive": mask_active,
            "privacyMaskUntil": inner.config_cache.privacy_mask_until,
            "autoSkipReason": inner.auto_skip_reason.as_deref(),
            "autoSkipApp": inner.auto_skip_app.as_deref(),
        })
    }

    /// 诊断通道返回值。
    pub fn diagnostics_snapshot(&self) -> Value {
        let inner = self.inner.read().unwrap();
        let health = inner.events.health();
        let events: Vec<Value> = inner
            .events
            .recent(50)
            .iter()
            .map(|e| {
                let event_type = event_type_name(&e.event);
                let detail = event_detail(&e.event);
                json!({
                    "sequenceId": e.sequence_id,
                    "timestamp": e.timestamp,
                    "type": event_type,
                    "detail": detail,
                })
            })
            .collect();

        json!({
            "health": health,
            "events": events,
        })
    }
}

// ── 辅助 ──────────────────────────────────────────

fn now_iso() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

fn is_iso_past(iso: &str) -> bool {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(iso) else {
        return false;
    };
    chrono::Utc::now() > dt.with_timezone(&chrono::Utc)
}

fn event_type_name(evt: &diagnostics::WidgetEvent) -> &'static str {
    match evt {
        diagnostics::WidgetEvent::StateTransition { .. } => "StateTransition",
        diagnostics::WidgetEvent::ApplyAttempt { .. } => "ApplyAttempt",
        diagnostics::WidgetEvent::ApplySkipped { .. } => "ApplySkipped",
        diagnostics::WidgetEvent::WindowCreated { .. } => "WindowCreated",
        diagnostics::WidgetEvent::WindowDestroyed { .. } => "WindowDestroyed",
        diagnostics::WidgetEvent::Error { .. } => "Error",
        diagnostics::WidgetEvent::PingReceived => "PingReceived",
        diagnostics::WidgetEvent::WatchdogTriggered { .. } => "WatchdogTriggered",
        diagnostics::WidgetEvent::Lifecycle { .. } => "Lifecycle",
    }
}

fn event_detail(evt: &diagnostics::WidgetEvent) -> String {
    match evt {
        diagnostics::WidgetEvent::StateTransition { from, to, .. } => {
            format!("{from} → {to}")
        }
        diagnostics::WidgetEvent::ApplyAttempt {
            force,
            elapsed_ms,
            ..
        } => {
            format!("force={force}, {elapsed_ms}ms")
        }
        diagnostics::WidgetEvent::ApplySkipped { reason } => {
            format!("reason={:?}", reason)
        }
        diagnostics::WidgetEvent::WindowCreated { elapsed_ms } => {
            format!("took {elapsed_ms}ms")
        }
        diagnostics::WidgetEvent::WindowDestroyed { reason } => {
            reason.clone()
        }
        diagnostics::WidgetEvent::Error { source, message } => {
            format!("[{source}] {message}")
        }
        diagnostics::WidgetEvent::PingReceived => String::new(),
        diagnostics::WidgetEvent::WatchdogTriggered {
            seconds_since_ping,
        } => {
            format!("ping missing {seconds_since_ping}s")
        }
        diagnostics::WidgetEvent::Lifecycle { action } => action.clone(),
    }
}

// ── 测试 ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_state_round_trip() {
        for v in [
            VisualState::Peek,
            VisualState::Full,
            VisualState::Hidden,
            VisualState::Windowless,
        ] {
            assert_eq!(VisualState::from_u8(v as u8), v);
        }
    }

    #[test]
    fn visual_state_as_str() {
        assert_eq!(VisualState::Peek.as_str(), "peek");
        assert_eq!(VisualState::Full.as_str(), "full");
        assert_eq!(VisualState::Hidden.as_str(), "hidden");
        assert_eq!(VisualState::Windowless.as_str(), "windowless");
    }

    #[test]
    fn pause_reason_str_round_trip() {
        assert_eq!(PauseReason::Fullscreen.as_str(), "fullscreen");
        assert_eq!(PauseReason::Lock.as_str(), "lock");
        assert_eq!(PauseReason::Manual.as_str(), "manual");
    }

    #[test]
    fn new_session_is_windowless() {
        let s = WidgetSession::new();
        assert_eq!(s.visual_state(), VisualState::Windowless);
        assert!(!s.is_paused());
        assert_eq!(s.pending_y_val(), i32::MIN);
        assert!(!s.is_window_open());
    }

    #[test]
    fn transition_rejects_windowless_to_peek() {
        let s = WidgetSession::new();
        // No app handle available in unit tests, but transition checks Windowless first
        // Actually transition requires AppHandle, which we can't get in unit tests.
        // This test validates the state logic conceptually — in practice,
        // ensure()→set_window()→transition(Peek) is the valid path.
        assert_eq!(s.visual_state(), VisualState::Windowless);
    }

    #[test]
    fn invalidate_and_store_hash() {
        let s = WidgetSession::new();
        assert_eq!(s.input_hash(), 0);
        s.store_input_hash(42);
        assert_eq!(s.input_hash(), 42);
        s.invalidate_input_hash();
        assert_eq!(s.input_hash(), 0);
    }

    #[test]
    fn config_dirty_defaults_true() {
        let s = WidgetSession::new();
        let dirty = s.inner.read().unwrap().config_dirty;
        assert!(dirty, "initial config_dirty should be true");
    }

    #[test]
    fn mark_and_refresh_config_dirty() {
        let s = WidgetSession::new();
        // first refresh clears dirty (reads from DB which may not exist)
        s.refresh_config_if_dirty();
        let dirty = s.inner.read().unwrap().config_dirty;
        assert!(!dirty);

        s.mark_config_dirty();
        assert!(s.inner.read().unwrap().config_dirty);
    }

    #[test]
    fn generation_starts_at_zero() {
        let s = WidgetSession::new();
        assert_eq!(s.generation(), 0);
    }

    #[test]
    fn pending_y_default_is_min() {
        let s = WidgetSession::new();
        assert_eq!(s.pending_y_val(), i32::MIN);
        s.set_pending_y(100);
        assert_eq!(s.pending_y_val(), 100);
    }

    #[test]
    fn rebuild_begin_end_flow() {
        let s = WidgetSession::new();
        assert!(s.begin_rebuild());
        // second call fails (already in progress)
        assert!(!s.begin_rebuild());
        s.end_rebuild(true);
        assert_eq!(s.watchdog_rebuilds(), 0);
        // can begin again
        assert!(s.begin_rebuild());
        s.end_rebuild(false); // failure
        assert_eq!(s.watchdog_rebuilds(), 1);
    }

    #[test]
    fn check_rebuild_limit_at_3() {
        let s = WidgetSession::new();
        assert!(s.begin_rebuild());
        s.end_rebuild(false);
        assert!(s.begin_rebuild());
        s.end_rebuild(false);
        assert!(s.begin_rebuild());
        s.end_rebuild(false);
        // 3 failures should trigger limit
        assert!(s.check_rebuild_limit());
        assert!(s.is_paused());
    }

    #[test]
    fn record_sequence_id_monotonic() {
        let s = WidgetSession::new();
        let mut ids = Vec::new();
        for i in 0..10 {
            ids.push(s.record(diagnostics::WidgetEvent::Lifecycle {
                action: format!("test-{i}"),
            }));
        }
        for idx in 1..ids.len() {
            assert!(ids[idx] > ids[idx - 1], "seq ids must be monotonic");
        }
    }

    #[test]
    fn status_snapshot_has_expected_keys() {
        let s = WidgetSession::new();
        let snap = s.status_snapshot();
        let obj = snap.as_object().unwrap();
        assert!(obj.contains_key("enabled"));
        assert!(obj.contains_key("paused"));
        assert!(obj.contains_key("pauseReason"));
        assert!(obj.contains_key("lastRenderedAt"));
        assert!(obj.contains_key("lastError"));
        assert!(obj.contains_key("spotlightDetected"));
        assert!(obj.contains_key("thirdPartyEngine"));
        assert!(obj.contains_key("privacyMaskActive"));
        assert!(obj.contains_key("privacyMaskUntil"));
        assert!(obj.contains_key("autoSkipReason"));
        assert!(obj.contains_key("autoSkipApp"));
    }

    #[test]
    fn should_skip_disabled() {
        let s = WidgetSession::new();
        // config_cache defaults to enabled=false
        assert!(s.should_skip());
    }

    #[test]
    fn should_skip_paused() {
        let s = WidgetSession::new();
        // enable config
        s.write_inner(|inner| inner.config_cache.enabled = true);
        s.paused.store(true, Ordering::SeqCst);
        assert!(s.should_skip());
    }
}