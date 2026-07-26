import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, listenMock, listeners } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
}));

vi.mock("../bridge/tauri", () => ({ invokeToolByChannel: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import type {
  ActionCombinationDetail,
  ActionCombinationRunDetail,
  ActionCombinationTarget,
} from "../types/action-center";
import { useActionCombinations } from "./useActionCombinations";

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function target(id: string): ActionCombinationTarget {
  return { id, label: id, available: true };
}

function runningRun(id: string, combinationId = 7): ActionCombinationRunDetail {
  return {
    id,
    combinationId,
    combinationName: "开发环境",
    executionMode: "serial",
    status: "running",
    createdAt: "2026-07-26 10:00:00",
    startedAt: "2026-07-26 10:00:01",
    steps: [],
  };
}

function combinationDetail(id: number): ActionCombinationDetail {
  return {
    id,
    name: `组合 ${id}`,
    executionMode: "serial",
    steps: [],
    createdAt: "2026-07-26 10:00:00",
    updatedAt: "2026-07-26 10:00:00",
  };
}

function emitRunUpdate(payload: unknown): void {
  listeners.get("action-center://combination-run-updated")?.({ payload });
}

describe("useActionCombinations", () => {
  beforeEach(() => {
    vi.useRealTimers();
    invokeMock.mockReset();
    listenMock.mockReset().mockImplementation(
      async (name: string, handler: (event: { payload: unknown }) => void) => {
        listeners.set(name, handler);
        return vi.fn();
      },
    );
    listeners.clear();
  });

  it("ignores target responses for an old action selection", async () => {
    const first = deferred<{ targets: ActionCombinationTarget[] }>();
    invokeMock
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce({ targets: [target("edge")] });
    const state = useActionCombinations({ pollIntervalMs: 10_000 });

    const oldRequest = state.loadStepTargets("step-1", "hosts.activate");
    await state.loadStepTargets("step-1", "browser_profile.launch");
    first.resolve({ targets: [target("hosts")] });
    await oldRequest;

    expect(state.stepTargets.value.get("step-1")?.[0].id).toBe("edge");
  });

  it("restores a running combination from summaries and recent runs", async () => {
    const restored = runningRun("run-restored");
    invokeMock
      .mockResolvedValueOnce({ definitions: [] })
      .mockResolvedValueOnce({ combinations: [{
        id: 7, name: "开发环境", executionMode: "serial", stepCount: 2,
        latestRunStatus: "running", updatedAt: "2026-07-26 10:00:00",
      }] })
      .mockResolvedValueOnce({ runs: [restored] });
    const state = useActionCombinations({ pollIntervalMs: 10_000 });
    await state.start();
    expect(invokeMock).toHaveBeenCalledWith(
      "tool:action-center:combination-run-list", { combinationId: 7 },
    );
    expect(state.activeRun.value?.id).toBe("run-restored");
    expect(state.runActive.value).toBe(true);
    state.stop();
  });

  it("does not restore an active run after stop", async () => {
    vi.useFakeTimers();
    const runListResponse = deferred<{ runs: ActionCombinationRunDetail[] }>();
    invokeMock
      .mockResolvedValueOnce({ definitions: [] })
      .mockResolvedValueOnce({ combinations: [{
        id: 7, name: "开发环境", executionMode: "serial", stepCount: 2,
        latestRunStatus: "running", updatedAt: "2026-07-26 10:00:00",
      }] })
      .mockReturnValueOnce(runListResponse.promise);
    const state = useActionCombinations({ pollIntervalMs: 5 });
    const starting = state.start();
    for (let attempt = 0; attempt < 10 && invokeMock.mock.calls.length < 3; attempt += 1) {
      await Promise.resolve();
    }
    expect(invokeMock).toHaveBeenCalledWith(
      "tool:action-center:combination-run-list", { combinationId: 7 },
    );

    state.stop();
    runListResponse.resolve({ runs: [runningRun("run-restored")] });
    await starting;

    expect(state.activeRun.value).toBeNull();
    expect(state.runActive.value).toBe(false);
    const callsAfterStop = invokeMock.mock.calls.length;
    await vi.advanceTimersByTimeAsync(20);
    expect(invokeMock).toHaveBeenCalledTimes(callsAfterStop);
  });

  it("does not let an older history response overwrite the current selection", async () => {
    const oldHistory = deferred<{ runs: ActionCombinationRunDetail[] }>();
    const runA = runningRun("run-a", 7);
    const runB = runningRun("run-b", 8);
    invokeMock
      .mockResolvedValueOnce(combinationDetail(7))
      .mockReturnValueOnce(oldHistory.promise)
      .mockResolvedValueOnce(combinationDetail(8))
      .mockResolvedValueOnce({ runs: [runB] });
    const state = useActionCombinations();
    const selectA = state.selectCombination(7);
    await Promise.resolve();
    await state.selectCombination(8);
    oldHistory.resolve({ runs: [runA] });
    await selectA;
    expect(state.selectedId.value).toBe(8);
    expect(state.runHistory.value.map((run) => run.id)).toEqual(["run-b"]);
  });

  it("sets operation pending synchronously and rejects a duplicate save", async () => {
    const saveResponse = deferred<{ id: number }>();
    invokeMock
      .mockReturnValueOnce(saveResponse.promise)
      .mockResolvedValueOnce({ combinations: [] })
      .mockResolvedValueOnce(combinationDetail(7))
      .mockResolvedValueOnce({ runs: [] });
    const state = useActionCombinations();
    state.createCombination();
    const firstSave = state.saveCombination();
    expect(state.saving.value).toBe(true);
    expect(state.operationPending.value).toBe(true);
    await expect(state.saveCombination()).rejects.toThrow("组合动作操作正在进行中");
    expect(invokeMock).toHaveBeenCalledTimes(1);
    saveResponse.resolve({ id: 7 });
    await firstSave;
    expect(state.saving.value).toBe(false);
    expect(state.operationPending.value).toBe(false);
  });

  it("sets operation pending synchronously and rejects a duplicate run", async () => {
    const runResponse = deferred<ActionCombinationRunDetail>();
    invokeMock.mockReturnValueOnce(runResponse.promise);
    const state = useActionCombinations({ pollIntervalMs: 10_000 });
    const firstRun = state.runCombination(7);
    expect(state.starting.value).toBe(true);
    expect(state.operationPending.value).toBe(true);
    await expect(state.runCombination(7)).rejects.toThrow("组合动作操作正在进行中");
    expect(invokeMock).toHaveBeenCalledTimes(1);
    runResponse.resolve(runningRun("run-1"));
    await firstRun;
    expect(state.starting.value).toBe(false);
    expect(state.operationPending.value).toBe(false);
    state.stop();
  });

  it("does not track or poll a run response that arrives after stop", async () => {
    vi.useFakeTimers();
    const runResponse = deferred<ActionCombinationRunDetail>();
    invokeMock.mockReturnValueOnce(runResponse.promise);
    const state = useActionCombinations({ pollIntervalMs: 5 });

    const running = state.runCombination(7);
    state.stop();
    runResponse.resolve(runningRun("run-after-stop"));
    await running;

    expect(state.activeRun.value).toBeNull();
    expect(state.runActive.value).toBe(false);
    const callsAfterStop = invokeMock.mock.calls.length;
    await vi.advanceTimersByTimeAsync(20);
    expect(invokeMock).toHaveBeenCalledTimes(callsAfterStop);
  });

  it("reloads the active run from an event and stops polling at terminal state", async () => {
    vi.useFakeTimers();
    const succeeded = { ...runningRun("run-1"), status: "succeeded" as const };
    invokeMock
      .mockResolvedValueOnce({ definitions: [] })
      .mockResolvedValueOnce({ combinations: [] })
      .mockResolvedValueOnce(succeeded)
      .mockResolvedValueOnce({ runs: [succeeded] });
    const state = useActionCombinations({ pollIntervalMs: 5 });
    await state.start();
    await state.trackRun(runningRun("run-1"));

    emitRunUpdate({ runId: "run-1", status: "succeeded" });
    await Promise.resolve();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith(
      "tool:action-center:combination-run-get",
      { runId: "run-1" },
    );
    expect(state.activeRun.value?.status).toBe("succeeded");

    const callsAfterTerminal = invokeMock.mock.calls.length;
    await vi.advanceTimersByTimeAsync(20);
    expect(invokeMock).toHaveBeenCalledTimes(callsAfterTerminal);
    state.stop();
  });

  it("does not let an older poll response overwrite a terminal event refresh", async () => {
    vi.useFakeTimers();
    const oldPoll = deferred<ActionCombinationRunDetail>();
    const succeeded = { ...runningRun("run-1"), status: "succeeded" as const };
    invokeMock
      .mockResolvedValueOnce({ definitions: [] })
      .mockResolvedValueOnce({ combinations: [] })
      .mockReturnValueOnce(oldPoll.promise)
      .mockResolvedValueOnce(succeeded)
      .mockResolvedValueOnce({ runs: [succeeded] });
    const state = useActionCombinations({ pollIntervalMs: 5 });
    await state.start();
    await state.trackRun(runningRun("run-1"));

    vi.advanceTimersByTime(5);
    emitRunUpdate({ runId: "run-1", status: "succeeded" });
    await Promise.resolve();
    await Promise.resolve();
    expect(state.activeRun.value?.status).toBe("succeeded");

    oldPoll.resolve(runningRun("run-1"));
    await Promise.resolve();
    await Promise.resolve();

    expect(state.activeRun.value?.status).toBe("succeeded");
    state.stop();
  });
});
