use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, Local};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::thread;
use std::time::Duration;

const SCHEMA_VERSION: u32 = 1;
const BUFFER_LIMIT: usize = 64 * 1024;
const FLUSH_INTERVAL: Duration = Duration::from_millis(500);
const START_LIMIT: u64 = 1024 * 1024;
const TAIL_LIMIT: u64 = 19 * 1024 * 1024;
const SEGMENT_LIMIT: u64 = 1024 * 1024;
const PROJECT_RUN_LIMIT: usize = 50;
const MAX_AGE_DAYS: i64 = 90;
const GLOBAL_LIMIT: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LogLane {
    Frontend,
    Backend,
    Upload,
}

impl LogLane {
    pub fn from_phase(phase: &str) -> Option<Self> {
        match phase {
            "frontend" => Some(Self::Frontend),
            "backend" => Some(Self::Backend),
            "upload" => Some(Self::Upload),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Frontend => "frontend",
            Self::Backend => "backend",
            Self::Upload => "upload",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RunLogDescriptor {
    pub run_id: String,
    pub project_id: i64,
    pub environment_id: i64,
    pub project_name: String,
    pub environment: String,
    pub operation: String,
    pub source_run_id: Option<String>,
    pub retry_kind: Option<String>,
    pub started_at: DateTime<FixedOffset>,
}

impl RunLogDescriptor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: impl Into<String>,
        project_id: i64,
        environment_id: i64,
        project_name: impl Into<String>,
        environment: impl Into<String>,
        operation: impl Into<String>,
        source_run_id: Option<String>,
        retry_kind: Option<String>,
        started_at: DateTime<FixedOffset>,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            project_id,
            environment_id,
            project_name: project_name.into(),
            environment: environment.into(),
            operation: operation.into(),
            source_run_id,
            retry_kind,
            started_at,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistenceWarning {
    pub action: String,
    pub path: String,
    pub cause: String,
}

impl std::fmt::Display for PersistenceWarning {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} {}: {}", self.action, self.path, self.cause)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SegmentManifest {
    file: String,
    order: u32,
    size_bytes: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LaneManifest {
    segments: Vec<SegmentManifest>,
    retained_bytes: u64,
    discarded_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    omission_marker: Option<String>,
    failed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunManifest {
    schema_version: u32,
    run_id: String,
    project_id: i64,
    environment_id: i64,
    project_name: String,
    environment: String,
    operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_kind: Option<String>,
    started_at: DateTime<FixedOffset>,
    updated_at: DateTime<FixedOffset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ended_at: Option<DateTime<FixedOffset>>,
    lifecycle: String,
    persistence: String,
    newline: String,
    command_output_may_contain_sensitive_data: bool,
    lanes: BTreeMap<String, LaneManifest>,
    warnings: Vec<PersistenceWarning>,
}

struct LaneState {
    pending: Vec<Vec<u8>>,
    pending_bytes: usize,
    start_size: u64,
    next_segment: u32,
    failed: bool,
}

impl Default for LaneState {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            pending_bytes: 0,
            start_size: 0,
            next_segment: 1,
            failed: false,
        }
    }
}

struct WriterState {
    manifest: RunManifest,
    lanes: BTreeMap<LogLane, LaneState>,
    stopped: bool,
}

struct WriterInner {
    root: PathBuf,
    run_dir: PathBuf,
    state: Mutex<WriterState>,
    timer_failures: Mutex<Vec<PersistenceWarning>>,
}

#[derive(Clone)]
pub struct RunLogWriter {
    inner: Arc<WriterInner>,
}

#[derive(Default)]
struct LogRegistry {
    active_run_dirs: HashSet<PathBuf>,
    deleting_projects: HashSet<PathBuf>,
}

fn log_registry() -> &'static Mutex<LogRegistry> {
    static REGISTRY: OnceLock<Mutex<LogRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(LogRegistry::default()))
}

fn active_writers() -> &'static Mutex<Vec<Weak<WriterInner>>> {
    static ACTIVE: OnceLock<Mutex<Vec<Weak<WriterInner>>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(Vec::new()))
}

impl RunLogWriter {
    pub fn create(root: &Path, descriptor: RunLogDescriptor) -> Result<Self, PersistenceWarning> {
        let mut registry = log_registry().lock().unwrap();
        let project_dir = root
            .join("projects")
            .join(descriptor.project_id.to_string());
        let run_dir = project_dir.join("runs").join(&descriptor.run_id);
        if registry.deleting_projects.contains(&project_dir) {
            return Err(warning(
                "create run directory",
                &run_dir,
                "project log deletion is in progress",
            ));
        }
        fs::create_dir_all(&run_dir)
            .map_err(|error| warning("create run directory", &run_dir, error))?;
        let manifest = RunManifest {
            schema_version: SCHEMA_VERSION,
            run_id: descriptor.run_id,
            project_id: descriptor.project_id,
            environment_id: descriptor.environment_id,
            project_name: descriptor.project_name,
            environment: descriptor.environment,
            operation: descriptor.operation,
            source_run_id: descriptor.source_run_id,
            retry_kind: descriptor.retry_kind,
            started_at: descriptor.started_at,
            updated_at: descriptor.started_at,
            ended_at: None,
            lifecycle: "running".into(),
            persistence: "complete".into(),
            newline: "\\n".into(),
            command_output_may_contain_sensitive_data: true,
            lanes: BTreeMap::new(),
            warnings: Vec::new(),
        };
        write_manifest(&run_dir, &manifest)?;
        let writer = Self {
            inner: Arc::new(WriterInner {
                root: root.to_path_buf(),
                run_dir: run_dir.clone(),
                state: Mutex::new(WriterState {
                    manifest,
                    lanes: BTreeMap::new(),
                    stopped: false,
                }),
                timer_failures: Mutex::new(Vec::new()),
            }),
        };
        registry.active_run_dirs.insert(run_dir);
        drop(registry);
        active_writers()
            .lock()
            .unwrap()
            .push(Arc::downgrade(&writer.inner));
        Ok(writer)
    }

    pub fn start_flush_timer(&self) {
        let weak = Arc::downgrade(&self.inner);
        thread::spawn(move || flush_loop(weak));
    }

    pub fn append(
        &self,
        lane: LogLane,
        stream: &str,
        line: &str,
    ) -> Result<(), PersistenceWarning> {
        self.append_at(lane, stream, line, Local::now().fixed_offset())
    }

    pub fn append_at(
        &self,
        lane: LogLane,
        stream: &str,
        line: &str,
        emitted_at: DateTime<FixedOffset>,
    ) -> Result<(), PersistenceWarning> {
        let normalized = line.replace('\r', "\\r").replace('\n', "\\n");
        let record =
            format!("{} [{}] {}\n", emitted_at.to_rfc3339(), stream, normalized).into_bytes();
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|error| warning("lock run log", &self.inner.run_dir, error))?;
        if state.stopped {
            return Ok(());
        }
        let lane_state = state.lanes.entry(lane).or_default();
        if lane_state.failed {
            return Ok(());
        }
        lane_state.pending_bytes += record.len();
        lane_state.pending.push(record);
        state.manifest.updated_at = emitted_at;
        if state.lanes[&lane].pending_bytes >= BUFFER_LIMIT {
            if let Err(error) = flush_lane(&self.inner.run_dir, &mut state, lane) {
                state.lanes.entry(lane).or_default().failed = true;
                state
                    .manifest
                    .lanes
                    .entry(lane.name().into())
                    .or_default()
                    .failed = true;
                return Err(error);
            }
            write_manifest(&self.inner.run_dir, &state.manifest)?;
        }
        Ok(())
    }

    pub fn flush_all(&self) -> Result<(), PersistenceWarning> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|error| warning("lock run log", &self.inner.run_dir, error))?;
        let mut additional_failures = Vec::new();
        let result = flush_all_locked(&self.inner.run_dir, &mut state, &mut additional_failures);
        self.inner
            .timer_failures
            .lock()
            .unwrap()
            .extend(additional_failures);
        result
    }

    pub fn finalize(&self, lifecycle: &str) -> Result<(), PersistenceWarning> {
        self.finalize_at(lifecycle, Local::now().fixed_offset())
    }

    pub fn finalize_at(
        &self,
        lifecycle: &str,
        ended_at: DateTime<FixedOffset>,
    ) -> Result<(), PersistenceWarning> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|error| warning("lock run log", &self.inner.run_dir, error))?;
        if state.stopped {
            return Ok(());
        }
        let mut additional_failures = Vec::new();
        let flush_failure =
            flush_all_locked(&self.inner.run_dir, &mut state, &mut additional_failures).err();
        self.inner
            .timer_failures
            .lock()
            .unwrap()
            .extend(additional_failures);
        if let Some(failure) = &flush_failure {
            state.manifest.persistence = "partial".into();
            if !state
                .manifest
                .warnings
                .iter()
                .any(|existing| existing.action == failure.action && existing.path == failure.path)
            {
                state.manifest.warnings.push(failure.clone());
            }
        }
        state.manifest.lifecycle = lifecycle.to_string();
        state.manifest.updated_at = ended_at;
        state.manifest.ended_at = Some(ended_at);
        state.stopped = true;
        let manifest_result = write_manifest(&self.inner.run_dir, &state.manifest);
        log_registry()
            .lock()
            .unwrap()
            .active_run_dirs
            .remove(&self.inner.run_dir);
        match (flush_failure, manifest_result) {
            (Some(failure), _) => Err(failure),
            (None, result) => result,
        }
    }

    pub fn record_failure(&self, failure: PersistenceWarning) -> Option<PersistenceWarning> {
        if let Ok(mut state) = self.inner.state.lock() {
            state.manifest.persistence = "partial".into();
            if !state
                .manifest
                .warnings
                .iter()
                .any(|existing| existing.action == failure.action && existing.path == failure.path)
            {
                state.manifest.warnings.push(failure.clone());
            }
            return write_manifest(&self.inner.run_dir, &state.manifest).err();
        }
        Some(warning(
            "lock run log failure state",
            &self.inner.run_dir,
            "run log lock poisoned",
        ))
    }

    pub fn take_timer_failures(&self) -> Vec<PersistenceWarning> {
        std::mem::take(&mut *self.inner.timer_failures.lock().unwrap())
    }

    pub fn reconcile_retention(&self) -> Vec<PersistenceWarning> {
        reconcile(&self.inner.root)
    }
}

impl RunLogWriter {
    fn seal_for_shutdown(&self) -> Vec<PersistenceWarning> {
        let mut state = match self.inner.state.lock() {
            Ok(state) => state,
            Err(error) => {
                return vec![warning(
                    "lock run log for shutdown",
                    &self.inner.run_dir,
                    error,
                )]
            }
        };
        if state.stopped {
            return Vec::new();
        }
        let mut failures = Vec::new();
        if let Err(primary) = flush_all_locked(&self.inner.run_dir, &mut state, &mut failures) {
            failures.insert(0, primary);
        }
        state.stopped = true;
        log_registry()
            .lock()
            .unwrap()
            .active_run_dirs
            .remove(&self.inner.run_dir);
        failures
    }
}

pub fn seal_active_for_shutdown() -> Vec<PersistenceWarning> {
    let mut writers = active_writers().lock().unwrap();
    let mut warnings = Vec::new();
    writers.retain(|weak| {
        let Some(inner) = weak.upgrade() else {
            return false;
        };
        let writer = RunLogWriter { inner };
        warnings.extend(writer.seal_for_shutdown());
        true
    });
    warnings
}

fn flush_loop(inner: Weak<WriterInner>) {
    loop {
        thread::sleep(FLUSH_INTERVAL);
        let Some(inner) = inner.upgrade() else {
            return;
        };
        let Ok(mut state) = inner.state.lock() else {
            return;
        };
        if state.stopped {
            return;
        }
        let mut additional_failures = Vec::new();
        if let Err(failure) = flush_all_locked(&inner.run_dir, &mut state, &mut additional_failures)
        {
            state.manifest.persistence = "partial".into();
            if !state
                .manifest
                .warnings
                .iter()
                .any(|existing| existing.action == failure.action && existing.path == failure.path)
            {
                state.manifest.warnings.push(failure.clone());
                if let Err(manifest_failure) = write_manifest(&inner.run_dir, &state.manifest) {
                    inner.timer_failures.lock().unwrap().push(manifest_failure);
                }
            }
            inner.timer_failures.lock().unwrap().push(failure);
        }
        inner
            .timer_failures
            .lock()
            .unwrap()
            .extend(additional_failures);
    }
}

fn flush_all_locked(
    run_dir: &Path,
    state: &mut WriterState,
    additional_failures: &mut Vec<PersistenceWarning>,
) -> Result<(), PersistenceWarning> {
    let mut first_failure = None;
    for lane in [LogLane::Frontend, LogLane::Backend, LogLane::Upload] {
        if state
            .lanes
            .get(&lane)
            .is_some_and(|value| !value.pending.is_empty())
        {
            if let Err(error) = flush_lane(run_dir, state, lane) {
                state.lanes.entry(lane).or_default().failed = true;
                state
                    .manifest
                    .lanes
                    .entry(lane.name().into())
                    .or_default()
                    .failed = true;
                if first_failure.is_none() {
                    first_failure = Some(error);
                } else {
                    additional_failures.push(error);
                }
            }
        }
    }
    if let Err(error) = write_manifest(run_dir, &state.manifest) {
        if first_failure.is_none() {
            first_failure = Some(error);
        } else {
            additional_failures.push(error);
        }
    }
    match first_failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn flush_lane(
    run_dir: &Path,
    state: &mut WriterState,
    lane: LogLane,
) -> Result<(), PersistenceWarning> {
    let lane_state = state.lanes.entry(lane).or_default();
    let records = std::mem::take(&mut lane_state.pending);
    lane_state.pending_bytes = 0;
    for record in records {
        let mut remaining = record.as_slice();
        if lane_state.start_size < START_LIMIT {
            let available = (START_LIMIT - lane_state.start_size) as usize;
            let prefix_len = utf8_prefix_len(remaining, available);
            let file = format!("{}-start.log", lane.name());
            append_file(run_dir, &file, &remaining[..prefix_len])?;
            lane_state.start_size += prefix_len as u64;
            update_segment(&mut state.manifest, lane, &file, 0, prefix_len as u64);
            remaining = &remaining[prefix_len..];
        }
        while !remaining.is_empty() {
            let first_char_len = std::str::from_utf8(remaining)
                .expect("formatted log records are UTF-8")
                .chars()
                .next()
                .expect("remaining log content is not empty")
                .len_utf8() as u64;
            let lane_manifest = state.manifest.lanes.entry(lane.name().into()).or_default();
            let current = lane_manifest
                .segments
                .last()
                .filter(|segment| {
                    segment.order > 0
                        && SEGMENT_LIMIT.saturating_sub(segment.size_bytes) >= first_char_len
                })
                .map(|segment| (segment.file.clone(), segment.order, segment.size_bytes));
            let (file, order, size) = current.unwrap_or_else(|| {
                let order = lane_state.next_segment;
                lane_state.next_segment += 1;
                (format!("{}-tail-{order:06}.log", lane.name()), order, 0)
            });
            let chunk_len = utf8_prefix_len(remaining, (SEGMENT_LIMIT - size) as usize);
            append_file(run_dir, &file, &remaining[..chunk_len])?;
            update_segment(&mut state.manifest, lane, &file, order, chunk_len as u64);
            remaining = &remaining[chunk_len..];
            trim_tail(run_dir, state.manifest.lanes.get_mut(lane.name()).unwrap())?;
        }
    }
    Ok(())
}

fn utf8_prefix_len(bytes: &[u8], limit: usize) -> usize {
    let text = std::str::from_utf8(bytes).expect("formatted log records are UTF-8");
    let mut length = limit.min(bytes.len());
    while !text.is_char_boundary(length) {
        length -= 1;
    }
    length
}

fn append_file(run_dir: &Path, file: &str, content: &[u8]) -> Result<(), PersistenceWarning> {
    let path = run_dir.join(file);
    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| warning("append lane", &path, error))?;
    output
        .write_all(content)
        .map_err(|error| warning("append lane", &path, error))?;
    output
        .flush()
        .map_err(|error| warning("flush lane", &path, error))
}

fn update_segment(manifest: &mut RunManifest, lane: LogLane, file: &str, order: u32, added: u64) {
    let lane_manifest = manifest.lanes.entry(lane.name().into()).or_default();
    if let Some(segment) = lane_manifest
        .segments
        .iter_mut()
        .find(|item| item.file == file)
    {
        segment.size_bytes += added;
    } else {
        lane_manifest.segments.push(SegmentManifest {
            file: file.into(),
            order,
            size_bytes: added,
        });
        lane_manifest.segments.sort_by_key(|segment| segment.order);
    }
    lane_manifest.retained_bytes += added;
}

fn trim_tail(run_dir: &Path, lane: &mut LaneManifest) -> Result<(), PersistenceWarning> {
    let mut tail_size: u64 = lane
        .segments
        .iter()
        .filter(|segment| segment.order > 0)
        .map(|segment| segment.size_bytes)
        .sum();
    while tail_size > TAIL_LIMIT {
        let Some(index) = lane.segments.iter().position(|segment| segment.order > 0) else {
            break;
        };
        let overflow = tail_size - TAIL_LIMIT;
        let oldest = &mut lane.segments[index];
        let path = run_dir.join(&oldest.file);
        let discarded = if oldest.size_bytes <= overflow {
            let removed = lane.segments.remove(index);
            fs::remove_file(&path)
                .map_err(|error| warning("remove rotated segment", &path, error))?;
            removed.size_bytes
        } else {
            let content =
                fs::read(&path).map_err(|error| warning("read rotated segment", &path, error))?;
            let text = std::str::from_utf8(&content)
                .map_err(|error| warning("read rotated segment", &path, error))?;
            let mut cut = overflow as usize;
            while !text.is_char_boundary(cut) {
                cut += 1;
            }
            let temporary = path.with_extension("log.tmp");
            fs::write(&temporary, &content[cut..])
                .map_err(|error| warning("write rotated segment temporary", &temporary, error))?;
            fs::rename(&temporary, &path)
                .map_err(|error| warning("replace rotated segment", &path, error))?;
            oldest.size_bytes -= cut as u64;
            cut as u64
        };
        tail_size -= discarded;
        lane.retained_bytes -= discarded;
        lane.discarded_bytes += discarded;
        lane.omission_marker = Some(format!(
            "[LazyCat omitted {} bytes from the middle of this log]\\n",
            lane.discarded_bytes
        ));
    }
    Ok(())
}

fn write_manifest(run_dir: &Path, manifest: &RunManifest) -> Result<(), PersistenceWarning> {
    let path = run_dir.join("manifest.json");
    let temporary = run_dir.join("manifest.json.tmp");
    let content = serde_json::to_vec_pretty(manifest)
        .map_err(|error| warning("serialize manifest", &path, error))?;
    fs::write(&temporary, content)
        .map_err(|error| warning("write manifest temporary", &temporary, error))?;
    fs::rename(&temporary, &path).map_err(|error| warning("replace manifest", &path, error))
}

fn warning(action: &str, path: &Path, error: impl std::fmt::Display) -> PersistenceWarning {
    PersistenceWarning {
        action: action.into(),
        path: path.to_string_lossy().into_owned(),
        cause: error.to_string(),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLogScope {
    pub run_count: usize,
    pub size_bytes: u64,
}

pub struct ProjectLogTombstone {
    original: PathBuf,
    tombstone: Option<PathBuf>,
}

pub fn project_scope(root: &Path, project_id: i64) -> Result<ProjectLogScope, PersistenceWarning> {
    let project_dir = root.join("projects").join(project_id.to_string());
    if !project_dir.exists() {
        return Ok(ProjectLogScope {
            run_count: 0,
            size_bytes: 0,
        });
    }
    let runs_dir = project_dir.join("runs");
    let mut count = 0;
    if runs_dir.exists() {
        for entry in fs::read_dir(&runs_dir)
            .map_err(|error| warning("inspect project logs", &runs_dir, error))?
        {
            let entry = entry.map_err(|error| warning("inspect project logs", &runs_dir, error))?;
            if !entry.path().is_dir() || !entry.path().join("manifest.json").is_file() {
                return Err(warning(
                    "inspect project logs",
                    &entry.path(),
                    "unknown run content",
                ));
            }
            read_manifest(&entry.path())?;
            count += 1;
        }
    }
    Ok(ProjectLogScope {
        run_count: count,
        size_bytes: directory_size(&project_dir)?,
    })
}

pub fn begin_project_delete(
    root: &Path,
    project_id: i64,
) -> Result<ProjectLogTombstone, PersistenceWarning> {
    let mut registry = log_registry().lock().unwrap();
    let original = root.join("projects").join(project_id.to_string());
    if registry
        .active_run_dirs
        .iter()
        .any(|run| run.starts_with(original.join("runs")))
    {
        return Err(warning(
            "delete project logs",
            &original,
            "project has an active run",
        ));
    }
    project_scope(root, project_id)?;
    if !original.exists() {
        registry.deleting_projects.insert(original.clone());
        return Ok(ProjectLogTombstone {
            original,
            tombstone: None,
        });
    }
    let tombstone_dir = root.join(".tombstones");
    fs::create_dir_all(&tombstone_dir)
        .map_err(|error| warning("create deletion tombstone directory", &tombstone_dir, error))?;
    let tombstone = tombstone_dir.join(format!("project-{project_id}-{}", uuid::Uuid::new_v4()));
    fs::rename(&original, &tombstone)
        .map_err(|error| warning("rename project logs to tombstone", &original, error))?;
    registry.deleting_projects.insert(original.clone());
    Ok(ProjectLogTombstone {
        original,
        tombstone: Some(tombstone),
    })
}

impl ProjectLogTombstone {
    pub fn restore(self) -> Result<(), PersistenceWarning> {
        if let Some(tombstone) = &self.tombstone {
            fs::rename(tombstone, &self.original)
                .map_err(|error| warning("restore project logs", &self.original, error))?;
        }
        log_registry()
            .lock()
            .unwrap()
            .deleting_projects
            .remove(&self.original);
        Ok(())
    }

    pub fn purge(self) -> Result<(), PersistenceWarning> {
        if let Some(tombstone) = &self.tombstone {
            fs::remove_dir_all(tombstone)
                .map_err(|error| warning("remove project log tombstone", tombstone, error))?;
        }
        log_registry()
            .lock()
            .unwrap()
            .deleting_projects
            .remove(&self.original);
        Ok(())
    }
}

#[derive(Clone)]
struct RecordCandidate {
    run_dir: PathBuf,
    project_id: i64,
    started_at: DateTime<FixedOffset>,
    size: u64,
}

#[cfg(test)]
fn initialize(root: &Path) -> Vec<PersistenceWarning> {
    initialize_with_project_check(root, |_| Ok(false))
}

pub fn initialize_with_project_check(
    root: &Path,
    mut project_exists: impl FnMut(i64) -> Result<bool, String>,
) -> Vec<PersistenceWarning> {
    let warnings = purge_tombstones(root, &mut project_exists);
    reconcile_records(root, warnings, true)
}

fn reconcile_records(
    root: &Path,
    mut warnings: Vec<PersistenceWarning>,
    mark_running_incomplete: bool,
) -> Vec<PersistenceWarning> {
    let mut candidates = Vec::new();
    let projects = root.join("projects");
    let project_entries = match fs::read_dir(&projects) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return warnings,
        Err(error) => {
            warnings.push(warning(
                "inspect release package log projects",
                &projects,
                error,
            ));
            return warnings;
        }
    };
    for project in project_entries {
        let project = match project {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(warning(
                    "inspect release package log project",
                    &projects,
                    error,
                ));
                continue;
            }
        };
        if !project.path().is_dir() {
            warnings.push(warning(
                "inspect release package log project",
                &project.path(),
                "unknown project content",
            ));
            continue;
        }
        let runs = project.path().join("runs");
        let run_entries = match fs::read_dir(&runs) {
            Ok(entries) => entries,
            Err(error) => {
                warnings.push(warning("inspect release package runs", &runs, error));
                continue;
            }
        };
        for run in run_entries {
            let run = match run {
                Ok(entry) => entry,
                Err(error) => {
                    warnings.push(warning("inspect release package run", &runs, error));
                    continue;
                }
            };
            if !run.path().is_dir() {
                warnings.push(warning(
                    "inspect release package run",
                    &run.path(),
                    "unknown run content",
                ));
                continue;
            }
            match read_manifest(&run.path()) {
                Ok(mut manifest) => {
                    if mark_running_incomplete && manifest.lifecycle == "running" {
                        manifest.lifecycle = "incomplete".into();
                        manifest.updated_at = Local::now().fixed_offset();
                        if let Err(error) = write_manifest(&run.path(), &manifest) {
                            warnings.push(error);
                            continue;
                        }
                    }
                    match directory_size(&run.path()) {
                        Ok(size) => candidates.push(RecordCandidate {
                            run_dir: run.path(),
                            project_id: manifest.project_id,
                            started_at: manifest.started_at,
                            size,
                        }),
                        Err(error) => warnings.push(error),
                    }
                }
                Err(error) => warnings.push(error),
            }
        }
    }
    warnings.extend(reconcile_candidates(
        candidates,
        Local::now().fixed_offset(),
    ));
    warnings
}

pub fn reconcile(root: &Path) -> Vec<PersistenceWarning> {
    reconcile_records(root, Vec::new(), false)
}

fn reconcile_candidates(
    mut candidates: Vec<RecordCandidate>,
    now: DateTime<FixedOffset>,
) -> Vec<PersistenceWarning> {
    candidates.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then_with(|| left.run_dir.cmp(&right.run_dir))
    });
    let active = log_registry().lock().unwrap().active_run_dirs.clone();
    let mut remove = HashSet::new();
    for candidate in &candidates {
        if !active.contains(&candidate.run_dir)
            && candidate.started_at < now - ChronoDuration::days(MAX_AGE_DAYS)
        {
            remove.insert(candidate.run_dir.clone());
        }
    }
    let project_ids: HashSet<i64> = candidates.iter().map(|item| item.project_id).collect();
    for project_id in project_ids {
        let project: Vec<_> = candidates
            .iter()
            .filter(|item| item.project_id == project_id)
            .collect();
        for candidate in project
            .iter()
            .filter(|candidate| !active.contains(&candidate.run_dir))
            .take(project.len().saturating_sub(PROJECT_RUN_LIMIT))
        {
            remove.insert(candidate.run_dir.clone());
        }
    }
    let mut retained_size: u64 = candidates
        .iter()
        .filter(|item| !remove.contains(&item.run_dir))
        .map(|item| item.size)
        .sum();
    for candidate in &candidates {
        if retained_size <= GLOBAL_LIMIT {
            break;
        }
        if !active.contains(&candidate.run_dir) && remove.insert(candidate.run_dir.clone()) {
            retained_size = retained_size.saturating_sub(candidate.size);
        }
    }
    let mut warnings = Vec::new();
    for path in remove {
        if let Err(error) = fs::remove_dir_all(&path) {
            warnings.push(warning("remove retained run", &path, error));
        }
    }
    warnings
}

fn purge_tombstones(
    root: &Path,
    project_exists: &mut impl FnMut(i64) -> Result<bool, String>,
) -> Vec<PersistenceWarning> {
    let directory = root.join(".tombstones");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => return vec![warning("inspect project log tombstones", &directory, error)],
    };
    let mut warnings = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(warning("inspect project log tombstone", &directory, error));
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let parsed_name = name
            .strip_prefix("project-")
            .and_then(|value| value.split_once('-'))
            .and_then(|(project_id, suffix)| {
                let project_id = project_id.parse::<i64>().ok()?;
                uuid::Uuid::parse_str(suffix).ok()?;
                Some(project_id)
            });
        let Some(project_id) = parsed_name.filter(|_| entry.path().is_dir()) else {
            warnings.push(warning(
                "inspect project log tombstone",
                &entry.path(),
                "unknown tombstone content",
            ));
            continue;
        };
        match project_exists(project_id) {
            Ok(true) => {
                let original = root.join("projects").join(project_id.to_string());
                if let Err(error) = fs::rename(entry.path(), &original) {
                    warnings.push(warning("restore project log tombstone", &original, error));
                }
                continue;
            }
            Ok(false) => {}
            Err(error) => {
                warnings.push(warning("inspect tombstone project", &entry.path(), error));
                continue;
            }
        }
        if let Err(error) = fs::remove_dir_all(entry.path()) {
            warnings.push(warning(
                "remove project log tombstone",
                &entry.path(),
                error,
            ));
        }
    }
    warnings
}

fn read_manifest(run_dir: &Path) -> Result<RunManifest, PersistenceWarning> {
    let path = run_dir.join("manifest.json");
    let bytes = fs::read(&path).map_err(|error| warning("read manifest", &path, error))?;
    let manifest: RunManifest =
        serde_json::from_slice(&bytes).map_err(|error| warning("parse manifest", &path, error))?;
    if manifest.schema_version != SCHEMA_VERSION {
        return Err(warning(
            "parse manifest",
            &path,
            "unsupported schema version",
        ));
    }
    for lane in manifest.lanes.values() {
        for segment in &lane.segments {
            let segment_path = run_dir.join(&segment.file);
            if !segment_path.is_file() {
                return Err(warning(
                    "inspect run record",
                    &segment_path,
                    "missing segment",
                ));
            }
        }
    }
    Ok(manifest)
}

fn directory_size(path: &Path) -> Result<u64, PersistenceWarning> {
    let mut size = 0;
    let entries =
        fs::read_dir(path).map_err(|error| warning("inspect directory size", path, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| warning("inspect directory size", path, error))?;
        let metadata = entry
            .metadata()
            .map_err(|error| warning("inspect directory size", &entry.path(), error))?;
        if metadata.is_dir() {
            size += directory_size(&entry.path())?;
        } else {
            size += metadata.len();
        }
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn timestamp(value: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(value).unwrap()
    }

    fn descriptor(run_id: &str, operation: &str) -> RunLogDescriptor {
        RunLogDescriptor::new(
            run_id,
            7,
            41,
            "Portal",
            "test",
            operation,
            None,
            None,
            timestamp("2026-08-26T10:00:00+08:00"),
        )
    }

    #[test]
    fn real_run_events_create_readable_lane_files_and_terminal_manifest() {
        let root = tempdir().unwrap();
        let writer =
            RunLogWriter::create(root.path(), descriptor("run-1", "local_archive")).unwrap();
        writer
            .append_at(
                LogLane::Frontend,
                "stdout",
                "frontend ready",
                timestamp("2026-08-26T10:00:01+08:00"),
            )
            .unwrap();
        writer
            .append_at(
                LogLane::Backend,
                "stderr",
                "backend warning",
                timestamp("2026-08-26T10:00:02+08:00"),
            )
            .unwrap();
        writer
            .finalize_at("succeeded", timestamp("2026-08-26T10:00:03+08:00"))
            .unwrap();
        let run_dir = root.path().join("projects/7/runs/run-1");
        assert_eq!(
            fs::read_to_string(run_dir.join("frontend-start.log")).unwrap(),
            "2026-08-26T10:00:01+08:00 [stdout] frontend ready\n"
        );
        assert_eq!(
            fs::read_to_string(run_dir.join("backend-start.log")).unwrap(),
            "2026-08-26T10:00:02+08:00 [stderr] backend warning\n"
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(run_dir.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["lifecycle"], "succeeded");
        assert_eq!(manifest["projectName"], "Portal");
        assert!(manifest["lanes"]["upload"].is_null());
    }

    #[test]
    fn retry_manifest_keeps_source_identity_and_normalizes_physical_lines() {
        let root = tempdir().unwrap();
        let mut value = descriptor("retry-1", "upload_retry");
        value.source_run_id = Some("run-1".into());
        value.retry_kind = Some("upload".into());
        let writer = RunLogWriter::create(root.path(), value).unwrap();
        writer
            .append_at(
                LogLane::Upload,
                "stdout",
                "first\r\nsecond",
                timestamp("2026-08-26T10:00:01+08:00"),
            )
            .unwrap();
        writer.finalize("failed").unwrap();
        let run_dir = root.path().join("projects/7/runs/retry-1");
        let log = fs::read_to_string(run_dir.join("upload-start.log")).unwrap();
        assert!(log.contains("first\\r\\nsecond\n"));
        let manifest: RunManifest = read_manifest(&run_dir).unwrap();
        assert_eq!(manifest.source_run_id.as_deref(), Some("run-1"));
        assert_eq!(manifest.retry_kind.as_deref(), Some("upload"));
    }

    #[test]
    fn startup_marks_running_records_incomplete_and_keeps_saved_output() {
        let root = tempdir().unwrap();
        let writer =
            RunLogWriter::create(root.path(), descriptor("interrupted", "server_upload")).unwrap();
        writer
            .append_at(
                LogLane::Upload,
                "stderr",
                "connection lost",
                timestamp("2026-08-26T10:00:01+08:00"),
            )
            .unwrap();
        writer.flush_all().unwrap();
        log_registry()
            .lock()
            .unwrap()
            .active_run_dirs
            .remove(&root.path().join("projects/7/runs/interrupted"));
        drop(writer);
        let warnings = initialize(root.path());
        assert!(warnings.is_empty());
        let run_dir = root.path().join("projects/7/runs/interrupted");
        assert_eq!(read_manifest(&run_dir).unwrap().lifecycle, "incomplete");
        assert!(fs::read_to_string(run_dir.join("upload-start.log"))
            .unwrap()
            .contains("connection lost"));
    }

    #[test]
    fn project_delete_tombstone_can_restore_or_purge_the_whole_scope() {
        let root = tempdir().unwrap();
        let writer =
            RunLogWriter::create(root.path(), descriptor("run-delete", "local_archive")).unwrap();
        writer.finalize("succeeded").unwrap();
        let scope = project_scope(root.path(), 7).unwrap();
        assert_eq!(scope.run_count, 1);
        let tombstone = begin_project_delete(root.path(), 7).unwrap();
        assert!(!root.path().join("projects/7").exists());
        tombstone.restore().unwrap();
        let tombstone = begin_project_delete(root.path(), 7).unwrap();
        tombstone.purge().unwrap();
        assert!(!root.path().join("projects/7").exists());
    }

    #[test]
    fn lane_capacity_keeps_start_and_latest_segments_with_discard_accounting() {
        let root = tempdir().unwrap();
        let writer =
            RunLogWriter::create(root.path(), descriptor("large-run", "local_archive")).unwrap();
        for index in 0..180 {
            let line = format!("marker-{index:03} {}", "x".repeat(128 * 1024));
            writer
                .append_at(
                    LogLane::Frontend,
                    "stdout",
                    &line,
                    timestamp("2026-08-26T10:00:01+08:00"),
                )
                .unwrap();
        }
        writer.finalize("succeeded").unwrap();
        let run_dir = root.path().join("projects/7/runs/large-run");
        let manifest = read_manifest(&run_dir).unwrap();
        let lane = &manifest.lanes["frontend"];
        assert!(lane.retained_bytes <= START_LIMIT + TAIL_LIMIT);
        assert!(lane.discarded_bytes > 0);
        assert!(lane.omission_marker.as_deref().unwrap().contains("omitted"));
        assert!(fs::read_to_string(run_dir.join("frontend-start.log"))
            .unwrap()
            .contains("marker-000"));
        let latest = lane.segments.last().unwrap();
        assert!(fs::read_to_string(run_dir.join(&latest.file))
            .unwrap()
            .contains("marker-179"));
    }

    #[test]
    fn retention_keeps_fifty_newest_records_and_preserves_unknown_content() {
        let root = tempdir().unwrap();
        for index in 0..52 {
            let mut value = descriptor(&format!("run-{index:02}"), "local_archive");
            value.started_at = timestamp("2026-07-01T10:00:00+08:00") + ChronoDuration::days(index);
            RunLogWriter::create(root.path(), value)
                .unwrap()
                .finalize("succeeded")
                .unwrap();
        }
        let unknown = root.path().join("projects/7/runs/unknown");
        fs::create_dir_all(&unknown).unwrap();
        fs::write(unknown.join("keep.txt"), "do not delete").unwrap();
        let warnings = initialize(root.path());
        assert!(warnings
            .iter()
            .any(|warning| warning.path.contains("unknown")));
        assert!(unknown.join("keep.txt").is_file());
        let known_count = fs::read_dir(root.path().join("projects/7/runs"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().join("manifest.json").is_file())
            .count();
        assert_eq!(known_count, 50);
        assert!(!root.path().join("projects/7/runs/run-00").exists());
        assert!(root.path().join("projects/7/runs/run-51").exists());
    }

    #[test]
    fn one_lane_failure_does_not_block_other_lane_flushes() {
        let root = tempdir().unwrap();
        let writer =
            RunLogWriter::create(root.path(), descriptor("partial-run", "local_archive")).unwrap();
        let run_dir = root.path().join("projects/7/runs/partial-run");
        fs::create_dir(run_dir.join("frontend-start.log")).unwrap();
        writer
            .append_at(
                LogLane::Frontend,
                "stdout",
                "frontend",
                timestamp("2026-08-26T10:00:01+08:00"),
            )
            .unwrap();
        writer
            .append_at(
                LogLane::Backend,
                "stdout",
                "backend",
                timestamp("2026-08-26T10:00:01+08:00"),
            )
            .unwrap();
        let failure = writer.flush_all().unwrap_err();
        assert!(writer.record_failure(failure.clone()).is_none());
        assert_eq!(failure.action, "append lane");
        assert!(failure.path.contains("frontend-start.log"));
        assert!(fs::read_to_string(run_dir.join("backend-start.log"))
            .unwrap()
            .contains("backend"));
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(run_dir.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["persistence"], "partial");
        assert_eq!(manifest["lanes"]["frontend"]["failed"], true);
    }

    #[test]
    fn terminal_flush_failure_keeps_delivery_lifecycle_and_marks_partial_persistence() {
        let root = tempdir().unwrap();
        let writer =
            RunLogWriter::create(root.path(), descriptor("terminal-partial", "local_archive"))
                .unwrap();
        let run_dir = root.path().join("projects/7/runs/terminal-partial");
        fs::create_dir(run_dir.join("frontend-start.log")).unwrap();
        writer
            .append_at(
                LogLane::Frontend,
                "stdout",
                "delivery succeeded",
                timestamp("2026-08-26T10:00:01+08:00"),
            )
            .unwrap();

        assert!(writer.finalize("succeeded").is_err());
        drop(writer);
        let manifest: RunManifest = read_manifest(&run_dir).unwrap();
        assert_eq!(manifest.lifecycle, "succeeded");
        assert_eq!(manifest.persistence, "partial");
    }

    #[test]
    fn shutdown_seal_flushes_pending_output_and_rejects_late_worker_appends() {
        let root = tempdir().unwrap();
        let writer =
            RunLogWriter::create(root.path(), descriptor("shutdown-run", "server_upload")).unwrap();
        writer
            .append_at(
                LogLane::Upload,
                "stdout",
                "before shutdown",
                timestamp("2026-08-26T10:00:01+08:00"),
            )
            .unwrap();
        assert!(writer.seal_for_shutdown().is_empty());
        writer
            .append_at(
                LogLane::Upload,
                "stdout",
                "late worker output",
                timestamp("2026-08-26T10:00:02+08:00"),
            )
            .unwrap();

        let run_dir = root.path().join("projects/7/runs/shutdown-run");
        let log = fs::read_to_string(run_dir.join("upload-start.log")).unwrap();
        assert!(log.contains("before shutdown"));
        assert!(!log.contains("late worker output"));
        assert_eq!(read_manifest(&run_dir).unwrap().lifecycle, "running");
    }

    #[test]
    fn project_delete_registration_blocks_a_new_writer_until_delete_finishes() {
        let root = tempdir().unwrap();
        let deletion = begin_project_delete(root.path(), 7).unwrap();
        let failure = RunLogWriter::create(root.path(), descriptor("racing-run", "local_archive"))
            .err()
            .unwrap();
        assert!(failure.cause.contains("deletion is in progress"));
        deletion.restore().unwrap();
        RunLogWriter::create(root.path(), descriptor("accepted-run", "local_archive"))
            .unwrap()
            .finalize("succeeded")
            .unwrap();
    }

    #[test]
    fn startup_restores_tombstone_when_its_project_still_exists() {
        let root = tempdir().unwrap();
        RunLogWriter::create(root.path(), descriptor("restore-run", "local_archive"))
            .unwrap()
            .finalize("succeeded")
            .unwrap();
        let original = root.path().join("projects/7");
        let tombstones = root.path().join(".tombstones");
        fs::create_dir_all(&tombstones).unwrap();
        let tombstone = tombstones.join(format!("project-7-{}", uuid::Uuid::new_v4()));
        fs::rename(&original, &tombstone).unwrap();

        let warnings = initialize_with_project_check(root.path(), |project_id| Ok(project_id == 7));
        assert!(warnings.is_empty());
        assert!(original.join("runs/restore-run/manifest.json").is_file());
        assert!(!tombstone.exists());
    }

    #[test]
    fn oversized_entry_still_retains_the_latest_utf8_content() {
        let root = tempdir().unwrap();
        let writer =
            RunLogWriter::create(root.path(), descriptor("oversized", "local_archive")).unwrap();
        let line = format!("{} latest-marker", "界".repeat(8 * 1024 * 1024));
        writer
            .append_at(
                LogLane::Frontend,
                "stdout",
                &line,
                timestamp("2026-08-26T10:00:01+08:00"),
            )
            .unwrap();
        writer.finalize("succeeded").unwrap();
        let run_dir = root.path().join("projects/7/runs/oversized");
        let manifest = read_manifest(&run_dir).unwrap();
        let lane = &manifest.lanes["frontend"];
        let latest = lane.segments.last().unwrap();
        assert!(fs::read_to_string(run_dir.join(&latest.file))
            .unwrap()
            .contains("latest-marker"));
        assert!(lane.retained_bytes <= START_LIMIT + TAIL_LIMIT);
        assert!(lane.discarded_bytes > 0);
    }

    #[test]
    fn global_budget_removes_the_oldest_record_first() {
        let root = tempdir().unwrap();
        for (run_id, started_at) in [
            ("older", "2026-08-01T10:00:00+08:00"),
            ("newer", "2026-08-02T10:00:00+08:00"),
        ] {
            let mut value = descriptor(run_id, "local_archive");
            value.started_at = timestamp(started_at);
            let writer = RunLogWriter::create(root.path(), value).unwrap();
            writer.finalize("succeeded").unwrap();
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(
                    root.path()
                        .join(format!("projects/7/runs/{run_id}/capacity.bin")),
                )
                .unwrap()
                .set_len(600 * 1024 * 1024)
                .unwrap();
        }
        assert!(initialize(root.path()).is_empty());
        assert!(!root.path().join("projects/7/runs/older").exists());
        assert!(root.path().join("projects/7/runs/newer").exists());
    }
}
