//! 挂件诊断事件系统
//!
//! 提供结构化 `WidgetEvent` 枚举（替代散落 eprintln!）、
//! `EventRing` 环形缓冲（最大 50 条）+ 每日计数器。
//!
//! 本模块是叶模块 — 不导入任何其他 widget 模块，避免循环依赖。

use std::collections::VecDeque;

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ── 事件类型 ─────────────────────────────────────

/// 挂件运行时事件（9 变体），替代所有 eprintln! 日志。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WidgetEvent {
    StateTransition {
        from: String,
        to: String,
        trigger: String,
    },
    ApplyAttempt {
        force: bool,
        result: ApplyResult,
        elapsed_ms: u64,
    },
    ApplySkipped {
        reason: SkipReason,
    },
    WindowCreated {
        elapsed_ms: u64,
    },
    WindowDestroyed {
        reason: String,
    },
    Error {
        source: String,
        message: String,
    },
    PingReceived,
    WatchdogTriggered {
        seconds_since_ping: u64,
    },
    Lifecycle {
        action: String, // "enable" | "disable" | "pause" | "resume"
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApplyResult {
    Ok { privacy_mask: bool },
    Skipped { reason: String },
    Failed { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    Disabled,
    Paused,
    Locked,
    Fullscreen,
    NoChange,
}

// ── 环形缓冲 ─────────────────────────────────────

const MAX_EVENTS: usize = 50;

/// 带时间戳的事件条目（存储在环形缓冲中）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    pub sequence_id: u64,
    pub timestamp: String,
    #[serde(flatten)]
    pub event: WidgetEvent,
}

/// 环形缓冲 + 每日计数器。
pub struct EventRing {
    events: VecDeque<EventEntry>,
    next_sequence_id: u64,
    // 每日计数器
    today_skip_count: u32,
    today_watchdog_count: u32,
    today_rebuild_count: u32,
    today_date: String,
}

impl EventRing {
    pub fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(MAX_EVENTS),
            next_sequence_id: 0,
            today_skip_count: 0,
            today_watchdog_count: 0,
            today_rebuild_count: 0,
            today_date: today_str(),
        }
    }

    /// 压入一条事件；超出容量时挤出最旧条目。返回分配的 sequence_id。
    pub fn push(&mut self, event: WidgetEvent) -> u64 {
        self.roll_date_if_needed();

        // 每日计数器
        match &event {
            WidgetEvent::ApplySkipped { .. } => self.today_skip_count += 1,
            WidgetEvent::WatchdogTriggered { .. } => self.today_watchdog_count += 1,
            WidgetEvent::Error { source, .. } if source == "rebuild_window" => {
                self.today_rebuild_count += 1;
            }
            _ => {}
        }

        let seq = self.next_sequence_id;
        self.next_sequence_id += 1;

        if self.events.len() >= MAX_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(EventEntry {
            sequence_id: seq,
            timestamp: now_iso(),
            event,
        });
        seq
    }

    /// 返回最近事件快照（最多 20 条）。
    pub fn recent(&self, limit: usize) -> Vec<&EventEntry> {
        self.events
            .iter()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// 健康概览统计数据。
    pub fn health(&self) -> Value {
        let last_ping = self
            .events
            .iter()
            .rev()
            .find(|e| matches!(e.event, WidgetEvent::PingReceived))
            .map(|e| e.sequence_id);

        let last_apply = self
            .events
            .iter()
            .rev()
            .find(|e| matches!(e.event, WidgetEvent::ApplyAttempt { .. }))
            .map(|e| e.sequence_id);

        json!({
            "todaySkipCount": self.today_skip_count,
            "todayWatchdogCount": self.today_watchdog_count,
            "todayRebuildCount": self.today_rebuild_count,
            "lastPingSequenceId": last_ping,
            "lastApplySequenceId": last_apply,
            "totalEvents": self.events.len(),
        })
    }

    fn roll_date_if_needed(&mut self) {
        let today = today_str();
        if today != self.today_date {
            self.today_date = today;
            self.today_skip_count = 0;
            self.today_watchdog_count = 0;
            self.today_rebuild_count = 0;
        }
    }
}

// ── 辅助 ──────────────────────────────────────────

fn today_str() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn now_iso() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(variant: &str) -> WidgetEvent {
        match variant {
            "transition" => WidgetEvent::StateTransition {
                from: "peek".into(),
                to: "full".into(),
                trigger: "hover".into(),
            },
            "apply_ok" => WidgetEvent::ApplyAttempt {
                force: false,
                result: ApplyResult::Ok {
                    privacy_mask: false,
                },
                elapsed_ms: 12,
            },
            "apply_skipped" => WidgetEvent::ApplySkipped {
                reason: SkipReason::NoChange,
            },
            "window_created" => WidgetEvent::WindowCreated { elapsed_ms: 45 },
            "window_destroyed" => WidgetEvent::WindowDestroyed {
                reason: "disable".into(),
            },
            "error" => WidgetEvent::Error {
                source: "watchdog".into(),
                message: "rebuild failed".into(),
            },
            "ping" => WidgetEvent::PingReceived,
            "watchdog" => WidgetEvent::WatchdogTriggered {
                seconds_since_ping: 20,
            },
            "lifecycle" => WidgetEvent::Lifecycle {
                action: "enable".into(),
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn serde_roundtrip_all_variants() {
        let variants = [
            "transition",
            "apply_ok",
            "apply_skipped",
            "window_created",
            "window_destroyed",
            "error",
            "ping",
            "watchdog",
            "lifecycle",
        ];
        for v in &variants {
            let event = make_event(v);
            let json = serde_json::to_string(&event).expect("serialize");
            let _back: WidgetEvent = serde_json::from_str(&json).expect("deserialize");
        }
    }

    #[test]
    fn ring_buffer_enforces_capacity() {
        let mut ring = EventRing::new();
        for _ in 0..100 {
            ring.push(make_event("ping"));
        }
        let recent = ring.recent(50);
        assert_eq!(recent.len(), 50);
    }

    #[test]
    fn sequence_id_is_monotonic() {
        let mut ring = EventRing::new();
        let mut ids = Vec::new();
        for _ in 0..10 {
            ids.push(ring.push(make_event("ping")));
        }
        for i in 1..ids.len() {
            assert!(ids[i] > ids[i - 1], "seq ids must be monotonic");
        }
    }

    #[test]
    fn daily_counters_increment() {
        let mut ring = EventRing::new();
        ring.push(make_event("apply_skipped"));
        ring.push(make_event("apply_skipped"));
        ring.push(make_event("watchdog"));
        assert_eq!(ring.today_skip_count, 2);
        assert_eq!(ring.today_watchdog_count, 1);
    }

    #[test]
    fn health_snapshot_aggregates_correctly() {
        let mut ring = EventRing::new();
        ring.push(make_event("ping"));
        ring.push(make_event("apply_skipped"));
        ring.push(make_event("apply_ok"));
        ring.push(make_event("watchdog"));
        let h = ring.health();
        assert_eq!(h["todaySkipCount"].as_u64().unwrap(), 1);
        assert_eq!(h["todayWatchdogCount"].as_u64().unwrap(), 1);
        assert_eq!(h["totalEvents"].as_u64().unwrap(), 4);
    }
}