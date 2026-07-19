import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, reactive, ref, toRefs } from "vue";
import { APP_EVENTS } from "../bridge/events";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ReleasePackageLogEvent,
  ReleasePackagePhase,
  ReleasePackageRunStatus,
  ReleasePackageStatusEvent,
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
  state.status = event.status;
  state.phase = event.phase;
  state.archivePath = event.archivePath ?? state.archivePath;
  state.error = event.error ?? "";
  if (event.status !== "running") state.pendingProjectId = null;
}

const state = reactive(createReleasePackageRuntimeState());
const logs = ref<ReleasePackageLogEvent[]>([]);
let listenerPromise: Promise<UnlistenFn[]> | null = null;

function acceptPendingEvent(runId: string, projectId: number): boolean {
  if (!state.activeRunId && state.pendingProjectId === projectId) {
    state.activeRunId = runId;
  }
  return acceptReleasePackageEvent(state.activeRunId, { runId });
}

function ensureListeners(): Promise<void> {
  if (!listenerPromise) {
    listenerPromise = Promise.all([
      listen<ReleasePackageLogEvent>(APP_EVENTS.RELEASE_PACKAGE_LOG, ({ payload }) => {
        if (!acceptPendingEvent(payload.runId, payload.projectId)) return;
        state.activeProjectId = payload.projectId;
        logs.value = appendReleasePackageLog(logs.value, payload, 2_000);
      }),
      listen<ReleasePackageStatusEvent>(APP_EVENTS.RELEASE_PACKAGE_STATUS, ({ payload }) => {
        reduceReleasePackageStatus(state, payload);
      }),
    ]);
  }
  return listenerPromise.then(() => undefined);
}

function beginStart(projectId: number): void {
  Object.assign(state, createReleasePackageRuntimeState(), {
    pendingProjectId: projectId,
    status: "running",
  });
  logs.value = [];
}

function bindStartedRun(runId: string, projectId: number): void {
  state.activeRunId = runId;
  state.activeProjectId = projectId;
  state.pendingProjectId = projectId;
}

function abortStart(message: string): void {
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
  logs.value = [];
}

export function useReleasePackageRuntime() {
  return {
    ...toRefs(state),
    logs,
    isRunning: computed(() => state.status === "running"),
    ensureListeners,
    beginStart,
    bindStartedRun,
    abortStart,
    cancel,
    reset,
  };
}
