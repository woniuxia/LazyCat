use super::{
    atomic_actions::{normalize_atomic_failure, AtomicActionExecutor, AtomicStepSuccessStatus},
    combinations::ExecutionMode,
};
use std::{
    any::Any,
    io,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::Arc,
    thread::{self, JoinHandle},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlannedStep {
    pub run_step_id: i64,
    pub action_type: String,
    pub target_id: String,
    pub sort_order: i64,
    pub validation_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StepTerminalStatus {
    Succeeded,
    AlreadySatisfied,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecutedStep {
    pub run_step_id: i64,
    pub sort_order: i64,
    pub status: StepTerminalStatus,
    pub result_code: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RunTerminalStatus {
    Succeeded,
    PartiallySucceeded,
    Failed,
}

pub(crate) trait ExecutionObserver: Send + Sync + 'static {
    fn step_started(&self, run_step_id: i64);
    fn step_finished(&self, result: &ExecutedStep);
}

struct NoopExecutionObserver;

impl ExecutionObserver for NoopExecutionObserver {
    fn step_started(&self, _run_step_id: i64) {}

    fn step_finished(&self, _result: &ExecutedStep) {}
}

pub(crate) fn execute_plan(
    mode: ExecutionMode,
    steps: Vec<PlannedStep>,
    executor: Arc<dyn AtomicActionExecutor>,
) -> Vec<ExecutedStep> {
    execute_plan_with_observer(mode, steps, executor, Arc::new(NoopExecutionObserver))
}

pub(crate) fn execute_plan_with_observer(
    mode: ExecutionMode,
    steps: Vec<PlannedStep>,
    executor: Arc<dyn AtomicActionExecutor>,
    observer: Arc<dyn ExecutionObserver>,
) -> Vec<ExecutedStep> {
    match mode {
        ExecutionMode::Serial => steps
            .into_iter()
            .map(|step| execute_step(step, &executor, &observer))
            .collect(),
        ExecutionMode::Parallel => execute_parallel(steps, executor, observer),
    }
}

fn execute_parallel(
    steps: Vec<PlannedStep>,
    executor: Arc<dyn AtomicActionExecutor>,
    observer: Arc<dyn ExecutionObserver>,
) -> Vec<ExecutedStep> {
    execute_parallel_with_spawner(steps, executor, observer, |task| {
        thread::Builder::new().spawn(task)
    })
}

type WorkerTask = Box<dyn FnOnce() -> ExecutedStep + Send>;

fn execute_parallel_with_spawner<S>(
    steps: Vec<PlannedStep>,
    executor: Arc<dyn AtomicActionExecutor>,
    observer: Arc<dyn ExecutionObserver>,
    mut spawner: S,
) -> Vec<ExecutedStep>
where
    S: FnMut(WorkerTask) -> io::Result<JoinHandle<ExecutedStep>>,
{
    let mut workers = Vec::with_capacity(steps.len());
    let mut results = Vec::with_capacity(steps.len());
    for step in steps {
        let fallback_step = step.clone();
        let executor = executor.clone();
        let worker_observer = observer.clone();
        let task: WorkerTask = Box::new(move || execute_step(step, &executor, &worker_observer));
        match spawn_worker(fallback_step, task, &mut spawner, &observer) {
            Ok(worker) => workers.push(worker),
            Err(result) => results.push(result),
        }
    }

    results.extend(
        workers
            .into_iter()
            .map(|(step, worker)| join_worker(step, worker, &observer)),
    );
    results.sort_by_key(|result| (result.sort_order, result.run_step_id));
    results
}

fn spawn_worker<S>(
    step: PlannedStep,
    task: WorkerTask,
    spawner: &mut S,
    observer: &Arc<dyn ExecutionObserver>,
) -> Result<(PlannedStep, JoinHandle<ExecutedStep>), ExecutedStep>
where
    S: FnMut(WorkerTask) -> io::Result<JoinHandle<ExecutedStep>>,
{
    match spawner(task) {
        Ok(worker) => Ok((step, worker)),
        Err(error) => {
            let result = failed_step(
                &step,
                format!("failed to spawn parallel step worker: {error}"),
            );
            notify_step_finished(observer, &result);
            Err(result)
        }
    }
}

fn join_worker(
    step: PlannedStep,
    worker: std::thread::JoinHandle<ExecutedStep>,
    observer: &Arc<dyn ExecutionObserver>,
) -> ExecutedStep {
    match worker.join() {
        Ok(result) => result,
        Err(payload) => {
            let result = failed_step(
                &step,
                format!("parallel step worker panicked: {}", panic_message(payload)),
            );
            notify_step_finished(observer, &result);
            result
        }
    }
}

fn execute_step(
    step: PlannedStep,
    executor: &Arc<dyn AtomicActionExecutor>,
    observer: &Arc<dyn ExecutionObserver>,
) -> ExecutedStep {
    let result = match catch_unwind(AssertUnwindSafe(|| observer.step_started(step.run_step_id))) {
        Ok(()) => match &step.validation_error {
            Some(error) => failed_step(&step, error.clone()),
            None => execute_atomic_step(&step, executor),
        },
        Err(payload) => failed_step(
            &step,
            format!(
                "execution observer step_started panicked: {}",
                panic_message(payload)
            ),
        ),
    };
    notify_step_finished(observer, &result);
    result
}

fn notify_step_finished(observer: &Arc<dyn ExecutionObserver>, result: &ExecutedStep) {
    let _ = catch_unwind(AssertUnwindSafe(|| observer.step_finished(result)));
}

fn execute_atomic_step(
    step: &PlannedStep,
    executor: &Arc<dyn AtomicActionExecutor>,
) -> ExecutedStep {
    match catch_unwind(AssertUnwindSafe(|| {
        executor.execute(&step.action_type, &step.target_id)
    })) {
        Ok(Ok(success)) => ExecutedStep {
            run_step_id: step.run_step_id,
            sort_order: step.sort_order,
            status: match success.status {
                AtomicStepSuccessStatus::Succeeded => StepTerminalStatus::Succeeded,
                AtomicStepSuccessStatus::AlreadySatisfied => StepTerminalStatus::AlreadySatisfied,
            },
            result_code: success.result_code,
            message: success.message,
        },
        Ok(Err(error)) => {
            let (result_code, message) = normalize_atomic_failure(&step.action_type, error);
            failed_step_with_result_code(step, result_code, message)
        }
        Err(payload) => failed_step(
            step,
            format!(
                "atomic action executor panicked: {}",
                panic_message(payload)
            ),
        ),
    }
}

fn failed_step(step: &PlannedStep, message: String) -> ExecutedStep {
    failed_step_with_result_code(step, None, message)
}

fn failed_step_with_result_code(
    step: &PlannedStep,
    result_code: Option<String>,
    message: String,
) -> ExecutedStep {
    ExecutedStep {
        run_step_id: step.run_step_id,
        sort_order: step.sort_order,
        status: StepTerminalStatus::Failed,
        result_code,
        message: Some(message),
    }
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".into()
    }
}

pub(crate) fn aggregate_status(results: &[ExecutedStep]) -> RunTerminalStatus {
    if results.is_empty() {
        return RunTerminalStatus::Failed;
    }

    let failed_count = results
        .iter()
        .filter(|result| result.status == StepTerminalStatus::Failed)
        .count();
    match failed_count {
        0 => RunTerminalStatus::Succeeded,
        count if count == results.len() => RunTerminalStatus::Failed,
        _ => RunTerminalStatus::PartiallySucceeded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::action_center::{
        atomic_actions::{AtomicActionExecutor, AtomicStepSuccess, AtomicStepSuccessStatus},
        combinations::ExecutionMode,
    };
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
            mpsc::{self, Sender},
            Arc, Condvar, Mutex,
        },
        time::Duration,
    };

    #[derive(Clone)]
    enum Behavior {
        Success(AtomicStepSuccess),
        Error(String),
        Panic(&'static str),
    }

    struct ParallelGate {
        open: Mutex<bool>,
        changed: Condvar,
    }

    impl ParallelGate {
        fn closed() -> Self {
            Self {
                open: Mutex::new(false),
                changed: Condvar::new(),
            }
        }

        fn wait(&self) {
            let mut open = self.open.lock().unwrap();
            while !*open {
                open = self.changed.wait(open).unwrap();
            }
        }

        fn open(&self) {
            *self.open.lock().unwrap() = true;
            self.changed.notify_all();
        }
    }

    struct ScriptedExecutor {
        behaviors: HashMap<String, Behavior>,
        calls: Arc<Mutex<Vec<String>>>,
        entered: Option<Sender<String>>,
        gate: Option<Arc<ParallelGate>>,
    }

    impl ScriptedExecutor {
        fn new(behaviors: impl IntoIterator<Item = (&'static str, Behavior)>) -> Self {
            Self {
                behaviors: behaviors
                    .into_iter()
                    .map(|(target_id, behavior)| (target_id.to_string(), behavior))
                    .collect(),
                calls: Arc::new(Mutex::new(Vec::new())),
                entered: None,
                gate: None,
            }
        }

        fn with_parallel_probe(mut self, entered: Sender<String>, gate: Arc<ParallelGate>) -> Self {
            self.entered = Some(entered);
            self.gate = Some(gate);
            self
        }
    }

    impl AtomicActionExecutor for ScriptedExecutor {
        fn execute(
            &self,
            _action_type: &str,
            target_id: &str,
        ) -> Result<AtomicStepSuccess, String> {
            self.calls.lock().unwrap().push(target_id.to_string());
            if let Some(entered) = &self.entered {
                entered.send(target_id.to_string()).unwrap();
            }
            if let Some(gate) = &self.gate {
                gate.wait();
            }

            match self.behaviors.get(target_id).unwrap().clone() {
                Behavior::Success(result) => Ok(result),
                Behavior::Error(error) => Err(error),
                Behavior::Panic(message) => panic!("{message}"),
            }
        }
    }

    fn succeeded(result_code: Option<&str>, message: Option<&str>) -> Behavior {
        Behavior::Success(AtomicStepSuccess {
            status: AtomicStepSuccessStatus::Succeeded,
            result_code: result_code.map(str::to_string),
            message: message.map(str::to_string),
        })
    }

    fn already_satisfied(message: Option<&str>) -> Behavior {
        Behavior::Success(AtomicStepSuccess {
            status: AtomicStepSuccessStatus::AlreadySatisfied,
            result_code: None,
            message: message.map(str::to_string),
        })
    }

    fn step(run_step_id: i64, target_id: &str, sort_order: i64) -> PlannedStep {
        PlannedStep {
            run_step_id,
            action_type: "test.action".into(),
            target_id: target_id.into(),
            sort_order,
            validation_error: None,
        }
    }

    fn result(run_step_id: i64, sort_order: i64, status: StepTerminalStatus) -> ExecutedStep {
        ExecutedStep {
            run_step_id,
            sort_order,
            status,
            result_code: None,
            message: None,
        }
    }

    #[test]
    fn serial_keeps_order_continues_after_failure_and_aggregates_partial_success() {
        let executor = Arc::new(ScriptedExecutor::new([
            ("a", succeeded(Some("created"), Some("a done"))),
            ("b", Behavior::Error("b failed".into())),
            ("c", already_satisfied(Some("c unchanged"))),
        ]));
        let calls = executor.calls.clone();

        let results = execute_plan(
            ExecutionMode::Serial,
            vec![step(10, "a", 0), step(11, "b", 1), step(12, "c", 2)],
            executor,
        );

        assert_eq!(*calls.lock().unwrap(), ["a", "b", "c"]);
        assert_eq!(
            results,
            vec![
                ExecutedStep {
                    run_step_id: 10,
                    sort_order: 0,
                    status: StepTerminalStatus::Succeeded,
                    result_code: Some("created".into()),
                    message: Some("a done".into()),
                },
                ExecutedStep {
                    run_step_id: 11,
                    sort_order: 1,
                    status: StepTerminalStatus::Failed,
                    result_code: None,
                    message: Some("b failed".into()),
                },
                ExecutedStep {
                    run_step_id: 12,
                    sort_order: 2,
                    status: StepTerminalStatus::AlreadySatisfied,
                    result_code: None,
                    message: Some("c unchanged".into()),
                },
            ]
        );
        assert_eq!(
            aggregate_status(&results),
            RunTerminalStatus::PartiallySucceeded
        );
    }

    #[test]
    fn request_forward_failure_keeps_stable_result_code_and_original_message() {
        let message = "TCP 监听绑定失败: address already in use";
        let error = crate::tools::request_forward::encode_action_error(message, "failed");
        let executor = Arc::new(ScriptedExecutor::new([("forward", Behavior::Error(error))]));
        let mut forward_step = step(10, "forward", 0);
        forward_step.action_type =
            crate::tools::action_center::definitions::REQUEST_FORWARD_START.into();

        let results = execute_plan(ExecutionMode::Serial, vec![forward_step], executor);

        assert_eq!(results[0].status, StepTerminalStatus::Failed);
        assert_eq!(results[0].result_code.as_deref(), Some("listener_in_use"));
        assert_eq!(results[0].message.as_deref(), Some(message));
    }

    #[test]
    fn parallel_workers_overlap_but_results_are_sorted() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let gate = Arc::new(ParallelGate::closed());
        let executor = Arc::new(
            ScriptedExecutor::new([
                ("a", succeeded(None, None)),
                ("b", succeeded(None, None)),
                ("c", succeeded(None, None)),
                ("d", succeeded(None, None)),
            ])
            .with_parallel_probe(entered_tx, gate.clone()),
        );

        let execution = std::thread::spawn(move || {
            execute_plan(
                ExecutionMode::Parallel,
                vec![
                    step(12, "c", 2),
                    step(10, "a", 0),
                    step(11, "b", 1),
                    step(9, "d", 1),
                ],
                executor,
            )
        });

        let entered = (0..4)
            .map(|_| entered_rx.recv_timeout(Duration::from_secs(5)))
            .collect::<Result<Vec<_>, _>>();
        gate.open();
        let results = execution.join().unwrap();

        assert_eq!(entered.unwrap().len(), 4);
        assert_eq!(
            results
                .iter()
                .map(|item| (item.sort_order, item.run_step_id))
                .collect::<Vec<_>>(),
            vec![(0, 10), (1, 9), (1, 11), (2, 12)]
        );
    }

    #[test]
    fn parallel_executor_panic_fails_only_that_step() {
        let executor = Arc::new(ScriptedExecutor::new([
            ("ok", succeeded(None, None)),
            ("panic", Behavior::Panic("worker exploded")),
        ]));

        let results = execute_plan(
            ExecutionMode::Parallel,
            vec![step(1, "ok", 0), step(2, "panic", 1)],
            executor,
        );

        assert_eq!(results[0].status, StepTerminalStatus::Succeeded);
        assert_eq!(results[1].status, StepTerminalStatus::Failed);
        assert!(results[1]
            .message
            .as_deref()
            .unwrap()
            .contains("worker exploded"));
    }

    #[test]
    fn join_worker_panic_returns_failed_and_notifies_finished_once() {
        let finished_calls = Arc::new(AtomicUsize::new(0));
        let observer: Arc<dyn ExecutionObserver> = Arc::new(PanicOnFinishObserver {
            finished_calls: finished_calls.clone(),
        });
        let worker = std::thread::spawn(|| -> ExecutedStep {
            panic!("join worker exploded");
        });

        let result = join_worker(step(7, "panic", 3), worker, &observer);

        assert_eq!(result.run_step_id, 7);
        assert_eq!(result.sort_order, 3);
        assert_eq!(result.status, StepTerminalStatus::Failed);
        assert!(result
            .message
            .as_deref()
            .unwrap()
            .contains("join worker exploded"));
        assert_eq!(finished_calls.load(Ordering::SeqCst), 1);
    }

    struct RecordingPanicOnFinishObserver {
        finished: Arc<Mutex<Vec<(i64, StepTerminalStatus)>>>,
    }

    impl ExecutionObserver for RecordingPanicOnFinishObserver {
        fn step_started(&self, _run_step_id: i64) {}

        fn step_finished(&self, result: &ExecutedStep) {
            self.finished
                .lock()
                .unwrap()
                .push((result.run_step_id, result.status));
            panic!("finished observer exploded");
        }
    }

    #[test]
    fn parallel_spawn_failure_fails_step_notifies_once_and_joins_created_worker() {
        let finished = Arc::new(Mutex::new(Vec::new()));
        let executor = Arc::new(ScriptedExecutor::new([
            ("created", succeeded(Some("joined"), None)),
            ("not-spawned", succeeded(None, None)),
        ]));
        let calls = executor.calls.clone();
        let mut spawn_attempt = 0;

        let results = execute_parallel_with_spawner(
            vec![step(1, "created", 0), step(2, "not-spawned", 1)],
            executor,
            Arc::new(RecordingPanicOnFinishObserver {
                finished: finished.clone(),
            }),
            move |task| {
                spawn_attempt += 1;
                if spawn_attempt == 2 {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "simulated worker exhaustion",
                    ))
                } else {
                    std::thread::Builder::new().spawn(task)
                }
            },
        );

        assert_eq!(*calls.lock().unwrap(), ["created"]);
        assert_eq!(results[0].status, StepTerminalStatus::Succeeded);
        assert_eq!(results[0].result_code.as_deref(), Some("joined"));
        assert_eq!(results[1].status, StepTerminalStatus::Failed);
        assert!(results[1]
            .message
            .as_deref()
            .unwrap()
            .contains("simulated worker exhaustion"));

        let finished = finished.lock().unwrap();
        assert_eq!(finished.len(), 2);
        assert_eq!(
            finished
                .iter()
                .filter(|event| **event == (2, StepTerminalStatus::Failed))
                .count(),
            1
        );
    }

    struct PanicOnStartObserver {
        panic_run_step_id: i64,
        finished: Arc<Mutex<Vec<(i64, StepTerminalStatus)>>>,
    }

    impl ExecutionObserver for PanicOnStartObserver {
        fn step_started(&self, run_step_id: i64) {
            if run_step_id == self.panic_run_step_id {
                panic!("observer exploded");
            }
        }

        fn step_finished(&self, result: &ExecutedStep) {
            self.finished
                .lock()
                .unwrap()
                .push((result.run_step_id, result.status));
        }
    }

    #[test]
    fn serial_started_observer_panic_fails_that_step_notifies_finished_and_continues() {
        let finished = Arc::new(Mutex::new(Vec::new()));
        let executor = Arc::new(ScriptedExecutor::new([
            ("skipped", succeeded(None, None)),
            ("ok", succeeded(None, None)),
        ]));
        let calls = executor.calls.clone();

        let results = execute_plan_with_observer(
            ExecutionMode::Serial,
            vec![step(1, "skipped", 0), step(2, "ok", 1)],
            executor,
            Arc::new(PanicOnStartObserver {
                panic_run_step_id: 1,
                finished: finished.clone(),
            }),
        );

        assert_eq!(*calls.lock().unwrap(), ["ok"]);
        assert_eq!(results[0].status, StepTerminalStatus::Failed);
        assert!(results[0]
            .message
            .as_deref()
            .unwrap()
            .contains("observer exploded"));
        assert_eq!(results[1].status, StepTerminalStatus::Succeeded);
        assert_eq!(
            *finished.lock().unwrap(),
            [
                (1, StepTerminalStatus::Failed),
                (2, StepTerminalStatus::Succeeded),
            ]
        );
    }

    #[test]
    fn parallel_started_observer_panic_fails_that_step_and_notifies_finished() {
        let finished = Arc::new(Mutex::new(Vec::new()));
        let executor = Arc::new(ScriptedExecutor::new([
            ("skipped", succeeded(None, None)),
            ("ok", succeeded(None, None)),
        ]));

        let results = execute_plan_with_observer(
            ExecutionMode::Parallel,
            vec![step(1, "skipped", 0), step(2, "ok", 1)],
            executor,
            Arc::new(PanicOnStartObserver {
                panic_run_step_id: 1,
                finished: finished.clone(),
            }),
        );

        assert_eq!(results[0].status, StepTerminalStatus::Failed);
        assert!(results[0]
            .message
            .as_deref()
            .unwrap()
            .contains("observer exploded"));
        assert_eq!(results[1].status, StepTerminalStatus::Succeeded);
        let finished = finished.lock().unwrap();
        assert!(finished.contains(&(1, StepTerminalStatus::Failed)));
        assert!(finished.contains(&(2, StepTerminalStatus::Succeeded)));
    }

    struct PanicOnFinishObserver {
        finished_calls: Arc<AtomicUsize>,
    }

    impl ExecutionObserver for PanicOnFinishObserver {
        fn step_started(&self, _run_step_id: i64) {}

        fn step_finished(&self, _result: &ExecutedStep) {
            self.finished_calls.fetch_add(1, Ordering::SeqCst);
            panic!("finished observer exploded");
        }
    }

    fn assert_finished_observer_panic_does_not_change_results(mode: ExecutionMode) {
        let finished_calls = Arc::new(AtomicUsize::new(0));
        let executor = Arc::new(ScriptedExecutor::new([
            ("a", succeeded(None, None)),
            ("b", succeeded(None, None)),
        ]));

        let results = execute_plan_with_observer(
            mode,
            vec![step(1, "a", 0), step(2, "b", 1)],
            executor,
            Arc::new(PanicOnFinishObserver {
                finished_calls: finished_calls.clone(),
            }),
        );

        assert_eq!(
            results
                .iter()
                .map(|result| result.status)
                .collect::<Vec<_>>(),
            [StepTerminalStatus::Succeeded, StepTerminalStatus::Succeeded,]
        );
        assert_eq!(finished_calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn serial_finished_observer_panic_does_not_stop_execution() {
        assert_finished_observer_panic_does_not_change_results(ExecutionMode::Serial);
    }

    #[test]
    fn parallel_finished_observer_panic_does_not_change_results() {
        assert_finished_observer_panic_does_not_change_results(ExecutionMode::Parallel);
    }

    #[test]
    fn validation_error_skips_executor_and_other_steps_continue() {
        let executor = Arc::new(ScriptedExecutor::new([("ok", succeeded(None, None))]));
        let calls = executor.calls.clone();
        let mut invalid = step(1, "invalid", 0);
        invalid.validation_error = Some("target disappeared".into());

        let results = execute_plan(
            ExecutionMode::Serial,
            vec![invalid, step(2, "ok", 1)],
            executor,
        );

        assert_eq!(*calls.lock().unwrap(), ["ok"]);
        assert_eq!(results[0].status, StepTerminalStatus::Failed);
        assert_eq!(results[0].message.as_deref(), Some("target disappeared"));
        assert_eq!(results[1].status, StepTerminalStatus::Succeeded);
    }

    #[test]
    fn aggregate_status_covers_success_partial_failure_and_empty_plan() {
        assert_eq!(aggregate_status(&[]), RunTerminalStatus::Failed);
        assert_eq!(
            aggregate_status(&[
                result(1, 0, StepTerminalStatus::Succeeded),
                result(2, 1, StepTerminalStatus::AlreadySatisfied),
            ]),
            RunTerminalStatus::Succeeded
        );
        assert_eq!(
            aggregate_status(&[
                result(1, 0, StepTerminalStatus::Succeeded),
                result(2, 1, StepTerminalStatus::Failed),
            ]),
            RunTerminalStatus::PartiallySucceeded
        );
        assert_eq!(
            aggregate_status(&[
                result(1, 0, StepTerminalStatus::Failed),
                result(2, 1, StepTerminalStatus::Failed),
            ]),
            RunTerminalStatus::Failed
        );
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum ObserverEvent {
        Started(i64),
        Finished(i64, StepTerminalStatus),
    }

    struct RecordingObserver {
        events: Arc<Mutex<Vec<ObserverEvent>>>,
    }

    impl ExecutionObserver for RecordingObserver {
        fn step_started(&self, run_step_id: i64) {
            self.events
                .lock()
                .unwrap()
                .push(ObserverEvent::Started(run_step_id));
        }

        fn step_finished(&self, result: &ExecutedStep) {
            self.events
                .lock()
                .unwrap()
                .push(ObserverEvent::Finished(result.run_step_id, result.status));
        }
    }

    #[test]
    fn serial_observer_records_started_then_finished_for_every_step() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let observer = Arc::new(RecordingObserver {
            events: events.clone(),
        });
        let executor = Arc::new(ScriptedExecutor::new([
            ("a", succeeded(None, None)),
            ("b", Behavior::Error("failed".into())),
            ("c", already_satisfied(None)),
        ]));

        execute_plan_with_observer(
            ExecutionMode::Serial,
            vec![step(1, "a", 0), step(2, "b", 1), step(3, "c", 2)],
            executor,
            observer,
        );

        assert_eq!(
            *events.lock().unwrap(),
            vec![
                ObserverEvent::Started(1),
                ObserverEvent::Finished(1, StepTerminalStatus::Succeeded),
                ObserverEvent::Started(2),
                ObserverEvent::Finished(2, StepTerminalStatus::Failed),
                ObserverEvent::Started(3),
                ObserverEvent::Finished(3, StepTerminalStatus::AlreadySatisfied),
            ]
        );
    }
}
