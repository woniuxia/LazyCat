import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, reactive, toRefs } from "vue";
import { APP_EVENTS } from "../bridge/events";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ReleasePackageEnvironmentKind,
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
  activeEnvironmentId: number | null;
  pendingEnvironmentId: number | null;
  status: ReleasePackageRunStatus;
  phase: ReleasePackagePhase | null;
  archivePath: string;
  error: string;
}

export interface ReleasePackageEnvironmentRuntime {
  runId: string | null;
  readonly projectId: number | null;
  readonly environment: ReleasePackageEnvironmentKind | null;
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

type MutableReleasePackageEnvironmentRuntime = {
  -readonly [Key in keyof ReleasePackageEnvironmentRuntime]: ReleasePackageEnvironmentRuntime[Key];
};

export function createReleasePackageRuntimeState(): ReleasePackageRuntimeState {
  return {
    activeRunId: null,
    activeEnvironmentId: null,
    pendingEnvironmentId: null,
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
  if (!state.activeRunId && state.pendingEnvironmentId === event.environmentId) {
    state.activeRunId = event.runId;
    state.activeEnvironmentId = event.environmentId;
  }
  if (state.activeEnvironmentId !== event.environmentId) return;
  if (!acceptReleasePackageEvent(state.activeRunId, event)) return;
  state.activeEnvironmentId = event.environmentId;
  state.phase = event.phase;
  if (event.phase === "upload") {
    state.status = event.status;
    return;
  }
  if (event.phase !== "overall") return;
  state.status = event.status;
  state.archivePath = event.archivePath ?? "";
  state.error = event.error ?? "";
  if (event.status !== "running") state.pendingEnvironmentId = null;
}

const state = reactive(createReleasePackageRuntimeState());
const environmentRuntimes = reactive(new Map<number, MutableReleasePackageEnvironmentRuntime>());
const logs = computed(() => {
  if (state.activeEnvironmentId === null) return [];
  const runtime = environmentRuntimes.get(state.activeEnvironmentId);
  return runtime
    ? [...runtime.frontendLogs, ...runtime.backendLogs, ...runtime.uploadLogs]
    : [];
});
let listenerPromise: Promise<UnlistenFn[]> | null = null;

function createEnvironmentRuntime(
  targets: readonly ReleasePackageTarget[] = [],
): MutableReleasePackageEnvironmentRuntime {
  return {
    runId: null,
    projectId: null,
    environment: null,
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

function getMutableEnvironmentRuntime(environmentId: number): MutableReleasePackageEnvironmentRuntime {
  let runtime = environmentRuntimes.get(environmentId);
  if (!runtime) {
    environmentRuntimes.set(environmentId, createEnvironmentRuntime());
    runtime = environmentRuntimes.get(environmentId)!;
  }
  return runtime;
}

function getEnvironmentRuntime(environmentId: number): ReleasePackageEnvironmentRuntime {
  return getMutableEnvironmentRuntime(environmentId);
}

function applyEnvironmentStatus(event: ReleasePackageStatusEvent): void {
  const runtime = getMutableEnvironmentRuntime(event.environmentId);
  runtime.runId = event.runId;
  runtime.projectId = event.projectId;
  runtime.environment = event.environment;
  if (event.phase === "frontend" || event.phase === "backend") {
    if (
      event.status !== "partially_succeeded"
      && event.status !== "prechecking"
      && event.status !== "uploading"
      && event.status !== "package_succeeded_upload_failed"
      && event.status !== "upload_succeeded_command_failed"
      && event.status !== "deployed_health_check_failed"
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

function acceptPendingEvent(runId: string, environmentId: number): boolean {
  if (!state.activeRunId && state.pendingEnvironmentId === environmentId) {
    state.activeRunId = runId;
    getMutableEnvironmentRuntime(environmentId).runId = runId;
  }
  return state.activeEnvironmentId !== null
    && state.activeEnvironmentId === environmentId
    && acceptReleasePackageEvent(state.activeRunId, { runId });
}

function ensureListeners(): Promise<void> {
  if (!listenerPromise) {
    listenerPromise = Promise.all([
      listen<ReleasePackageLogEvent>(APP_EVENTS.RELEASE_PACKAGE_LOG, ({ payload }) => {
        if (!acceptPendingEvent(payload.runId, payload.environmentId)) return;
        state.activeEnvironmentId = payload.environmentId;
        if (payload.phase === "overall") return;
        const runtime = getMutableEnvironmentRuntime(payload.environmentId);
        runtime.projectId = payload.projectId;
        runtime.environment = payload.environment;
        const key = payload.phase === "frontend"
          ? "frontendLogs"
          : payload.phase === "backend"
            ? "backendLogs"
            : "uploadLogs";
        runtime[key] = appendReleasePackageLog(runtime[key], payload, 1_000);
      }),
      listen<ReleasePackageStatusEvent>(APP_EVENTS.RELEASE_PACKAGE_STATUS, ({ payload }) => {
        if (!acceptPendingEvent(payload.runId, payload.environmentId)) return;
        applyEnvironmentStatus(payload);
        reduceReleasePackageStatus(state, payload);
      }),
    ]);
  }
  return listenerPromise.then(() => undefined);
}

function beginStart(environmentId: number, targets: readonly ReleasePackageTarget[] = ["frontend", "backend"]): void {
  Object.assign(state, createReleasePackageRuntimeState(), {
    activeEnvironmentId: environmentId,
    pendingEnvironmentId: environmentId,
    status: "running",
  });
  environmentRuntimes.set(environmentId, createEnvironmentRuntime(targets));
  getMutableEnvironmentRuntime(environmentId).status = "running";
}

function bindStartedRun(runId: string, environmentId: number): void {
  state.activeRunId = runId;
  state.activeEnvironmentId = environmentId;
  state.pendingEnvironmentId = environmentId;
  getMutableEnvironmentRuntime(environmentId).runId = runId;
}

function abortStart(message: string): void {
  const environmentId = state.pendingEnvironmentId ?? state.activeEnvironmentId;
  if (environmentId !== null) {
    const runtime = getMutableEnvironmentRuntime(environmentId);
    runtime.status = "failed";
    runtime.error = message;
  }
  state.activeRunId = null;
  state.activeEnvironmentId = null;
  state.pendingEnvironmentId = null;
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
  environmentRuntimes.clear();
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
    getEnvironmentRuntime,
    reset,
  };
}
