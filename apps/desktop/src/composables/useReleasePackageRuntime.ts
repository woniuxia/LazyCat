import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, reactive, toRefs } from "vue";
import { APP_EVENTS } from "../bridge/events";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ReleasePackageLogEvent,
  ReleasePackagePhase,
  ReleasePackageRunStatus,
  ReleasePackageStatusEvent,
  ReleasePackageTarget,
  ReleasePackageTargetStatus,
  ReleasePackageCommandStatus,
  ReleasePackageUploadProgress,
} from "../types/release-package";
import { acceptReleasePackageEvent, appendReleasePackageLog } from "../utils/releasePackage";

export interface ReleasePackageRuntimeState {
  activeRunId: string | null;
  activeProjectId: number | null;
  pendingProjectId: number | null;
  status: ReleasePackageRunStatus;
  phase: ReleasePackagePhase | null;
  archivePath: string;
  error: string;
}

export interface ReleasePackageProjectRuntime {
  runId: string | null;
  status: ReleasePackageRunStatus;
  archivePath: string;
  error: string;
  targetStatus: Record<ReleasePackageTarget, ReleasePackageTargetStatus>;
  targetErrors: Partial<Record<ReleasePackageTarget, string>>;
  commandStatus: Record<ReleasePackageTarget, ReleasePackageCommandStatus>;
  commandErrors: Partial<Record<ReleasePackageTarget, string>>;
  commandRetryToken: string;
  frontendLogs: ReleasePackageLogEvent[];
  backendLogs: ReleasePackageLogEvent[];
  uploadLogs: ReleasePackageLogEvent[];
  uploadProgress: ReleasePackageUploadProgress;
  retryToken: string;
}

export function createReleasePackageRuntimeState(): ReleasePackageRuntimeState {
  return {
    activeRunId: null,
    activeProjectId: null,
    pendingProjectId: null,
    status: "idle",
    phase: null,
    archivePath: "",
    error: "",
  };
}

export function reduceReleasePackageStatus(
  state: ReleasePackageRuntimeState,
  event: ReleasePackageStatusEvent,
): void {
  if (!state.activeRunId && state.pendingProjectId === event.projectId) {
    state.activeRunId = event.runId;
  }
  if (!acceptReleasePackageEvent(state.activeRunId, event)) return;
  state.activeProjectId = event.projectId;
  state.phase = event.phase;
  if (event.phase === "upload") {
    state.status = event.status;
    return;
  }
  if (event.phase !== "overall") return;
  state.status = event.status;
  state.archivePath = event.archivePath ?? "";
  state.error = event.error ?? "";
  if (event.status !== "running") state.pendingProjectId = null;
}

const state = reactive(createReleasePackageRuntimeState());
const projectRuntimes = reactive(new Map<number, ReleasePackageProjectRuntime>());
const logs = computed(() => {
  if (state.activeProjectId === null) return [];
  const runtime = projectRuntimes.get(state.activeProjectId);
  return runtime
    ? [...runtime.frontendLogs, ...runtime.backendLogs, ...runtime.uploadLogs]
    : [];
});
let listenerPromise: Promise<UnlistenFn[]> | null = null;

function createProjectRuntime(
  targets: readonly ReleasePackageTarget[] = [],
): ReleasePackageProjectRuntime {
  return {
    runId: null,
    status: "idle",
    archivePath: "",
    error: "",
    targetStatus: {
      frontend: targets.includes("frontend") ? "pending" : "skipped",
      backend: targets.includes("backend") ? "pending" : "skipped",
    },
    targetErrors: {},
    frontendLogs: [],
    backendLogs: [],
    uploadLogs: [],
    uploadProgress: {
      uploadedBytes: 0,
      totalBytes: 0,
      currentPath: "",
    },
    commandStatus: {
      frontend: targets.includes("frontend") ? "pending" : "skipped",
      backend: targets.includes("backend") ? "pending" : "skipped",
    },
    commandErrors: {},
    retryToken: "",
    commandRetryToken: "",
  };
}

function getProjectRuntime(projectId: number): ReleasePackageProjectRuntime {
  let runtime = projectRuntimes.get(projectId);
  if (!runtime) {
    projectRuntimes.set(projectId, createProjectRuntime());
    runtime = projectRuntimes.get(projectId)!;
  }
  return runtime;
}

function applyProjectStatus(event: ReleasePackageStatusEvent): void {
  const runtime = getProjectRuntime(event.projectId);
  runtime.runId = event.runId;
  if (event.phase === "frontend" || event.phase === "backend") {
    if (
      event.status !== "partially_succeeded"
      && event.status !== "prechecking"
      && event.status !== "uploading"
      && event.status !== "package_succeeded_upload_failed"
    ) {
      runtime.targetStatus[event.phase] = event.status;
    }
    if (event.error) runtime.targetErrors[event.phase] = event.error;
    return;
  }
  if (event.phase === "upload") {
    if (event.commandTarget && event.commandStatus) {
      runtime.commandStatus[event.commandTarget] = event.commandStatus;
      if (event.error) runtime.commandErrors[event.commandTarget] = event.error;
      return;
    }
    runtime.status = event.status;
    runtime.uploadProgress = {
      uploadedBytes: event.uploadedBytes ?? runtime.uploadProgress.uploadedBytes,
      totalBytes: event.totalBytes ?? runtime.uploadProgress.totalBytes,
      currentPath: event.currentPath ?? runtime.uploadProgress.currentPath,
    };
    return;
  }
  runtime.status = event.status;
  runtime.archivePath = event.archivePath ?? "";
  runtime.error = event.error ?? "";
  runtime.retryToken = event.retryToken ?? "";
  runtime.commandRetryToken = event.commandRetryToken ?? "";
}

function acceptPendingEvent(runId: string, projectId: number): boolean {
  if (!state.activeRunId && state.pendingProjectId === projectId) {
    state.activeRunId = runId;
    getProjectRuntime(projectId).runId = runId;
  }
  return state.activeProjectId !== null
    && state.activeProjectId === projectId
    && acceptReleasePackageEvent(state.activeRunId, { runId });
}

function ensureListeners(): Promise<void> {
  if (!listenerPromise) {
    listenerPromise = Promise.all([
      listen<ReleasePackageLogEvent>(APP_EVENTS.RELEASE_PACKAGE_LOG, ({ payload }) => {
        if (!acceptPendingEvent(payload.runId, payload.projectId)) return;
        state.activeProjectId = payload.projectId;
        if (payload.phase === "overall") return;
        const runtime = getProjectRuntime(payload.projectId);
        const key = payload.phase === "frontend"
          ? "frontendLogs"
          : payload.phase === "backend"
            ? "backendLogs"
            : "uploadLogs";
        runtime[key] = appendReleasePackageLog(runtime[key], payload, 1_000);
      }),
      listen<ReleasePackageStatusEvent>(APP_EVENTS.RELEASE_PACKAGE_STATUS, ({ payload }) => {
        if (!acceptPendingEvent(payload.runId, payload.projectId)) return;
        applyProjectStatus(payload);
        reduceReleasePackageStatus(state, payload);
      }),
    ]);
  }
  return listenerPromise.then(() => undefined);
}

function beginStart(projectId: number, targets: readonly ReleasePackageTarget[] = ["frontend", "backend"]): void {
  Object.assign(state, createReleasePackageRuntimeState(), {
    activeProjectId: projectId,
    pendingProjectId: projectId,
    status: "running",
  });
  projectRuntimes.set(projectId, createProjectRuntime(targets));
  getProjectRuntime(projectId).status = "running";
}

function bindStartedRun(runId: string, projectId: number): void {
  state.activeRunId = runId;
  state.activeProjectId = projectId;
  state.pendingProjectId = projectId;
  getProjectRuntime(projectId).runId = runId;
}

function abortStart(message: string): void {
  const projectId = state.pendingProjectId ?? state.activeProjectId;
  if (projectId !== null) {
    const runtime = getProjectRuntime(projectId);
    runtime.status = "failed";
    runtime.error = message;
  }
  state.activeRunId = null;
  state.activeProjectId = null;
  state.pendingProjectId = null;
  state.status = "failed";
  state.phase = null;
  state.error = message;
}

async function cancel(): Promise<void> {
  if (!state.activeRunId) return;
  await invokeToolByChannel("tool:release-package:cancel", { runId: state.activeRunId });
}

function reset(): void {
  Object.assign(state, createReleasePackageRuntimeState());
  projectRuntimes.clear();
}

export function useReleasePackageRuntime() {
  return {
    ...toRefs(state),
    logs,
    isRunning: computed(() => state.status === "running" || state.status === "uploading"),
    ensureListeners,
    beginStart,
    bindStartedRun,
    abortStart,
    cancel,
    getProjectRuntime,
    reset,
  };
}
