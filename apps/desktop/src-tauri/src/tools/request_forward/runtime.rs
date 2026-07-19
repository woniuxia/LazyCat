use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock, RwLock, RwLockReadGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::Utc;
use serde::Serialize;

use super::http::HttpRuleRunner;
use super::model::{encode_request_forward_error, ForwardProtocol, ForwardRule};
use super::observability::{
    HttpEvent, HttpObservability, ObservationBatch, ObservationCursor, TcpEvent, TcpObservability,
    UdpEvent, UdpObservability,
};
use super::repository::{self, ForwardLogWrite, ForwardStats, StatsDelta};
use super::tcp::TcpRuleRunner;
use super::udp::UdpRuleRunner;
use super::validation::validate_start_target;
use crate::tools::helpers::db_conn;

const OBSERVABILITY_FLUSH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

impl RuntimeState {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeStatus {
    pub rule_id: i64,
    pub state: RuntimeState,
    pub last_error: Option<String>,
    pub last_observability_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BatchOperationResult {
    pub rule_id: i64,
    pub ok: bool,
    pub error: Option<String>,
    pub state: RuntimeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RunningHandle(pub(super) u64);

pub(super) trait RuleRunner: Send + Sync {
    fn start(&self, rule: &ForwardRule) -> Result<RunningHandle, String>;
    /// Consumes the handle and leaves no live task, including when cleanup reports an error.
    fn stop(&self, handle: RunningHandle) -> Result<(), String>;

    fn take_failure(&self, _handle: RunningHandle) -> Option<String> {
        None
    }

    fn observation_source(&self, _handle: RunningHandle) -> Option<ObservationSource> {
        None
    }
}

pub(crate) trait LifecycleRepository {
    fn auto_start_rules(&self) -> Result<Vec<ForwardRule>, String>;
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub rule_id: i64,
    pub ok: bool,
    pub error: Option<String>,
    pub state: RuntimeState,
}

pub(crate) trait ObservabilityPersistence: Send + Sync {
    fn persist(
        &self,
        rule_id: i64,
        delta: StatsDelta,
        logs: &[ForwardLogWrite],
    ) -> Result<(), String>;

    fn stats(&self, rule_id: i64) -> Result<ForwardStats, String> {
        let _ = rule_id;
        Err("转发统计读取未实现".into())
    }

    fn reset_stats(&self, rule_id: i64) -> Result<(), String> {
        let _ = rule_id;
        Err("转发统计重置未实现".into())
    }
}

struct DatabaseObservabilityPersistence;

impl ObservabilityPersistence for DatabaseObservabilityPersistence {
    fn persist(
        &self,
        rule_id: i64,
        delta: StatsDelta,
        logs: &[ForwardLogWrite],
    ) -> Result<(), String> {
        let mut conn = db_conn()?;
        repository::persist_observability_with_conn(&mut conn, rule_id, delta, logs)
    }

    fn stats(&self, rule_id: i64) -> Result<ForwardStats, String> {
        let conn = db_conn()?;
        repository::get_stats_with_conn(&conn, rule_id)
    }

    fn reset_stats(&self, rule_id: i64) -> Result<(), String> {
        let conn = db_conn()?;
        repository::reset_stats_with_conn(&conn, rule_id)
    }
}

#[derive(Clone)]
pub(crate) enum ObservationSource {
    Tcp(Arc<TcpObservability>),
    Udp(Arc<UdpObservability>),
    Http(Arc<HttpObservability>),
}

enum UnifiedObservationBatch {
    Tcp(ObservationBatch<TcpEvent>),
    Udp(ObservationBatch<UdpEvent>),
    Http(ObservationBatch<HttpEvent>),
}

impl ObservationSource {
    fn batch_since(&self, cursor: ObservationCursor) -> UnifiedObservationBatch {
        match self {
            Self::Tcp(source) => UnifiedObservationBatch::Tcp(source.batch_since(cursor)),
            Self::Udp(source) => UnifiedObservationBatch::Udp(source.batch_since(cursor)),
            Self::Http(source) => UnifiedObservationBatch::Http(source.batch_since(cursor)),
        }
    }
}

#[derive(Clone)]
struct RuntimeInstance {
    state: RuntimeState,
    handle: Option<RunningHandle>,
    last_error: Option<String>,
}

impl RuntimeInstance {
    fn stopped() -> Self {
        Self {
            state: RuntimeState::Stopped,
            handle: None,
            last_error: None,
        }
    }

    fn status(&self, rule_id: i64, last_observability_error: Option<String>) -> RuntimeStatus {
        RuntimeStatus {
            rule_id,
            state: self.state,
            last_error: self
                .last_error
                .as_deref()
                .map(|error| encode_request_forward_error(error, self.state.as_str())),
            last_observability_error,
        }
    }
}

#[derive(Default)]
struct RuleObservabilityState {
    cursor: ObservationCursor,
    source: Option<ObservationSource>,
    last_error: Option<String>,
}

struct ObservationWorker {
    stop: Sender<()>,
    join: JoinHandle<()>,
}

pub(crate) struct RuntimeManager {
    runner: Arc<dyn RuleRunner>,
    observability_persistence: Arc<dyn ObservabilityPersistence>,
    lifecycle: RwLock<bool>,
    instances: Mutex<HashMap<i64, RuntimeInstance>>,
    rule_locks: Mutex<HashMap<i64, Arc<Mutex<()>>>>,
    observability_states: Arc<Mutex<HashMap<i64, Arc<Mutex<RuleObservabilityState>>>>>,
    observability_workers: Mutex<HashMap<i64, ObservationWorker>>,
}

impl RuntimeManager {
    pub(super) fn new(runner: Arc<dyn RuleRunner>) -> Self {
        Self::with_observability_persistence(runner, Arc::new(DatabaseObservabilityPersistence))
    }

    pub(super) fn with_observability_persistence(
        runner: Arc<dyn RuleRunner>,
        observability_persistence: Arc<dyn ObservabilityPersistence>,
    ) -> Self {
        Self {
            runner,
            observability_persistence,
            lifecycle: RwLock::new(false),
            instances: Mutex::new(HashMap::new()),
            rule_locks: Mutex::new(HashMap::new()),
            observability_states: Arc::new(Mutex::new(HashMap::new())),
            observability_workers: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn start(&self, rule: &ForwardRule) -> Result<RuntimeStatus, String> {
        self.with_rule_lock(rule.id, || self.start_locked(rule))
    }

    pub(crate) fn start_loaded(
        &self,
        rule_id: i64,
        load_rule: impl FnOnce() -> Result<ForwardRule, String>,
    ) -> Result<RuntimeStatus, String> {
        self.with_rule_lock(rule_id, || {
            let rule = load_rule()?;
            if rule.id != rule_id {
                return Err("转发规则 ID 不匹配".into());
            }
            self.start_locked(&rule)
        })
    }

    pub(crate) fn stop(&self, rule: &ForwardRule) -> Result<RuntimeStatus, String> {
        self.with_rule_lock(rule.id, || self.stop_locked(rule))
    }

    pub(crate) fn stop_loaded(
        &self,
        rule_id: i64,
        load_rule: impl FnOnce() -> Result<ForwardRule, String>,
    ) -> Result<RuntimeStatus, String> {
        self.with_rule_lock(rule_id, || {
            let rule = load_rule()?;
            if rule.id != rule_id {
                return Err("转发规则 ID 不匹配".into());
            }
            self.stop_locked(&rule)
        })
    }

    pub(crate) fn start_all_loaded(
        &self,
        rule_ids: &[i64],
        load_rule: impl Fn(i64) -> Result<ForwardRule, String>,
    ) -> Vec<BatchOperationResult> {
        rule_ids
            .iter()
            .map(|&rule_id| self.batch_result(rule_id, self.start_loaded(rule_id, || load_rule(rule_id))))
            .collect()
    }

    pub(crate) fn stop_all_loaded(
        &self,
        rule_ids: &[i64],
        load_rule: impl Fn(i64) -> Result<ForwardRule, String>,
    ) -> Vec<BatchOperationResult> {
        rule_ids
            .iter()
            .map(|&rule_id| self.batch_result(rule_id, self.stop_loaded(rule_id, || load_rule(rule_id))))
            .collect()
    }

    pub(crate) fn restore_auto_start_rules(
        &self,
        repository: &dyn LifecycleRepository,
    ) -> Result<Vec<RestoreResult>, String> {
        Ok(repository
            .auto_start_rules()?
            .into_iter()
            .map(|rule| {
                let rule_id = rule.id;
                let result = self.with_rule_lock(rule_id, || self.start_restored_locked(&rule));
                match result {
                    Ok(status) => RestoreResult {
                        rule_id,
                        ok: true,
                        error: None,
                        state: status.state,
                    },
                    Err(error) => {
                        let state = self.status(rule_id).state;
                        RestoreResult {
                            rule_id,
                            ok: false,
                            error: Some(encode_request_forward_error(&error, state.as_str())),
                            state,
                        }
                    }
                }
            })
            .collect())
    }

    pub(crate) fn on_app_exit(&self) -> Vec<BatchOperationResult> {
        {
            let mut shutting_down = self
                .lifecycle
                .write()
                .expect("request-forward lifecycle lock poisoned");
            *shutting_down = true;
        }
        let rule_ids = self
            .instances
            .lock()
            .expect("request-forward instances lock poisoned")
            .keys()
            .copied()
            .collect::<Vec<_>>();
        rule_ids
            .into_iter()
            .map(|rule_id| {
                self.batch_result(
                    rule_id,
                    self.with_rule_lock(rule_id, || self.stop_for_shutdown_locked(rule_id)),
                )
            })
            .collect()
    }

    pub(crate) fn status(&self, rule_id: i64) -> RuntimeStatus {
        self.reconcile_runner_failure(rule_id);
        let last_observability_error = self.last_observability_error(rule_id);
        self.instances
            .lock()
            .expect("request-forward instances lock poisoned")
            .get(&rule_id)
            .map(|instance| instance.status(rule_id, last_observability_error.clone()))
            .unwrap_or_else(|| RuntimeInstance::stopped().status(rule_id, last_observability_error))
    }

    pub(crate) fn statuses(&self, rule_ids: impl IntoIterator<Item = i64>) -> Vec<RuntimeStatus> {
        rule_ids
            .into_iter()
            .map(|rule_id| self.status(rule_id))
            .collect()
    }

    pub(crate) fn ensure_rule_mutable(&self, rule_id: i64) -> Result<(), String> {
        match self.status(rule_id).state {
            RuntimeState::Stopped | RuntimeState::Failed => Ok(()),
            RuntimeState::Starting | RuntimeState::Running | RuntimeState::Stopping => {
                Err("已启动的转发规则不能修改或删除".into())
            }
        }
    }

    pub(crate) fn with_rule_mutation<T>(
        &self,
        rule_id: i64,
        mutation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        self.with_rule_lock(rule_id, || {
            self.ensure_rule_mutable(rule_id)?;
            mutation()
        })
    }

    pub(crate) fn with_auto_start_mutation<T>(
        &self,
        rule_id: i64,
        mutation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        self.with_rule_lock(rule_id, mutation)
    }

    fn with_rule_lock<T>(
        &self,
        rule_id: i64,
        action: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let rule_lock = self.rule_lock(rule_id);
        let _rule_guard = rule_lock
            .lock()
            .expect("request-forward rule lock poisoned");
        action()
    }

    fn start_lifecycle_guard(&self) -> Result<RwLockReadGuard<'_, bool>, String> {
        let guard = self
            .lifecycle
            .read()
            .expect("request-forward lifecycle lock poisoned");
        if *guard {
            return Err("应用正在退出，无法启动转发规则".into());
        }
        Ok(guard)
    }

    pub(crate) fn clear_rule_state(&self, rule_id: i64) {
        self.stop_observability(rule_id);
        self.instances
            .lock()
            .expect("request-forward instances lock poisoned")
            .remove(&rule_id);
        self.observability_states
            .lock()
            .expect("request-forward observability states lock poisoned")
            .remove(&rule_id);
    }

    #[cfg(test)]
    pub(crate) fn flush_observability(&self, rule_id: i64) {
        let state = self.observability_state(rule_id);
        flush_observability_state(rule_id, &state, &*self.observability_persistence);
    }

    pub(crate) fn stats(&self, rule_id: i64) -> Result<ForwardStats, String> {
        let state = self.observability_state(rule_id);
        let guard = state
            .lock()
            .expect("request-forward rule observability lock poisoned");
        let mut stats = self.observability_persistence.stats(rule_id)?;
        if let Some(source) = &guard.source {
            let batch = source.batch_since(guard.cursor);
            let delta = unified_delta(&batch);
            stats.event_count = stats.event_count.saturating_add(delta.event_count);
            stats.upload_bytes = stats.upload_bytes.saturating_add(delta.upload_bytes);
            stats.download_bytes = stats.download_bytes.saturating_add(delta.download_bytes);
            stats.error_count = stats.error_count.saturating_add(delta.error_count);
        }
        Ok(stats)
    }

    pub(crate) fn reset_stats(&self, rule_id: i64) -> Result<ForwardStats, String> {
        let state = self.observability_state(rule_id);
        let mut guard = state
            .lock()
            .expect("request-forward rule observability lock poisoned");
        self.observability_persistence.reset_stats(rule_id)?;
        let current = guard
            .source
            .as_ref()
            .map(|source| unified_next_cursor(&source.batch_since(ObservationCursor::default())))
            .unwrap_or_default();
        guard.cursor.totals = current.totals;
        self.observability_persistence.stats(rule_id)
    }

    fn start_locked(
        &self,
        rule: &ForwardRule,
    ) -> Result<RuntimeStatus, String> {
        let _lifecycle_guard = self.start_lifecycle_guard()?;
        if self.status(rule.id).state == RuntimeState::Running {
            return Ok(self.status(rule.id));
        }

        self.set_instance(
            rule.id,
            RuntimeInstance {
                state: RuntimeState::Starting,
                handle: None,
                last_error: None,
            },
        );

        let handle = match self.runner.start(rule) {
            Ok(handle) => handle,
            Err(error) => {
                self.set_failed(rule.id, error.clone());
                return Err(error);
            }
        };

        self.set_instance(
            rule.id,
            RuntimeInstance {
                state: RuntimeState::Running,
                handle: Some(handle),
                last_error: None,
            },
        );

        self.start_observability(rule, handle);

        Ok(self.status(rule.id))
    }

    fn start_restored_locked(&self, rule: &ForwardRule) -> Result<RuntimeStatus, String> {
        let _lifecycle_guard = self.start_lifecycle_guard()?;
        if self.status(rule.id).state == RuntimeState::Running {
            return Ok(self.status(rule.id));
        }
        self.set_state(rule.id, RuntimeState::Starting, None, None);
        let handle = match self.runner.start(rule) {
            Ok(handle) => handle,
            Err(error) => {
                self.set_failed(rule.id, error.clone());
                return Err(error);
            }
        };
        self.set_state(rule.id, RuntimeState::Running, Some(handle), None);
        self.start_observability(rule, handle);
        Ok(self.status(rule.id))
    }

    fn stop_for_shutdown_locked(&self, rule_id: i64) -> Result<RuntimeStatus, String> {
        self.reconcile_runner_failure(rule_id);
        let previous = self.instance(rule_id);
        let Some(handle) = previous.handle else {
            self.stop_observability(rule_id);
            return Ok(self.status(rule_id));
        };
        self.set_state(rule_id, RuntimeState::Stopping, Some(handle), None);
        let stop_result = self.runner.stop(handle);
        self.stop_observability(rule_id);
        match stop_result {
            Ok(()) => {
                self.set_instance(rule_id, RuntimeInstance::stopped());
                Ok(self.status(rule_id))
            }
            Err(error) => {
                self.set_failed(rule_id, error.clone());
                Err(error)
            }
        }
    }

    fn stop_locked(&self, rule: &ForwardRule) -> Result<RuntimeStatus, String> {
        self.reconcile_runner_failure(rule.id);
        let previous = self.instance(rule.id);

        if previous.state == RuntimeState::Failed {
            self.set_instance(rule.id, RuntimeInstance::stopped());
            return Ok(self.status(rule.id));
        }

        let had_running_handle = previous.state == RuntimeState::Running;

        if had_running_handle {
            let handle = previous.handle.expect("running state must have a handle");
            self.set_state(rule.id, RuntimeState::Stopping, Some(handle), None);
            if let Err(stop_error) = self.runner.stop(handle) {
                self.stop_observability(rule.id);
                self.set_failed(rule.id, stop_error.clone());
                return Err(stop_error);
            }
            self.stop_observability(rule.id);
            self.set_instance(rule.id, RuntimeInstance::stopped());
        }

        self.set_instance(rule.id, RuntimeInstance::stopped());
        Ok(self.status(rule.id))
    }

    fn batch_result(
        &self,
        rule_id: i64,
        result: Result<RuntimeStatus, String>,
    ) -> BatchOperationResult {
        match result {
            Ok(status) => BatchOperationResult {
                rule_id,
                ok: true,
                error: None,
                state: status.state,
            },
            Err(error) => {
                let state = self.status(rule_id).state;
                BatchOperationResult {
                    rule_id,
                    ok: false,
                    error: Some(encode_request_forward_error(&error, state.as_str())),
                    state,
                }
            }
        }
    }

    fn rule_lock(&self, rule_id: i64) -> Arc<Mutex<()>> {
        let mut locks = self
            .rule_locks
            .lock()
            .expect("request-forward rule locks mutex poisoned");
        locks
            .entry(rule_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn instance(&self, rule_id: i64) -> RuntimeInstance {
        self.instances
            .lock()
            .expect("request-forward instances lock poisoned")
            .get(&rule_id)
            .cloned()
            .unwrap_or_else(RuntimeInstance::stopped)
    }

    fn set_instance(&self, rule_id: i64, instance: RuntimeInstance) {
        self.instances
            .lock()
            .expect("request-forward instances lock poisoned")
            .insert(rule_id, instance);
    }

    fn set_state(
        &self,
        rule_id: i64,
        state: RuntimeState,
        handle: Option<RunningHandle>,
        last_error: Option<String>,
    ) {
        self.set_instance(
            rule_id,
            RuntimeInstance {
                state,
                handle,
                last_error,
            },
        );
    }

    fn set_failed(&self, rule_id: i64, error: String) {
        self.set_state(rule_id, RuntimeState::Failed, None, Some(error));
    }

    fn reconcile_runner_failure(&self, rule_id: i64) {
        let handle = self
            .instances
            .lock()
            .expect("request-forward instances lock poisoned")
            .get(&rule_id)
            .filter(|instance| instance.state == RuntimeState::Running)
            .and_then(|instance| instance.handle);
        if let Some(handle) = handle {
            if let Some(error) = self.runner.take_failure(handle) {
                self.stop_observability(rule_id);
                self.set_failed(rule_id, error);
            }
        }
    }

    fn start_observability(&self, rule: &ForwardRule, handle: RunningHandle) {
        let Some(source) = self.runner.observation_source(handle) else {
            return;
        };
        self.stop_observability(rule.id);
        let state = self.observability_state(rule.id);
        {
            let mut guard = state
                .lock()
                .expect("request-forward rule observability lock poisoned");
            guard.cursor = ObservationCursor::default();
            guard.source = Some(source);
            guard.last_error = None;
        }
        let (stop_tx, stop_rx) = mpsc::channel();
        let worker_state = Arc::clone(&state);
        let worker_persistence = Arc::clone(&self.observability_persistence);
        let rule_id = rule.id;
        let join = thread::Builder::new()
            .name(format!("request-forward-observability-{rule_id}"))
            .spawn(move || loop {
                match stop_rx.recv_timeout(OBSERVABILITY_FLUSH_INTERVAL) {
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => {
                        flush_observability_state(rule_id, &worker_state, &*worker_persistence);
                        worker_state
                            .lock()
                            .expect("request-forward rule observability lock poisoned")
                            .source = None;
                        break;
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        flush_observability_state(rule_id, &worker_state, &*worker_persistence)
                    }
                }
            });
        match join {
            Ok(join) => {
                self.observability_workers
                    .lock()
                    .expect("request-forward observability workers lock poisoned")
                    .insert(
                        rule_id,
                        ObservationWorker {
                            stop: stop_tx,
                            join,
                        },
                    );
            }
            Err(error) => {
                state
                    .lock()
                    .expect("request-forward rule observability lock poisoned")
                    .last_error = Some(format!("无法启动观测持久化线程: {error}"));
            }
        }
    }

    fn stop_observability(&self, rule_id: i64) {
        let worker = self
            .observability_workers
            .lock()
            .expect("request-forward observability workers lock poisoned")
            .remove(&rule_id);
        if let Some(worker) = worker {
            let _ = worker.stop.send(());
            if worker.join.join().is_err() {
                self.observability_state(rule_id)
                    .lock()
                    .expect("request-forward rule observability lock poisoned")
                    .last_error = Some("观测持久化线程异常退出".into());
            }
        }
    }

    fn observability_state(&self, rule_id: i64) -> Arc<Mutex<RuleObservabilityState>> {
        self.observability_states
            .lock()
            .expect("request-forward observability states lock poisoned")
            .entry(rule_id)
            .or_insert_with(|| Arc::new(Mutex::new(RuleObservabilityState::default())))
            .clone()
    }

    fn last_observability_error(&self, rule_id: i64) -> Option<String> {
        self.observability_states
            .lock()
            .expect("request-forward observability states lock poisoned")
            .get(&rule_id)
            .and_then(|state| {
                state
                    .lock()
                    .expect("request-forward rule observability lock poisoned")
                    .last_error
                    .clone()
            })
    }
}

fn flush_observability_state(
    rule_id: i64,
    state: &Arc<Mutex<RuleObservabilityState>>,
    persistence: &dyn ObservabilityPersistence,
) {
    let mut state = state
        .lock()
        .expect("request-forward rule observability lock poisoned");
    let Some(source) = state.source.clone() else {
        return;
    };
    let batch = source.batch_since(state.cursor);
    let delta = unified_delta(&batch);
    let gap_error = unified_gap_error(&batch);
    let next_cursor = unified_next_cursor(&batch);
    let logs = unified_logs(batch);
    if delta == StatsDelta::default() && logs.is_empty() && gap_error.is_none() {
        return;
    }
    match persistence.persist(rule_id, delta, &logs) {
        Ok(()) => {
            state.cursor = next_cursor;
            state.last_error = gap_error;
        }
        Err(error) => {
            state.last_error = Some(match gap_error {
                Some(gap) => format!("{error}; {gap}"),
                None => error,
            });
        }
    }
}

fn unified_delta(batch: &UnifiedObservationBatch) -> StatsDelta {
    match batch {
        UnifiedObservationBatch::Tcp(batch) => batch.delta,
        UnifiedObservationBatch::Udp(batch) => batch.delta,
        UnifiedObservationBatch::Http(batch) => batch.delta,
    }
}

fn unified_next_cursor(batch: &UnifiedObservationBatch) -> ObservationCursor {
    match batch {
        UnifiedObservationBatch::Tcp(batch) => batch.next_cursor,
        UnifiedObservationBatch::Udp(batch) => batch.next_cursor,
        UnifiedObservationBatch::Http(batch) => batch.next_cursor,
    }
}

fn unified_gap_error(batch: &UnifiedObservationBatch) -> Option<String> {
    let gap = match batch {
        UnifiedObservationBatch::Tcp(batch) => batch.gap,
        UnifiedObservationBatch::Udp(batch) => batch.gap,
        UnifiedObservationBatch::Http(batch) => batch.gap,
    }?;
    Some(format!(
        "观测事件缓冲区已丢弃 {} 条未持久化日志，首条可用序号为 {}",
        gap.dropped_events, gap.first_available_sequence
    ))
}

fn unified_logs(batch: UnifiedObservationBatch) -> Vec<ForwardLogWrite> {
    let created_at = || Utc::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
    match batch {
        UnifiedObservationBatch::Tcp(batch) => batch
            .events
            .into_iter()
            .map(|event| ForwardLogWrite {
                protocol: ForwardProtocol::Tcp,
                client_addr: event.client_addr,
                target_addr: event.target_addr.unwrap_or_else(|| "TCP".into()),
                method: None,
                path: None,
                status_code: None,
                duration_ms: None,
                upload_bytes: event.upload_bytes,
                download_bytes: event.download_bytes,
                request_headers: None,
                response_headers: None,
                request_body_preview: None,
                response_body_preview: None,
                request_body_truncated: false,
                response_body_truncated: false,
                error: event.error,
                created_at: created_at(),
            })
            .collect(),
        UnifiedObservationBatch::Udp(batch) => batch
            .events
            .into_iter()
            .map(|event| ForwardLogWrite {
                protocol: ForwardProtocol::Udp,
                client_addr: event.client_addr.map(|value| value.to_string()),
                target_addr: event
                    .target_addr
                    .map(|value| value.to_string())
                    .unwrap_or(event.target),
                method: None,
                path: None,
                status_code: None,
                duration_ms: None,
                upload_bytes: event.upload_bytes,
                download_bytes: event.download_bytes,
                request_headers: None,
                response_headers: None,
                request_body_preview: None,
                response_body_preview: None,
                request_body_truncated: false,
                response_body_truncated: false,
                error: event.error,
                created_at: created_at(),
            })
            .collect(),
        UnifiedObservationBatch::Http(batch) => batch
            .events
            .into_iter()
            .map(|event| {
                let request_body_truncated = event
                    .request_body_preview
                    .as_ref()
                    .is_some_and(|preview| preview.truncated);
                let response_body_truncated = event
                    .response_body_preview
                    .as_ref()
                    .is_some_and(|preview| preview.truncated);
                ForwardLogWrite {
                    protocol: ForwardProtocol::Http,
                    client_addr: event.client_addr,
                    target_addr: event.target_addr,
                    method: event.method,
                    path: event.path,
                    status_code: event.status_code,
                    duration_ms: event.duration_ms,
                    upload_bytes: event.upload_bytes,
                    download_bytes: event.download_bytes,
                    request_headers: event.request_headers,
                    response_headers: event.response_headers,
                    request_body_preview: event
                        .request_body_preview
                        .map(|preview| String::from_utf8_lossy(&preview.bytes).into_owned()),
                    response_body_preview: event
                        .response_body_preview
                        .map(|preview| String::from_utf8_lossy(&preview.bytes).into_owned()),
                    request_body_truncated,
                    response_body_truncated,
                    error: event.error,
                    created_at: created_at(),
                }
            })
            .collect(),
    }
}

struct ProtocolRunner {
    http: HttpRuleRunner,
    tcp: TcpRuleRunner,
    udp: UdpRuleRunner,
    next_handle: AtomicU64,
    running: Mutex<HashMap<u64, ProtocolChildHandle>>,
}

#[derive(Clone, Copy)]
enum ProtocolChildHandle {
    Http(RunningHandle),
    Tcp(RunningHandle),
    Udp(RunningHandle),
}

impl Default for ProtocolRunner {
    fn default() -> Self {
        Self {
            http: HttpRuleRunner::new(),
            tcp: TcpRuleRunner::new(),
            udp: UdpRuleRunner::new(),
            next_handle: AtomicU64::new(1),
            running: Mutex::new(HashMap::new()),
        }
    }
}

impl RuleRunner for ProtocolRunner {
    fn start(&self, rule: &ForwardRule) -> Result<RunningHandle, String> {
        validate_start_target(rule)?;
        let child_handle = match rule.protocol {
            ForwardProtocol::Http => self.http.start(rule),
            ForwardProtocol::Tcp => self.tcp.start(rule),
            ForwardProtocol::Udp => self.udp.start(rule),
        }?;
        let protocol_handle = match rule.protocol {
            ForwardProtocol::Http => ProtocolChildHandle::Http(child_handle),
            ForwardProtocol::Tcp => ProtocolChildHandle::Tcp(child_handle),
            ForwardProtocol::Udp => ProtocolChildHandle::Udp(child_handle),
        };
        let handle = RunningHandle(self.next_handle.fetch_add(1, Ordering::Relaxed));
        self.running
            .lock()
            .expect("request-forward protocol runner lock poisoned")
            .insert(handle.0, protocol_handle);
        Ok(handle)
    }

    fn stop(&self, handle: RunningHandle) -> Result<(), String> {
        let protocol_handle = self
            .running
            .lock()
            .expect("request-forward protocol runner lock poisoned")
            .remove(&handle.0)
            .ok_or_else(|| "转发规则运行句柄不存在".to_string())?;
        match protocol_handle {
            ProtocolChildHandle::Http(handle) => self.http.stop(handle),
            ProtocolChildHandle::Tcp(handle) => self.tcp.stop(handle),
            ProtocolChildHandle::Udp(handle) => self.udp.stop(handle),
        }
    }

    fn take_failure(&self, handle: RunningHandle) -> Option<String> {
        let protocol_handle = self
            .running
            .lock()
            .expect("request-forward protocol runner lock poisoned")
            .get(&handle.0)
            .copied()?;
        let failure = match protocol_handle {
            ProtocolChildHandle::Http(handle) => self.http.take_failure(handle),
            ProtocolChildHandle::Tcp(handle) => self.tcp.take_failure(handle),
            ProtocolChildHandle::Udp(handle) => self.udp.take_failure(handle),
        };
        if failure.is_some() {
            self.running
                .lock()
                .expect("request-forward protocol runner lock poisoned")
                .remove(&handle.0);
        }
        failure
    }

    fn observation_source(&self, handle: RunningHandle) -> Option<ObservationSource> {
        let protocol_handle = self
            .running
            .lock()
            .expect("request-forward protocol runner lock poisoned")
            .get(&handle.0)
            .copied()?;
        match protocol_handle {
            ProtocolChildHandle::Http(handle) => self
                .http
                .observability(handle)
                .ok()
                .map(ObservationSource::Http),
            ProtocolChildHandle::Tcp(handle) => self
                .tcp
                .observability(handle)
                .ok()
                .map(ObservationSource::Tcp),
            ProtocolChildHandle::Udp(handle) => self
                .udp
                .observability(handle)
                .ok()
                .map(ObservationSource::Udp),
        }
    }
}

static RUNTIME_MANAGER: OnceLock<RuntimeManager> = OnceLock::new();

pub(crate) fn global_manager() -> &'static RuntimeManager {
    RUNTIME_MANAGER.get_or_init(|| RuntimeManager::new(Arc::new(ProtocolRunner::default())))
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::net::{TcpListener, UdpSocket};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use super::{
        unified_delta, unified_logs, unified_next_cursor,
        LifecycleRepository, ObservabilityPersistence, ObservationBatch, ObservationCursor,
        ObservationSource, ProtocolRunner, RuleRunner, RunningHandle, RuntimeManager, RuntimeState,
        UnifiedObservationBatch,
    };
    use crate::tools::helpers::ensure_request_forward_schema_for_test;
    use crate::tools::request_forward::model::{ForwardProtocol, ForwardRule, RuleWriteInput};
    use crate::tools::request_forward::observability::{
        HttpObservability, TcpEvent, TcpEventKind, TcpObservability, UdpEvent, UdpEventKind,
        HTTP_BODY_PREVIEW_LIMIT,
    };
    use crate::tools::request_forward::repository;
    use crate::tools::request_forward::repository::{ForwardLogWrite, ForwardStats, StatsDelta};
    use hyper::http::{HeaderMap, HeaderValue};
    use rusqlite::Connection;

    #[test]
    fn tcp_unified_logs_keep_bytes_per_event() {
        let batch = UnifiedObservationBatch::Tcp(ObservationBatch {
            delta: StatsDelta {
                event_count: 2,
                upload_bytes: 15,
                download_bytes: 27,
                error_count: 0,
            },
            events: vec![
                TcpEvent {
                    sequence: 1,
                    kind: TcpEventKind::Accepted,
                    client_addr: Some("client-1".into()),
                    target_addr: Some("target-1".into()),
                    upload_bytes: 3,
                    download_bytes: 4,
                    error: None,
                },
                TcpEvent {
                    sequence: 2,
                    kind: TcpEventKind::Accepted,
                    client_addr: Some("client-2".into()),
                    target_addr: Some("target-2".into()),
                    upload_bytes: 12,
                    download_bytes: 23,
                    error: None,
                },
            ],
            next_cursor: ObservationCursor {
                totals: StatsDelta {
                    event_count: 2,
                    upload_bytes: 15,
                    download_bytes: 27,
                    error_count: 0,
                },
                last_event_sequence: 2,
            },
            gap: None,
        });

        let logs = unified_logs(batch);
        assert_eq!(logs.len(), 2);
        assert_eq!((logs[0].upload_bytes, logs[0].download_bytes), (3, 4));
        assert_eq!((logs[1].upload_bytes, logs[1].download_bytes), (12, 23));
    }

    #[test]
    fn udp_unified_logs_keep_bytes_per_session_event() {
        let batch = UnifiedObservationBatch::Udp(ObservationBatch {
            delta: StatsDelta {
                event_count: 2,
                upload_bytes: 9,
                download_bytes: 21,
                error_count: 0,
            },
            events: vec![
                UdpEvent {
                    sequence: 1,
                    kind: UdpEventKind::SessionExpired,
                    client_addr: Some("127.0.0.1:12001".parse().unwrap()),
                    target: "127.0.0.1:53".into(),
                    target_addr: Some("127.0.0.1:53".parse().unwrap()),
                    upload_bytes: 2,
                    download_bytes: 8,
                    error: None,
                },
                UdpEvent {
                    sequence: 2,
                    kind: UdpEventKind::SessionExpired,
                    client_addr: Some("127.0.0.1:12002".parse().unwrap()),
                    target: "127.0.0.1:53".into(),
                    target_addr: Some("127.0.0.1:53".parse().unwrap()),
                    upload_bytes: 7,
                    download_bytes: 13,
                    error: None,
                },
            ],
            next_cursor: ObservationCursor {
                totals: StatsDelta {
                    event_count: 2,
                    upload_bytes: 9,
                    download_bytes: 21,
                    error_count: 0,
                },
                last_event_sequence: 2,
            },
            gap: None,
        });

        let logs = unified_logs(batch);
        assert_eq!(logs.len(), 2);
        assert_eq!((logs[0].upload_bytes, logs[0].download_bytes), (2, 8));
        assert_eq!((logs[1].upload_bytes, logs[1].download_bytes), (7, 13));
    }

    #[derive(Default)]
    struct FakeRunner {
        inner: Mutex<FakeRunnerState>,
        start_calls: AtomicUsize,
        stop_calls: AtomicUsize,
    }

    #[derive(Default)]
    struct FakeRunnerState {
        next_handle: u64,
        live_handles: HashSet<u64>,
        start_errors: HashMap<i64, String>,
        stop_error: Option<String>,
        start_gate: Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>,
        observation_source: Option<ObservationSource>,
    }

    impl FakeRunner {
        fn fail_start_for(&self, rule_id: i64, error: &str) {
            self.inner
                .lock()
                .expect("lock fake runner")
                .start_errors
                .insert(rule_id, error.into());
        }

        fn fail_stop(&self, error: &str) {
            self.inner.lock().expect("lock fake runner").stop_error = Some(error.into());
        }

        fn block_next_start(&self) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
            let (entered_tx, entered_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            self.inner.lock().expect("lock fake runner").start_gate =
                Some((entered_tx, release_rx));
            (entered_rx, release_tx)
        }

        fn has_live_task(&self) -> bool {
            !self
                .inner
                .lock()
                .expect("lock fake runner")
                .live_handles
                .is_empty()
        }

        fn set_observation_source(&self, source: ObservationSource) {
            self.inner
                .lock()
                .expect("lock fake runner")
                .observation_source = Some(source);
        }
    }

    impl RuleRunner for FakeRunner {
        fn start(&self, rule: &ForwardRule) -> Result<RunningHandle, String> {
            self.start_calls.fetch_add(1, Ordering::SeqCst);
            let gate = {
                let mut state = self.inner.lock().expect("lock fake runner");
                if let Some(error) = state.start_errors.get(&rule.id) {
                    return Err(error.clone());
                }
                state.start_gate.take()
            };
            if let Some((entered, release)) = gate {
                entered.send(()).expect("signal entered start");
                release.recv().expect("release start");
            }

            let mut state = self.inner.lock().expect("lock fake runner");
            state.next_handle += 1;
            let handle = RunningHandle(state.next_handle);
            state.live_handles.insert(handle.0);
            Ok(handle)
        }

        fn stop(&self, handle: RunningHandle) -> Result<(), String> {
            self.stop_calls.fetch_add(1, Ordering::SeqCst);
            let mut state = self.inner.lock().expect("lock fake runner");
            state.live_handles.remove(&handle.0);
            if let Some(error) = state.stop_error.take() {
                return Err(error);
            }
            Ok(())
        }

        fn observation_source(&self, _handle: RunningHandle) -> Option<ObservationSource> {
            self.inner
                .lock()
                .expect("lock fake runner")
                .observation_source
                .clone()
        }
    }

    struct FailingObservabilityPersistence;

    impl ObservabilityPersistence for FailingObservabilityPersistence {
        fn persist(
            &self,
            _rule_id: i64,
            _delta: StatsDelta,
            _logs: &[ForwardLogWrite],
        ) -> Result<(), String> {
            Err("database is read-only".into())
        }
    }

    struct MemoryObservabilityPersistence {
        conn: Mutex<Connection>,
    }

    impl MemoryObservabilityPersistence {
        fn new(conn: Connection) -> Self {
            Self {
                conn: Mutex::new(conn),
            }
        }
    }

    impl ObservabilityPersistence for MemoryObservabilityPersistence {
        fn persist(
            &self,
            rule_id: i64,
            delta: StatsDelta,
            logs: &[ForwardLogWrite],
        ) -> Result<(), String> {
            repository::persist_observability_with_conn(
                &mut self.conn.lock().expect("lock memory database"),
                rule_id,
                delta,
                logs,
            )
        }

        fn stats(&self, rule_id: i64) -> Result<ForwardStats, String> {
            repository::get_stats_with_conn(
                &self.conn.lock().expect("lock memory database"),
                rule_id,
            )
        }

        fn reset_stats(&self, rule_id: i64) -> Result<(), String> {
            repository::reset_stats_with_conn(
                &self.conn.lock().expect("lock memory database"),
                rule_id,
            )
        }
    }

    #[derive(Default)]
    struct FakePersistence {
        values: Mutex<HashMap<i64, bool>>,
    }

    impl FakePersistence {
        fn value(&self, rule_id: i64) -> Option<bool> {
            self.values
                .lock()
                .expect("lock fake persistence")
                .get(&rule_id)
                .copied()
        }
    }

    impl FakePersistence {
        fn set_auto_start(&self, rule_id: i64, value: bool) -> Result<(), String> {
            self.values
                .lock()
                .expect("lock fake persistence")
                .insert(rule_id, value);
            Ok(())
        }
    }

    struct FakeLifecycleRepository {
        rules: Vec<ForwardRule>,
    }

    impl LifecycleRepository for FakeLifecycleRepository {
        fn auto_start_rules(&self) -> Result<Vec<ForwardRule>, String> {
            Ok(self.rules.clone())
        }
    }

    fn rule(id: i64) -> ForwardRule {
        ForwardRule {
            id,
            name: format!("规则 {id}"),
            protocol: ForwardProtocol::Tcp,
            bind_host: "127.0.0.1".into(),
            listen_port: 9000 + id as u16,
            target_url: None,
            target_host: Some("127.0.0.2".into()),
            target_port: Some(10_000 + id as u16),
            capture_http_headers: false,
            capture_http_body: false,
            auto_start: false,
            created_at: "2026-07-14 00:00:00".into(),
            updated_at: "2026-07-14 00:00:00".into(),
        }
    }

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        ensure_request_forward_schema_for_test(&conn).expect("create request-forward schema");
        conn
    }

    fn write_input(name: &str) -> RuleWriteInput {
        RuleWriteInput {
            name: name.into(),
            protocol: ForwardProtocol::Tcp,
            bind_host: "127.0.0.1".into(),
            listen_port: 9100,
            target_url: None,
            target_host: Some("127.0.0.2".into()),
            target_port: Some(10_100),
            capture_http_headers: false,
            capture_http_body: false,
        }
    }

    #[test]
    fn restore_attempts_every_auto_start_rule_and_isolates_failures() {
        let runner = Arc::new(FakeRunner::default());
        runner.fail_start_for(1, "first restore failed");
        let manager = RuntimeManager::new(runner.clone());
        let repository = FakeLifecycleRepository {
            rules: vec![rule(1), rule(2), rule(3)]
                .into_iter()
                .map(|mut rule| {
                    rule.auto_start = true;
                    rule
                })
                .collect(),
        };

        let results = manager
            .restore_auto_start_rules(&repository)
            .expect("load auto-start rules");

        assert_eq!(results.len(), 3);
        assert!(!results[0].ok);
        assert!(results[1].ok);
        assert!(results[2].ok);
        assert_eq!(runner.start_calls.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn restore_failure_keeps_auto_start_true_and_failed_runtime_state() {
        let runner = Arc::new(FakeRunner::default());
        runner.fail_start_for(1, "restore bind failed");
        let manager = RuntimeManager::new(runner);
        let mut auto_start_rule = rule(1);
        auto_start_rule.auto_start = true;
        let repository = FakeLifecycleRepository {
            rules: vec![auto_start_rule.clone()],
        };

        let results = manager
            .restore_auto_start_rules(&repository)
            .expect("load auto-start rules");
        let status = manager.status(auto_start_rule.id);

        assert!(auto_start_rule.auto_start);
        assert!(!results[0].ok);
        assert_eq!(status.state, RuntimeState::Failed);
        assert!(status
            .last_error
            .as_deref()
            .expect("restore error recorded")
            .contains("restore bind failed"));
    }

    #[test]
    fn app_shutdown_stops_runtime_without_changing_auto_start() {
        let runner = Arc::new(FakeRunner::default());
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        let mut auto_start_rule = rule(1);
        auto_start_rule.auto_start = true;
        persistence
            .set_auto_start(auto_start_rule.id, true)
            .expect("seed auto-start intent");
        manager
            .start(&auto_start_rule)
            .expect("start before shutdown");

        let _ = manager.on_app_exit();

        assert!(!runner.has_live_task());
        assert_eq!(
            manager.status(auto_start_rule.id).state,
            RuntimeState::Stopped
        );
        assert_eq!(persistence.value(auto_start_rule.id), Some(true));
    }

    #[test]
    fn restore_after_app_shutdown_is_rejected() {
        let runner = Arc::new(FakeRunner::default());
        let manager = RuntimeManager::new(runner.clone());
        let mut auto_start_rule = rule(1);
        auto_start_rule.auto_start = true;
        let repository = FakeLifecycleRepository {
            rules: vec![auto_start_rule],
        };

        let _ = manager.on_app_exit();
        let results = manager
            .restore_auto_start_rules(&repository)
            .expect("load auto-start rules after shutdown");

        assert_eq!(results.len(), 1);
        assert!(!results[0].ok);
        assert!(results[0]
            .error
            .as_deref()
            .expect("shutdown rejection error")
            .contains("应用正在退出"));
        assert_eq!(runner.start_calls.load(Ordering::SeqCst), 0);
        assert!(!runner.has_live_task());
    }

    #[test]
    fn manual_start_after_app_shutdown_is_rejected() {
        let runner = Arc::new(FakeRunner::default());
        let manager = RuntimeManager::new(runner.clone());
        let _ = manager.on_app_exit();
        let error = manager
            .start(&rule(1))
            .expect_err("start after shutdown must fail");

        assert!(error.contains("应用正在退出"));
        assert_eq!(runner.start_calls.load(Ordering::SeqCst), 0);
        assert!(!runner.has_live_task());
    }

    #[test]
    fn app_shutdown_reports_stop_failures_and_continues_other_rules() {
        let runner = Arc::new(FakeRunner::default());
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        persistence.set_auto_start(1, true).expect("seed rule 1 intent");
        persistence.set_auto_start(2, true).expect("seed rule 2 intent");
        manager.start(&rule(1)).expect("start rule 1");
        manager.start(&rule(2)).expect("start rule 2");
        runner.fail_stop("shutdown stop failed");

        let results = manager.on_app_exit();

        assert_eq!(runner.stop_calls.load(Ordering::SeqCst), 2);
        assert_eq!(results.len(), 2);
        let failures = results
            .iter()
            .filter(|result| !result.ok)
            .collect::<Vec<_>>();
        assert_eq!(failures.len(), 1);
        assert!(failures[0]
            .error
            .as_deref()
            .expect("shutdown stop error")
            .contains("shutdown stop failed"));
        assert_eq!(
            results
                .iter()
                .filter(|result| result.state == RuntimeState::Stopped)
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| result.state == RuntimeState::Failed)
                .count(),
            1
        );
        assert!(!runner.has_live_task());
        assert_eq!(persistence.value(1), Some(true));
        assert_eq!(persistence.value(2), Some(true));
    }

    #[test]
    fn manual_start_does_not_persist_auto_start_intent() {
        let runner = Arc::new(FakeRunner::default());
        let manager = RuntimeManager::new(runner);
        let persistence = FakePersistence::default();
        let mut manual_rule = rule(91);
        manual_rule.auto_start = false;

        manager
            .start(&manual_rule)
            .expect("manual start succeeds");

        assert_eq!(persistence.value(manual_rule.id), None);
    }

    #[test]
    fn failed_state_has_no_live_task_and_allows_update_delete() {
        let mut conn = test_conn();
        let stored_rule = repository::create_with_conn(&mut conn, write_input("失败规则"))
            .expect("create stored rule");
        let runner = Arc::new(FakeRunner::default());
        runner.fail_start_for(stored_rule.id, "runner start failed");
        let manager = RuntimeManager::new(runner.clone());
        let error = manager
            .start(&stored_rule)
            .expect_err("start fails");

        assert!(error.contains("runner start failed"));
        assert!(!runner.has_live_task());
        assert_eq!(manager.status(stored_rule.id).state, RuntimeState::Failed);
        assert!(manager.ensure_rule_mutable(stored_rule.id).is_ok());
        let updated = manager
            .with_rule_mutation(stored_rule.id, || {
                repository::update_with_conn(&conn, stored_rule.id, write_input("已更新规则"))
            })
            .expect("failed rule can update");
        assert_eq!(updated.name, "已更新规则");
        manager
            .with_rule_mutation(stored_rule.id, || {
                repository::delete_with_conn(&conn, stored_rule.id)
            })
            .expect("failed rule can delete");
    }

    #[test]
    fn observability_write_failure_keeps_forwarding_and_exposes_last_error() {
        let runner = Arc::new(FakeRunner::default());
        let observability = Arc::new(TcpObservability::default());
        runner.set_observation_source(ObservationSource::Tcp(Arc::clone(&observability)));
        let manager = RuntimeManager::with_observability_persistence(
            runner.clone(),
            Arc::new(FailingObservabilityPersistence),
        );
        let rule = rule(88);
        manager.start(&rule).expect("start rule");
        observability.accepted();

        manager.flush_observability(rule.id);
        let status = manager.status(rule.id);

        assert!(runner.has_live_task());
        assert_eq!(status.state, RuntimeState::Running);
        assert_eq!(status.last_error, None);
        assert!(status
            .last_observability_error
            .as_deref()
            .expect("observability error exposed")
            .contains("database is read-only"));
    }

    #[test]
    fn stats_get_repeated_flush_and_active_reset_do_not_double_count() {
        let mut conn = test_conn();
        let stored_rule = repository::create_with_conn(&mut conn, write_input("stats"))
            .expect("create stats rule");
        let persistence = Arc::new(MemoryObservabilityPersistence::new(conn));
        let runner = Arc::new(FakeRunner::default());
        let observability = Arc::new(TcpObservability::default());
        runner.set_observation_source(ObservationSource::Tcp(Arc::clone(&observability)));
        let manager = RuntimeManager::with_observability_persistence(runner, persistence.clone());
        manager
            .start(&stored_rule)
            .expect("start stats rule");
        observability.accepted();
        observability.transferred(10, 20);

        let first = manager.stats(stored_rule.id).expect("first stats");
        let repeated = manager.stats(stored_rule.id).expect("repeated stats");
        assert_eq!(first.event_count, 1);
        assert_eq!(repeated.event_count, 1);
        assert_eq!(repeated.upload_bytes, 10);
        assert_eq!(repeated.download_bytes, 20);

        manager.flush_observability(stored_rule.id);
        let after_flush = manager.stats(stored_rule.id).expect("stats after flush");
        assert_eq!(after_flush.event_count, 1);
        assert_eq!(after_flush.upload_bytes, 10);
        assert_eq!(after_flush.download_bytes, 20);

        observability.accepted();
        let reset = manager
            .reset_stats(stored_rule.id)
            .expect("reset active stats");
        assert_eq!(reset.event_count, 0);
        assert_eq!(manager.stats(stored_rule.id).unwrap().event_count, 0);
        manager.flush_observability(stored_rule.id);
        let log_count: i64 = persistence
            .conn
            .lock()
            .expect("lock memory database")
            .query_row(
                "SELECT COUNT(*) FROM request_forward_logs WHERE rule_id = ?1",
                [stored_rule.id],
                |row| row.get(0),
            )
            .expect("count logs after stats reset");
        assert_eq!(log_count, 2, "stats reset must not discard unflushed logs");
        observability.accepted();
        assert_eq!(manager.stats(stored_rule.id).unwrap().event_count, 1);
        manager
            .stop(&stored_rule)
            .expect("stop stats rule");
    }

    #[test]
    fn http_sensitive_headers_and_64k_truncation_reach_database() {
        let mut conn = test_conn();
        let stored_rule = repository::create_with_conn(&mut conn, write_input("http log"))
            .expect("create HTTP log rule");
        let observability = Arc::new(HttpObservability::default());
        let mut request_headers = HeaderMap::new();
        request_headers.insert("authorization", HeaderValue::from_static("Bearer secret"));
        request_headers.insert("content-type", HeaderValue::from_static("text/plain"));
        let trace = observability.accepted(
            "127.0.0.1:12345".parse().unwrap(),
            "example.com:80".into(),
            "POST".into(),
            "/items".into(),
            &request_headers,
            true,
            true,
        );
        let body_len = HTTP_BODY_PREVIEW_LIMIT + 1;
        trace.observe_request(&vec![b'a'; body_len]);
        trace.uploaded(body_len);
        let mut response_headers = HeaderMap::new();
        response_headers.insert("set-cookie", HeaderValue::from_static("session=secret"));
        response_headers.insert("content-type", HeaderValue::from_static("text/plain"));
        trace.response_started(201, &response_headers);
        trace.observe_response(&vec![b'b'; body_len]);
        trace.downloaded(body_len);
        let source = ObservationSource::Http(Arc::clone(&observability));
        let cursor = unified_next_cursor(&source.batch_since(ObservationCursor::default()));
        trace.response_completed();
        let batch = source.batch_since(cursor);
        let delta = unified_delta(&batch);
        let logs = unified_logs(batch);
        repository::persist_observability_with_conn(&mut conn, stored_rule.id, delta, &logs)
            .expect("persist captured HTTP log");

        let stored: (String, String, i64, i64, String, String, i64, i64, i64) = conn
            .query_row(
                "SELECT request_headers_json, response_headers_json,
                        request_body_truncated, response_body_truncated,
                        method, path, status_code, upload_bytes, download_bytes
                 FROM request_forward_logs WHERE rule_id = ?1",
                [stored_rule.id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .expect("read HTTP log");
        assert!(stored.0.contains("[REDACTED]"));
        assert!(!stored.0.contains("Bearer secret"));
        assert!(stored.1.contains("[REDACTED]"));
        assert_eq!((stored.2, stored.3), (1, 1));
        assert_eq!(
            (stored.4.as_str(), stored.5.as_str(), stored.6),
            ("POST", "/items", 201)
        );
        assert_eq!((stored.7, stored.8), (body_len as i64, body_len as i64,));
    }

    #[test]
    fn per_protocol_event_counts_persist_with_existing_semantics() {
        let mut conn = test_conn();
        let tcp_rule = repository::create_with_conn(&mut conn, write_input("tcp counts"))
            .expect("create TCP count rule");
        let udp_rule = repository::create_with_conn(&mut conn, write_input("udp counts"))
            .expect("create UDP count rule");
        let http_rule = repository::create_with_conn(&mut conn, write_input("http counts"))
            .expect("create HTTP count rule");

        let tcp = Arc::new(TcpObservability::default());
        tcp.accepted();
        tcp.accepted();
        let udp =
            Arc::new(crate::tools::request_forward::observability::UdpObservability::default());
        let client = "127.0.0.1:12000".parse().unwrap();
        for _ in 0..3 {
            udp.client_datagram(client, "127.0.0.1:53", None);
        }
        let http = Arc::new(HttpObservability::default());
        let headers = HeaderMap::new();
        for index in 0..4 {
            let _trace = http.accepted(
                client,
                "example.com:80".into(),
                "GET".into(),
                format!("/{index}"),
                &headers,
                false,
                false,
            );
        }

        for (rule_id, source) in [
            (tcp_rule.id, ObservationSource::Tcp(tcp)),
            (udp_rule.id, ObservationSource::Udp(udp)),
            (http_rule.id, ObservationSource::Http(http)),
        ] {
            let batch = source.batch_since(ObservationCursor::default());
            let delta = unified_delta(&batch);
            let logs = unified_logs(batch);
            repository::persist_observability_with_conn(&mut conn, rule_id, delta, &logs)
                .expect("persist protocol counters");
        }

        assert_eq!(
            repository::get_stats_with_conn(&conn, tcp_rule.id)
                .unwrap()
                .event_count,
            2
        );
        assert_eq!(
            repository::get_stats_with_conn(&conn, udp_rule.id)
                .unwrap()
                .event_count,
            3
        );
        assert_eq!(
            repository::get_stats_with_conn(&conn, http_rule.id)
                .unwrap()
                .event_count,
            4
        );
    }

    #[test]
    fn manual_start_keeps_auto_start_intent_unchanged() {
        let runner = Arc::new(FakeRunner::default());
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        persistence.set_auto_start(1, false).expect("seed manual intent");

        manager
            .start(&rule(1))
            .expect("runtime start is independent");

        assert_eq!(runner.stop_calls.load(Ordering::SeqCst), 0);
        assert!(runner.has_live_task());
        assert_eq!(manager.status(1).state, RuntimeState::Running);
        assert_eq!(persistence.value(1), Some(false));
    }

    #[test]
    fn manual_stop_keeps_auto_start_intent_unchanged() {
        let runner = Arc::new(FakeRunner::default());
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        persistence.set_auto_start(1, true).expect("seed auto-start intent");
        manager.start(&rule(1)).expect("start succeeds");

        manager
            .stop(&rule(1))
            .expect("runtime stop is independent");

        assert_eq!(runner.start_calls.load(Ordering::SeqCst), 1);
        assert!(!runner.has_live_task());
        assert_eq!(manager.status(1).state, RuntimeState::Stopped);
        assert_eq!(persistence.value(1), Some(true));
    }

    #[test]
    fn runner_stop_failure_is_terminal_without_changing_auto_start_intent() {
        let runner = Arc::new(FakeRunner::default());
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        persistence.set_auto_start(1, true).expect("seed auto-start intent");
        manager.start(&rule(1)).expect("start succeeds");
        runner.fail_stop("runner stop failed");

        let error = manager
            .stop(&rule(1))
            .expect_err("runner stop error remains visible");

        assert!(error.contains("runner stop failed"));
        assert!(!runner.has_live_task());
        assert_eq!(manager.status(1).state, RuntimeState::Failed);
        assert_eq!(persistence.value(1), Some(true));
    }

    #[test]
    fn same_rule_operations_are_serialized_and_start_is_idempotent() {
        let runner = Arc::new(FakeRunner::default());
        let manager = Arc::new(RuntimeManager::new(runner.clone()));
        let rule = rule(1);
        let (entered_start, release_start) = runner.block_next_start();

        let first_manager = manager.clone();
        let first_rule = rule.clone();
        let first = thread::spawn(move || first_manager.start(&first_rule));
        entered_start
            .recv_timeout(Duration::from_secs(1))
            .expect("first start reaches runner");

        let second_manager = manager.clone();
        let second_rule = rule.clone();
        let second =
            thread::spawn(move || second_manager.start(&second_rule));

        assert_eq!(runner.start_calls.load(Ordering::SeqCst), 1);
        release_start.send(()).expect("release first start");
        assert!(first.join().expect("join first start").is_ok());
        assert!(second.join().expect("join second start").is_ok());
        assert_eq!(runner.start_calls.load(Ordering::SeqCst), 1);
        assert_eq!(manager.status(1).state, RuntimeState::Running);
    }

    #[test]
    fn auto_start_mutation_is_serialized_with_runtime_operations() {
        let runner = Arc::new(FakeRunner::default());
        let manager = Arc::new(RuntimeManager::new(runner.clone()));
        let (entered_start, release_start) = runner.block_next_start();

        let start_manager = manager.clone();
        let start = thread::spawn(move || start_manager.start(&rule(1)));
        entered_start
            .recv_timeout(Duration::from_secs(1))
            .expect("start holds the rule lock");

        let mutation_count = Arc::new(AtomicUsize::new(0));
        let mutation_manager = manager.clone();
        let mutation_count_for_thread = mutation_count.clone();
        let mutation = thread::spawn(move || {
            mutation_manager.with_auto_start_mutation(1, || {
                mutation_count_for_thread.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        });

        assert_eq!(mutation_count.load(Ordering::SeqCst), 0);
        release_start.send(()).expect("release start");
        start.join().expect("join start").expect("start succeeds");
        mutation
            .join()
            .expect("join auto-start mutation")
            .expect("auto-start mutation succeeds");
        assert_eq!(mutation_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn batch_start_isolates_runner_failures_without_changing_auto_start() {
        let runner = Arc::new(FakeRunner::default());
        runner.fail_start_for(1, "first runner failed");
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        for id in 1..=3 {
            persistence.set_auto_start(id, false).expect("seed manual intent");
        }

        let results = manager.start_all_loaded(&[1, 2, 3], |id| Ok(rule(id)));

        assert_eq!(results.len(), 3);
        assert!(!results[0].ok);
        assert_eq!(results[0].state, RuntimeState::Failed);
        assert!(results[0]
            .error
            .as_deref()
            .expect("first error")
            .contains("first runner failed"));
        assert!(results[1].ok);
        assert_eq!(results[1].state, RuntimeState::Running);
        assert!(results[2].ok);
        assert_eq!(results[2].state, RuntimeState::Running);
        assert!(runner.has_live_task());
        assert_eq!(persistence.value(1), Some(false));
        assert_eq!(persistence.value(2), Some(false));
        assert_eq!(persistence.value(3), Some(false));
    }

    #[test]
    fn failed_stop_normalizes_to_stopped_without_changing_auto_start() {
        let runner = Arc::new(FakeRunner::default());
        runner.fail_start_for(1, "runner start failed");
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        let failed_rule = rule(1);

        manager
            .start(&failed_rule)
            .expect_err("start enters failed state");
        let status = manager
            .stop(&failed_rule)
            .expect("failed rule stop succeeds");

        assert!(!runner.has_live_task());
        assert_eq!(status.state, RuntimeState::Stopped);
        assert_eq!(persistence.value(1), None);
    }

    #[test]
    fn protocol_runner_assigns_distinct_handles_to_http_tcp_and_udp_rules() {
        let tcp_target = TcpListener::bind("127.0.0.1:0").expect("bind TCP target");
        let udp_target = UdpSocket::bind("127.0.0.1:0").expect("bind UDP target");
        let mut tcp_rule = rule(80);
        tcp_rule.listen_port = 0;
        tcp_rule.target_host = Some("127.0.0.1".into());
        tcp_rule.target_port = Some(tcp_target.local_addr().expect("read TCP target").port());
        let mut udp_rule = rule(81);
        udp_rule.protocol = ForwardProtocol::Udp;
        udp_rule.listen_port = 0;
        udp_rule.target_host = Some("127.0.0.1".into());
        udp_rule.target_port = Some(udp_target.local_addr().expect("read UDP target").port());
        let mut http_rule = rule(82);
        http_rule.protocol = ForwardProtocol::Http;
        http_rule.listen_port = 0;
        http_rule.target_url = Some(format!(
            "http://{}",
            tcp_target.local_addr().expect("read HTTP target")
        ));
        http_rule.target_host = None;
        http_rule.target_port = None;
        let runner = ProtocolRunner::default();

        let tcp_handle = runner.start(&tcp_rule).expect("start TCP rule");
        let udp_handle = runner.start(&udp_rule).expect("start UDP rule");
        let http_handle = runner.start(&http_rule).expect("start HTTP rule");
        assert_ne!(tcp_handle, udp_handle);
        assert_ne!(tcp_handle, http_handle);
        assert_ne!(udp_handle, http_handle);

        runner.stop(tcp_handle).expect("stop TCP rule");
        runner.stop(udp_handle).expect("stop UDP rule");
        runner.stop(http_handle).expect("stop HTTP rule");
    }

    #[test]
    fn protocol_runner_rejects_resolved_self_targets_before_starting_any_protocol() {
        let runner = ProtocolRunner::default();
        let free_tcp_port = || {
            let listener = TcpListener::bind("127.0.0.1:0").expect("reserve TCP port");
            listener.local_addr().expect("read TCP port").port()
        };
        let free_udp_port = || {
            let socket = UdpSocket::bind("127.0.0.1:0").expect("reserve UDP port");
            socket.local_addr().expect("read UDP port").port()
        };

        let mut http_rule = rule(90);
        http_rule.protocol = ForwardProtocol::Http;
        http_rule.listen_port = free_tcp_port();
        http_rule.target_url = Some(format!("http://localhost:{}", http_rule.listen_port));
        http_rule.target_host = None;
        http_rule.target_port = None;

        let mut tcp_rule = rule(91);
        tcp_rule.bind_host = "0.0.0.0".into();
        tcp_rule.listen_port = free_tcp_port();
        tcp_rule.target_host = Some("127.0.0.1".into());
        tcp_rule.target_port = Some(tcp_rule.listen_port);

        let mut udp_rule = rule(92);
        udp_rule.protocol = ForwardProtocol::Udp;
        udp_rule.bind_host = "0.0.0.0".into();
        udp_rule.listen_port = free_udp_port();
        udp_rule.target_host = Some("127.0.0.1".into());
        udp_rule.target_port = Some(udp_rule.listen_port);

        for rule in [&http_rule, &tcp_rule, &udp_rule] {
            match runner.start(rule) {
                Err(error) => assert!(error.contains("不能直接转发到自身")),
                Ok(handle) => {
                    runner.stop(handle).expect("cleanup unexpectedly started rule");
                    panic!("{} self target unexpectedly started", rule.protocol.as_str());
                }
            }
        }
    }

    #[test]
    fn protocol_runner_accepts_https_downstream() {
        let mut https_rule = rule(93);
        https_rule.protocol = ForwardProtocol::Http;
        https_rule.listen_port = 0;
        https_rule.target_url = Some("https://example.com/api".into());
        https_rule.target_host = None;
        https_rule.target_port = None;

        let runner = ProtocolRunner::default();
        let handle = runner
            .start(&https_rule)
            .expect("HTTPS downstream starts without an early protocol rejection");
        runner.stop(handle).expect("stop HTTPS downstream rule");
    }
}
