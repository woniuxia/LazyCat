import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReleasePackageLogEvent, ReleasePackageStatusEvent } from "../types/release-package";

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
  projectId: number,
  value: ReleasePackageStatusEvent["status"],
  phase: ReleasePackageStatusEvent["phase"] = "overall",
): ReleasePackageStatusEvent {
  return { runId, projectId, status: value, phase };
}

function log(
  runId: string,
  projectId: number,
  line: string,
  phase: ReleasePackageLogEvent["phase"] = "frontend",
): ReleasePackageLogEvent {
  return { runId, projectId, phase, stream: "stdout", line };
}

describe("release package runtime state", () => {
  beforeEach(() => {
    invokeMock.mockReset().mockResolvedValue({ cancelRequested: true });
    useReleasePackageRuntime().reset();
  });

  it("binds the first event while start is pending and rejects stale runs", () => {
    const state = createReleasePackageRuntimeState();
    state.pendingProjectId = 7;
    reduceReleasePackageStatus(state, status("run-1", 7, "running"));
    expect(state.activeRunId).toBe("run-1");

    reduceReleasePackageStatus(state, {
      ...status("old-run", 7, "failed"),
      error: "old",
    });
    expect(state.status).toBe("running");
  });

  it("keeps the final archive path on success", () => {
    const state = createReleasePackageRuntimeState();
    state.activeRunId = "run-1";
    reduceReleasePackageStatus(state, {
      ...status("run-1", 7, "succeeded"),
      archivePath: "D:\\releases\\20260723-客户门户",
    });
    expect(state.status).toBe("succeeded");
    expect(state.archivePath).toContain("20260723-客户门户");
  });

  it("keeps the overall run active when one target reaches a terminal state", () => {
    const state = createReleasePackageRuntimeState();
    state.activeRunId = "run-1";
    state.activeProjectId = 7;
    state.pendingProjectId = 7;
    state.status = "running";

    reduceReleasePackageStatus(state, status("run-1", 7, "succeeded", "frontend"));

    expect(state.status).toBe("running");
    expect(state.pendingProjectId).toBe(7);
  });

  it("registers singleton listeners, filters stale events, and retains terminal state", async () => {
    listenMock.mockImplementation(async (name: string, handler: (event: { payload: unknown }) => void) => {
      listeners.set(name, handler);
      return vi.fn();
    });
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();
    await runtime.ensureListeners();
    expect(listenMock).toHaveBeenCalledTimes(2);

    runtime.beginStart(7);
    emit("release-package://log", log("run-1", 7, "frontend ready"));
    emit("release-package://log", log("old-run", 7, "stale"));
    expect(runtime.activeRunId.value).toBe("run-1");
    expect(runtime.logs.value.map((entry) => entry.line)).toEqual(["frontend ready"]);

    runtime.bindStartedRun("run-1", 7);
    emit("release-package://status", status("run-1", 7, "succeeded"));
    expect(runtime.status.value).toBe("succeeded");
    expect(runtime.pendingProjectId.value).toBeNull();

    await runtime.cancel();
    expect(invokeMock).toHaveBeenCalledWith("tool:release-package:cancel", { runId: "run-1" });
    runtime.reset();
    expect(runtime.status.value).toBe("idle");
    expect(runtime.logs.value).toEqual([]);
  });

  it("keeps the latest runtime isolated by project and log column", async () => {
    listenMock.mockImplementation(async (name: string, handler: (event: { payload: unknown }) => void) => {
      listeners.set(name, handler);
      return vi.fn();
    });
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();

    runtime.beginStart(7, ["frontend", "backend"]);
    runtime.bindStartedRun("run-1", 7);
    emit("release-package://log", log("run-1", 7, "web"));
    emit("release-package://status", status("run-1", 7, "succeeded", "frontend"));
    emit("release-package://status", status("run-1", 7, "partially_succeeded"));

    runtime.beginStart(8, ["backend"]);
    runtime.bindStartedRun("run-2", 8);
    emit("release-package://log", log("run-2", 8, "server", "backend"));
    emit("release-package://log", log("run-1", 7, "late"));

    expect(runtime.getProjectRuntime(7).status).toBe("partially_succeeded");
    expect(runtime.getProjectRuntime(7).targetStatus.frontend).toBe("succeeded");
    expect(runtime.getProjectRuntime(7).frontendLogs.map((entry) => entry.line)).toEqual(["web"]);
    expect(runtime.getProjectRuntime(8).targetStatus.frontend).toBe("skipped");
    expect(runtime.getProjectRuntime(8).backendLogs.map((entry) => entry.line)).toEqual(["server"]);
  });

  it("clears previous archive paths immediately before a new upload run", async () => {
    listenMock.mockImplementation(async (name: string, handler: (event: { payload: unknown }) => void) => {
      listeners.set(name, handler);
      return vi.fn();
    });
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();
    runtime.beginStart(7);
    runtime.bindStartedRun("archive-run", 7);
    emit("release-package://status", {
      ...status("archive-run", 7, "succeeded"),
      archivePath: "D:\\releases\\portal",
    });
    expect(runtime.archivePath.value).toBe("D:\\releases\\portal");
    expect(runtime.getProjectRuntime(7).archivePath).toBe("D:\\releases\\portal");

    runtime.beginStart(7);
    expect(runtime.archivePath.value).toBe("");
    expect(runtime.getProjectRuntime(7).archivePath).toBe("");

    runtime.bindStartedRun("upload-run", 7);
    emit("release-package://status", status("upload-run", 7, "succeeded"));

    expect(runtime.archivePath.value).toBe("");
    expect(runtime.getProjectRuntime(7).archivePath).toBe("");
  });

  it("tracks upload logs, progress, failure retry token, and terminal running state", async () => {
    listenMock.mockImplementation(async (name: string, handler: (event: { payload: unknown }) => void) => {
      listeners.set(name, handler);
      return vi.fn();
    });
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();
    runtime.beginStart(7, ["frontend", "backend"]);
    runtime.bindStartedRun("run-1", 7);

    emit("release-package://log", log("run-1", 7, "上传中", "upload"));
    emit("release-package://status", {
      ...status("run-1", 7, "uploading", "upload"),
      uploadedBytes: 50,
      totalBytes: 100,
      currentPath: "assets/app.js",
    });
    expect(runtime.isRunning.value).toBe(true);
    emit("release-package://status", {
      ...status("run-1", 7, "package_succeeded_upload_failed"),
      retryToken: "retry-1",
      error: "服务器上传失败",
    });

    const projectRuntime = runtime.getProjectRuntime(7);
    expect(projectRuntime.uploadLogs.map((entry) => entry.line)).toEqual(["上传中"]);
    expect(projectRuntime.uploadProgress).toEqual({
      uploadedBytes: 50,
      totalBytes: 100,
      currentPath: "assets/app.js",
    });
    expect(projectRuntime.retryToken).toBe("retry-1");
    expect(projectRuntime.commandRetryToken).toBe("");
    expect(projectRuntime.archivePath).toBe("");
    expect(runtime.archivePath.value).toBe("");
    expect(runtime.isRunning.value).toBe(false);
  });

  it("tracks failed post-upload commands without overwriting the upload retry token", async () => {
    listenMock.mockImplementation(async (name: string, handler: (event: { payload: unknown }) => void) => {
      listeners.set(name, handler);
      return vi.fn();
    });
    const runtime = useReleasePackageRuntime();
    await runtime.ensureListeners();
    runtime.beginStart(7, ["frontend", "backend"]);
    runtime.bindStartedRun("run-1", 7);
    emit("release-package://status", {
      ...status("run-1", 7, "uploading", "upload"),
      uploadedBytes: 512,
      totalBytes: 1_024,
      currentPath: "assets/app.js",
    });


    emit("release-package://status", {
      ...status("run-1", 7, "failed", "upload"),
      commandTarget: "frontend",
      commandStatus: "failed",
      error: "退出码 7",
    });
    emit("release-package://status", {
      ...status("run-1", 7, "upload_succeeded_command_failed"),
      commandRetryToken: "command-retry-1",
    });

    const projectRuntime = runtime.getProjectRuntime(7);
    expect(projectRuntime.commandStatus.frontend).toBe("failed");
    expect(projectRuntime.commandErrors.frontend).toBe("退出码 7");
    expect(projectRuntime.commandStatus.backend).toBe("pending");
    expect(projectRuntime.commandRetryToken).toBe("command-retry-1");
    expect(projectRuntime.retryToken).toBe("");
    expect(projectRuntime.uploadProgress).toEqual({
      uploadedBytes: 512,
      totalBytes: 1_024,
      currentPath: "assets/app.js",
    });
  });
});
