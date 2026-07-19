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

function status(runId: string, projectId: number, value: ReleasePackageStatusEvent["status"]): ReleasePackageStatusEvent {
  return { runId, projectId, status: value, phase: value === "succeeded" ? "archive" : "frontend" };
}

function log(runId: string, projectId: number, line: string): ReleasePackageLogEvent {
  return { runId, projectId, phase: "frontend", stream: "stdout", line };
}

describe("release package runtime state", () => {
  beforeEach(() => {
    invokeMock.mockReset().mockResolvedValue({ cancelRequested: true });
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
});
