import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ReleasePackageEnvironmentKind,
  ReleasePackageLogEvent,
  ReleasePackageStatusEvent,
} from "../types/release-package";

const { listenMock, invokeMock, listeners } = vi.hoisted(() => ({
  listenMock: vi.fn(),
  invokeMock: vi.fn(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("../bridge/tauri", () => ({ invokeToolByChannel: invokeMock }));

import {
  createReleasePackageRuntimeState,
  reduceReleasePackageStatus,
  useReleasePackageRuntime,
} from "./useReleasePackageRuntime";

function emit(name: string, payload: unknown): void {
  listeners.get(name)?.({ payload });
}

function status(
  runId: string,
  environmentId: number,
  value: ReleasePackageStatusEvent["status"],
  phase: ReleasePackageStatusEvent["phase"] = "overall",
  environment: ReleasePackageEnvironmentKind = "test",
): ReleasePackageStatusEvent {
  return { runId, environmentId, projectId: 7, environment, status: value, phase };
}

function log(
  runId: string,
  environmentId: number,
  line: string,
  phase: ReleasePackageLogEvent["phase"] = "frontend",
  environment: ReleasePackageEnvironmentKind = "test",
): ReleasePackageLogEvent {
  return {
    runId,
    environmentId,
    projectId: 7,
    environment,
    phase,
    stream: "stdout",
    line,
  };
}

describe("release package runtime state", () => {
  beforeEach(() => {
    invokeMock.mockReset().mockResolvedValue({ cancelRequested: true });
    useReleasePackageRuntime().reset();
  });

  it("binds the first event while start is pending and rejects stale runs", () => {
    const state = createReleasePackageRuntimeState();
    state.pendingEnvironmentId = 41;
    reduceReleasePackageStatus(state, status("run-1", 41, "running"));
    expect(state.activeRunId).toBe("run-1");

    reduceReleasePackageStatus(state, {
      ...status("old-run", 41, "failed"),
      error: "old",
    });
    expect(state.status).toBe("running");
  });

  it("keeps the final archive path on success", () => {
    const state = createReleasePackageRuntimeState();
    state.activeRunId = "run-1";
    state.activeEnvironmentId = 41;
    reduceReleasePackageStatus(state, {
      ...status("run-1", 41, "succeeded"),
      archivePath: "D:\\releases\\20260723-客户门户",
    });
    expect(state.status).toBe("succeeded");
    expect(state.archivePath).toContain("20260723-客户门户");
  });

  it("keeps the overall run active when one target reaches a terminal state", () => {
    const state = createReleasePackageRuntimeState();
    state.activeRunId = "run-1";
    state.activeEnvironmentId = 41;
    state.pendingEnvironmentId = 41;
    state.status = "running";

    reduceReleasePackageStatus(state, status("run-1", 41, "succeeded", "frontend"));

    expect(state.status).toBe("running");
    expect(state.pendingEnvironmentId).toBe(41);
  });

  it("registers singleton listeners, filters stale events, and retains terminal state", async () => {
    listenMock.mockImplementation(
      async (name: string, handler: (event: { payload: unknown }) => void) => {
        listeners.set(name, handler);
        return vi.fn();
      },
    );
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();
    await runtime.ensureListeners();
    expect(listenMock).toHaveBeenCalledTimes(2);

    runtime.beginStart(41);
    emit("release-package://log", log("run-1", 41, "frontend ready"));
    emit("release-package://log", log("old-run", 41, "stale"));
    expect(runtime.activeRunId.value).toBe("run-1");
    expect(runtime.logs.value.map((entry) => entry.line)).toEqual(["frontend ready"]);

    runtime.bindStartedRun("run-1", 41);
    emit("release-package://status", status("run-1", 41, "succeeded"));
    expect(runtime.status.value).toBe("succeeded");
    expect(runtime.pendingEnvironmentId.value).toBeNull();

    expect(await runtime.cancel()).toBe(false);
    expect(invokeMock).not.toHaveBeenCalled();
    runtime.reset();
    expect(runtime.status.value).toBe("idle");
    expect(runtime.logs.value).toEqual([]);
  });

  it("keeps runtimes isolated by environment within the same project and log column", async () => {
    listenMock.mockImplementation(
      async (name: string, handler: (event: { payload: unknown }) => void) => {
        listeners.set(name, handler);
        return vi.fn();
      },
    );
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();

    runtime.beginStart(41, ["frontend", "backend"]);
    runtime.bindStartedRun("run-1", 41);
    emit("release-package://log", log("run-1", 41, "web"));
    emit("release-package://status", status("run-1", 41, "succeeded", "frontend"));
    emit("release-package://status", status("run-1", 41, "partially_succeeded"));

    runtime.beginStart(42, ["backend"]);
    runtime.bindStartedRun("run-2", 42);
    emit("release-package://log", log("run-2", 42, "server", "backend", "production"));
    emit("release-package://log", log("run-1", 41, "late"));

    const testRuntime = runtime.getEnvironmentRuntime(41);
    expect(testRuntime.projectId).toBe(7);
    expect(testRuntime.environment).toBe("test");
    expect(testRuntime.status).toBe("partially_succeeded");
    expect(testRuntime.targetStatus.frontend).toBe("succeeded");
    expect(testRuntime.frontendLogs.map((entry) => entry.line)).toEqual(["web"]);

    const productionRuntime = runtime.getEnvironmentRuntime(42);
    expect(productionRuntime.projectId).toBe(7);
    expect(productionRuntime.environment).toBe("production");
    expect(productionRuntime.targetStatus.frontend).toBe("skipped");
    expect(productionRuntime.backendLogs.map((entry) => entry.line)).toEqual(["server"]);
  });

  it("clears previous archive paths immediately before a new upload run", async () => {
    listenMock.mockImplementation(
      async (name: string, handler: (event: { payload: unknown }) => void) => {
        listeners.set(name, handler);
        return vi.fn();
      },
    );
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();
    runtime.beginStart(41);
    runtime.bindStartedRun("archive-run", 41);
    emit("release-package://status", {
      ...status("archive-run", 41, "succeeded"),
      archivePath: "D:\\releases\\portal",
    });
    expect(runtime.archivePath.value).toBe("D:\\releases\\portal");
    expect(runtime.getEnvironmentRuntime(41).archivePath).toBe("D:\\releases\\portal");

    runtime.beginStart(41);
    expect(runtime.archivePath.value).toBe("");
    expect(runtime.getEnvironmentRuntime(41).archivePath).toBe("");

    runtime.bindStartedRun("upload-run", 41);
    emit("release-package://status", status("upload-run", 41, "succeeded"));

    expect(runtime.archivePath.value).toBe("");
    expect(runtime.getEnvironmentRuntime(41).archivePath).toBe("");
  });

  it("tracks upload logs, progress, failure retry token, and terminal running state", async () => {
    listenMock.mockImplementation(
      async (name: string, handler: (event: { payload: unknown }) => void) => {
        listeners.set(name, handler);
        return vi.fn();
      },
    );
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();
    runtime.beginStart(41, ["frontend", "backend"]);
    runtime.bindStartedRun("run-1", 41);

    emit("release-package://log", log("run-1", 41, "上传中", "upload"));
    emit("release-package://status", {
      ...status("run-1", 41, "uploading", "upload"),
      uploadedBytes: 50,
      totalBytes: 100,
      currentPath: "assets/app.js",
    });
    expect(runtime.isRunning.value).toBe(true);
    emit("release-package://status", {
      ...status("run-1", 41, "package_succeeded_upload_failed"),
      retryToken: "retry-1",
      error: "服务器上传失败",
    });

    const environmentRuntime = runtime.getEnvironmentRuntime(41);
    expect(environmentRuntime.uploadLogs.map((entry) => entry.line)).toEqual(["上传中"]);
    expect(environmentRuntime.uploadProgress).toEqual({
      uploadedBytes: 50,
      totalBytes: 100,
      currentPath: "assets/app.js",
    });
    expect(environmentRuntime.retryToken).toBe("retry-1");
    expect(environmentRuntime.commandRetryToken).toBe("");
    expect(environmentRuntime.archivePath).toBe("");
    expect(runtime.archivePath.value).toBe("");
    expect(runtime.isRunning.value).toBe(false);
  });

  it("tracks failed post-upload commands without overwriting the upload retry token", async () => {
    listenMock.mockImplementation(
      async (name: string, handler: (event: { payload: unknown }) => void) => {
        listeners.set(name, handler);
        return vi.fn();
      },
    );
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();
    runtime.beginStart(41, ["frontend", "backend"]);
    runtime.bindStartedRun("run-1", 41);
    emit("release-package://status", {
      ...status("run-1", 41, "uploading", "upload"),
      uploadedBytes: 512,
      totalBytes: 1_024,
      currentPath: "assets/app.js",
    });

    emit("release-package://status", {
      ...status("run-1", 41, "failed", "upload"),
      commandTarget: "frontend",
      commandStatus: "failed",
      error: "退出码 7",
    });
    emit("release-package://status", {
      ...status("run-1", 41, "upload_succeeded_command_failed"),
      commandRetryToken: "command-retry-1",
    });

    const environmentRuntime = runtime.getEnvironmentRuntime(41);
    expect(environmentRuntime.commandStatus.frontend).toBe("failed");
    expect(environmentRuntime.commandErrors.frontend).toBe("退出码 7");
    expect(environmentRuntime.commandStatus.backend).toBe("pending");
    expect(environmentRuntime.commandRetryToken).toBe("command-retry-1");
    expect(environmentRuntime.retryToken).toBe("");
    expect(environmentRuntime.uploadProgress).toEqual({
      uploadedBytes: 512,
      totalBytes: 1_024,
      currentPath: "assets/app.js",
    });
  });

  it("locks a cancellation request until the terminal event arrives", async () => {
    listenMock.mockImplementation(
      async (name: string, handler: (event: { payload: unknown }) => void) => {
        listeners.set(name, handler);
        return vi.fn();
      },
    );
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();
    runtime.beginStart(41, ["frontend", "backend"]);
    runtime.bindStartedRun("run-1", 41);

    const first = runtime.cancel();
    const second = runtime.cancel();
    await expect(first).resolves.toBe(true);
    await expect(second).resolves.toBe(true);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(runtime.getEnvironmentRuntime(41).cancelRequested).toBe(true);

    emit("release-package://status", status("run-1", 41, "cancelled"));
    expect(runtime.getEnvironmentRuntime(41).cancelRequested).toBe(false);
  });

  it("keeps persistence warnings separate from delivery results and clears them for a new run", async () => {
    listenMock.mockImplementation(
      async (name: string, handler: (event: { payload: unknown }) => void) => {
        listeners.set(name, handler);
        return vi.fn();
      },
    );
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();
    runtime.beginStart(41);
    runtime.bindStartedRun("run-1", 41);
    emit("release-package://status", {
      ...status("run-1", 41, "running"),
      persistenceWarning: {
        action: "append lane",
        path: "D:\\logs\\frontend-start.log",
        cause: "Access denied",
      },
    });
    emit("release-package://status", {
      ...status("run-1", 41, "succeeded"),
      archivePath: "D:\\release\\portal",
    });

    const completed = runtime.getEnvironmentRuntime(41);
    expect(completed.status).toBe("succeeded");
    expect(completed.persistenceWarning).toEqual({
      action: "append lane",
      path: "D:\\logs\\frontend-start.log",
      cause: "Access denied",
    });

    runtime.beginStart(41);
    expect(runtime.getEnvironmentRuntime(41).persistenceWarning).toBeNull();
  });
});
