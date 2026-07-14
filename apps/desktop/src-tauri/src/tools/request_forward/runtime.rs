use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;

use super::model::{ForwardProtocol, ForwardRule};
use super::tcp::TcpRuleRunner;

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
    fn stop(&self, handle: RunningHandle) -> Result<(), String>;
}

pub(crate) trait AutoStartPersistence {
    fn set_auto_start(&self, rule_id: i64, value: bool) -> Result<(), String>;
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

    fn status(&self, rule_id: i64) -> RuntimeStatus {
        RuntimeStatus {
            rule_id,
            state: self.state,
            last_error: self.last_error.clone(),
        }
    }
}

pub(crate) struct RuntimeManager {
    runner: Arc<dyn RuleRunner>,
    instances: Mutex<HashMap<i64, RuntimeInstance>>,
    rule_locks: Mutex<HashMap<i64, Arc<Mutex<()>>>>,
}

impl RuntimeManager {
    pub(super) fn new(runner: Arc<dyn RuleRunner>) -> Self {
        Self {
            runner,
            instances: Mutex::new(HashMap::new()),
            rule_locks: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn start<P: AutoStartPersistence>(
        &self,
        rule: &ForwardRule,
        persistence: &P,
    ) -> Result<RuntimeStatus, String> {
        self.with_rule_lock(rule.id, || self.start_locked(rule, persistence))
    }

    pub(crate) fn start_loaded<P: AutoStartPersistence>(
        &self,
        rule_id: i64,
        persistence: &P,
        load_rule: impl FnOnce() -> Result<ForwardRule, String>,
    ) -> Result<RuntimeStatus, String> {
        self.with_rule_lock(rule_id, || {
            let rule = load_rule()?;
            if rule.id != rule_id {
                return Err("转发规则 ID 不匹配".into());
            }
            self.start_locked(&rule, persistence)
        })
    }

    pub(crate) fn stop<P: AutoStartPersistence>(
        &self,
        rule: &ForwardRule,
        persistence: &P,
    ) -> Result<RuntimeStatus, String> {
        self.with_rule_lock(rule.id, || self.stop_locked(rule, persistence))
    }

    pub(crate) fn stop_loaded<P: AutoStartPersistence>(
        &self,
        rule_id: i64,
        persistence: &P,
        load_rule: impl FnOnce() -> Result<ForwardRule, String>,
    ) -> Result<RuntimeStatus, String> {
        self.with_rule_lock(rule_id, || {
            let rule = load_rule()?;
            if rule.id != rule_id {
                return Err("转发规则 ID 不匹配".into());
            }
            self.stop_locked(&rule, persistence)
        })
    }

    pub(crate) fn start_all_loaded<P: AutoStartPersistence>(
        &self,
        rule_ids: &[i64],
        persistence: &P,
        load_rule: impl Fn(i64) -> Result<ForwardRule, String>,
    ) -> Vec<BatchOperationResult> {
        rule_ids
            .iter()
            .map(|&rule_id| {
                self.batch_result(
                    rule_id,
                    self.start_loaded(rule_id, persistence, || load_rule(rule_id)),
                )
            })
            .collect()
    }

    pub(crate) fn stop_all_loaded<P: AutoStartPersistence>(
        &self,
        rule_ids: &[i64],
        persistence: &P,
        load_rule: impl Fn(i64) -> Result<ForwardRule, String>,
    ) -> Vec<BatchOperationResult> {
        rule_ids
            .iter()
            .map(|&rule_id| {
                self.batch_result(
                    rule_id,
                    self.stop_loaded(rule_id, persistence, || load_rule(rule_id)),
                )
            })
            .collect()
    }

    pub(crate) fn status(&self, rule_id: i64) -> RuntimeStatus {
        self.instances
            .lock()
            .expect("request-forward instances lock poisoned")
            .get(&rule_id)
            .map(|instance| instance.status(rule_id))
            .unwrap_or_else(|| RuntimeInstance::stopped().status(rule_id))
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

    pub(crate) fn clear_rule_state(&self, rule_id: i64) {
        self.instances
            .lock()
            .expect("request-forward instances lock poisoned")
            .remove(&rule_id);
    }

    fn start_locked<P: AutoStartPersistence>(
        &self,
        rule: &ForwardRule,
        persistence: &P,
    ) -> Result<RuntimeStatus, String> {
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

        if let Err(primary_error) = persistence.set_auto_start(rule.id, true) {
            self.set_state(
                rule.id,
                RuntimeState::Stopping,
                Some(handle),
                Some(primary_error.clone()),
            );
            match self.runner.stop(handle) {
                Ok(()) => {
                    self.set_failed(rule.id, primary_error.clone());
                    return Err(primary_error);
                }
                Err(compensation_error) => {
                    let error = compensation_error_message(&primary_error, &compensation_error);
                    self.set_state(
                        rule.id,
                        RuntimeState::Running,
                        Some(handle),
                        Some(error.clone()),
                    );
                    return Err(error);
                }
            }
        }

        Ok(self.status(rule.id))
    }

    fn stop_locked<P: AutoStartPersistence>(
        &self,
        rule: &ForwardRule,
        persistence: &P,
    ) -> Result<RuntimeStatus, String> {
        let previous = self.instance(rule.id);

        if previous.state == RuntimeState::Failed {
            if let Err(error) = persistence.set_auto_start(rule.id, false) {
                self.set_failed(rule.id, error.clone());
                return Err(error);
            }
            self.set_instance(rule.id, RuntimeInstance::stopped());
            return Ok(self.status(rule.id));
        }

        let had_running_handle = previous.state == RuntimeState::Running;

        if had_running_handle {
            let handle = previous.handle.expect("running state must have a handle");
            self.set_state(rule.id, RuntimeState::Stopping, Some(handle), None);
            if let Err(error) = self.runner.stop(handle) {
                self.set_state(
                    rule.id,
                    RuntimeState::Running,
                    Some(handle),
                    Some(error.clone()),
                );
                return Err(error);
            }
            self.set_instance(rule.id, RuntimeInstance::stopped());
        }

        if !rule.auto_start && !had_running_handle {
            return Ok(self.status(rule.id));
        }

        if let Err(primary_error) = persistence.set_auto_start(rule.id, false) {
            self.set_instance(
                rule.id,
                RuntimeInstance {
                    state: RuntimeState::Starting,
                    handle: None,
                    last_error: Some(primary_error.clone()),
                },
            );
            match self.runner.start(rule) {
                Ok(handle) => {
                    self.set_state(
                        rule.id,
                        RuntimeState::Running,
                        Some(handle),
                        Some(primary_error.clone()),
                    );
                    return Err(primary_error);
                }
                Err(compensation_error) => {
                    let error = compensation_error_message(&primary_error, &compensation_error);
                    self.set_failed(rule.id, error.clone());
                    return Err(error);
                }
            }
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
            Err(error) => BatchOperationResult {
                rule_id,
                ok: false,
                error: Some(error),
                state: self.status(rule_id).state,
            },
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
}

fn compensation_error_message(primary_error: &str, compensation_error: &str) -> String {
    format!("{primary_error}; 补偿失败: {compensation_error}")
}

struct ProtocolRunner {
    tcp: TcpRuleRunner,
}

impl Default for ProtocolRunner {
    fn default() -> Self {
        Self {
            tcp: TcpRuleRunner::new(),
        }
    }
}

impl RuleRunner for ProtocolRunner {
    fn start(&self, rule: &ForwardRule) -> Result<RunningHandle, String> {
        match rule.protocol {
            ForwardProtocol::Tcp => self.tcp.start(rule),
            ForwardProtocol::Http | ForwardProtocol::Udp => Err("协议转发运行器尚未安装".into()),
        }
    }

    fn stop(&self, handle: RunningHandle) -> Result<(), String> {
        self.tcp.stop(handle)
    }
}

static RUNTIME_MANAGER: OnceLock<RuntimeManager> = OnceLock::new();

pub(crate) fn global_manager() -> &'static RuntimeManager {
    RUNTIME_MANAGER.get_or_init(|| RuntimeManager::new(Arc::new(ProtocolRunner::default())))
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use super::{AutoStartPersistence, RuleRunner, RunningHandle, RuntimeManager, RuntimeState};
    use crate::tools::helpers::ensure_request_forward_schema_for_test;
    use crate::tools::request_forward::model::{ForwardProtocol, ForwardRule, RuleWriteInput};
    use crate::tools::request_forward::repository;
    use rusqlite::Connection;

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
            if let Some(error) = state.stop_error.clone() {
                return Err(error);
            }
            state.live_handles.remove(&handle.0);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakePersistence {
        errors: Mutex<HashMap<(i64, bool), String>>,
        values: Mutex<HashMap<i64, bool>>,
    }

    impl FakePersistence {
        fn fail_for(&self, rule_id: i64, value: bool, error: &str) {
            self.errors
                .lock()
                .expect("lock fake persistence")
                .insert((rule_id, value), error.into());
        }

        fn value(&self, rule_id: i64) -> Option<bool> {
            self.values
                .lock()
                .expect("lock fake persistence")
                .get(&rule_id)
                .copied()
        }
    }

    impl AutoStartPersistence for FakePersistence {
        fn set_auto_start(&self, rule_id: i64, value: bool) -> Result<(), String> {
            if let Some(error) = self
                .errors
                .lock()
                .expect("lock fake persistence")
                .get(&(rule_id, value))
            {
                return Err(error.clone());
            }
            self.values
                .lock()
                .expect("lock fake persistence")
                .insert(rule_id, value);
            Ok(())
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
    fn failed_state_has_no_live_task_and_allows_update_delete() {
        let mut conn = test_conn();
        let stored_rule = repository::create_with_conn(&mut conn, write_input("失败规则"))
            .expect("create stored rule");
        let runner = Arc::new(FakeRunner::default());
        runner.fail_start_for(stored_rule.id, "runner start failed");
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();

        let error = manager
            .start(&stored_rule, &persistence)
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
    fn start_persist_failure_stops_new_runtime_before_returning_error() {
        let runner = Arc::new(FakeRunner::default());
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        persistence.fail_for(1, true, "persist true failed");

        let error = manager
            .start(&rule(1), &persistence)
            .expect_err("start fails");

        assert!(error.contains("persist true failed"));
        assert_eq!(runner.stop_calls.load(Ordering::SeqCst), 1);
        assert!(!runner.has_live_task());
        assert_eq!(manager.status(1).state, RuntimeState::Failed);
        assert_eq!(persistence.value(1), None);
    }

    #[test]
    fn stop_persist_failure_restarts_old_config_before_returning_error() {
        let runner = Arc::new(FakeRunner::default());
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        manager
            .start(&rule(1), &persistence)
            .expect("start succeeds");
        persistence.fail_for(1, false, "persist false failed");

        let error = manager
            .stop(&rule(1), &persistence)
            .expect_err("stop fails");

        assert!(error.contains("persist false failed"));
        assert_eq!(runner.start_calls.load(Ordering::SeqCst), 2);
        assert!(runner.has_live_task());
        assert_eq!(manager.status(1).state, RuntimeState::Running);
        assert_eq!(persistence.value(1), Some(true));
    }

    #[test]
    fn double_compensation_failure_reports_runtime_truth() {
        let runner = Arc::new(FakeRunner::default());
        runner.fail_stop("compensation stop failed");
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        persistence.fail_for(1, true, "persist true failed");

        let error = manager
            .start(&rule(1), &persistence)
            .expect_err("start fails");
        let status = manager.status(1);

        assert!(error.contains("persist true failed"));
        assert!(error.contains("compensation stop failed"));
        assert!(runner.has_live_task());
        assert_eq!(status.state, RuntimeState::Running);
        assert!(status
            .last_error
            .expect("error recorded")
            .contains("compensation stop failed"));
    }

    #[test]
    fn same_rule_operations_are_serialized_and_start_is_idempotent() {
        let runner = Arc::new(FakeRunner::default());
        let manager = Arc::new(RuntimeManager::new(runner.clone()));
        let persistence = Arc::new(FakePersistence::default());
        let rule = rule(1);
        let (entered_start, release_start) = runner.block_next_start();

        let first_manager = manager.clone();
        let first_persistence = persistence.clone();
        let first_rule = rule.clone();
        let first = thread::spawn(move || first_manager.start(&first_rule, &*first_persistence));
        entered_start
            .recv_timeout(Duration::from_secs(1))
            .expect("first start reaches runner");

        let second_manager = manager.clone();
        let second_persistence = persistence.clone();
        let second_rule = rule.clone();
        let second =
            thread::spawn(move || second_manager.start(&second_rule, &*second_persistence));

        assert_eq!(runner.start_calls.load(Ordering::SeqCst), 1);
        release_start.send(()).expect("release first start");
        assert!(first.join().expect("join first start").is_ok());
        assert!(second.join().expect("join second start").is_ok());
        assert_eq!(runner.start_calls.load(Ordering::SeqCst), 1);
        assert_eq!(manager.status(1).state, RuntimeState::Running);
    }

    #[test]
    fn batch_start_isolates_each_rule_and_never_claims_compensation_success() {
        let runner = Arc::new(FakeRunner::default());
        runner.fail_start_for(1, "first runner failed");
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        persistence.fail_for(2, true, "second persist failed");

        let results = manager.start_all_loaded(&[1, 2, 3], &persistence, |id| Ok(rule(id)));

        assert_eq!(results.len(), 3);
        assert!(!results[0].ok);
        assert_eq!(results[0].state, RuntimeState::Failed);
        assert!(results[0]
            .error
            .as_deref()
            .expect("first error")
            .contains("first runner failed"));
        assert!(!results[1].ok);
        assert_eq!(results[1].state, RuntimeState::Failed);
        assert!(results[1]
            .error
            .as_deref()
            .expect("second error")
            .contains("second persist failed"));
        assert!(results[2].ok);
        assert_eq!(results[2].state, RuntimeState::Running);
        assert!(runner.has_live_task());
    }

    #[test]
    fn failed_stop_normalizes_to_stopped_and_persists_false() {
        let runner = Arc::new(FakeRunner::default());
        runner.fail_start_for(1, "runner start failed");
        let manager = RuntimeManager::new(runner.clone());
        let persistence = FakePersistence::default();
        let failed_rule = rule(1);

        manager
            .start(&failed_rule, &persistence)
            .expect_err("start enters failed state");
        let status = manager
            .stop(&failed_rule, &persistence)
            .expect("failed rule stop succeeds");

        assert!(!runner.has_live_task());
        assert_eq!(status.state, RuntimeState::Stopped);
        assert_eq!(persistence.value(1), Some(false));
    }

    #[test]
    fn failed_stop_persist_failure_returns_error() {
        let runner = Arc::new(FakeRunner::default());
        runner.fail_start_for(1, "runner start failed");
        let manager = RuntimeManager::new(runner);
        let persistence = FakePersistence::default();
        let failed_rule = rule(1);
        manager
            .start(&failed_rule, &persistence)
            .expect_err("start enters failed state");
        persistence.fail_for(1, false, "persist false failed");

        let error = manager
            .stop(&failed_rule, &persistence)
            .expect_err("failed stop persistence error propagates");

        assert!(error.contains("persist false failed"));
        assert_eq!(manager.status(1).state, RuntimeState::Failed);
        assert_eq!(persistence.value(1), None);
    }
}
