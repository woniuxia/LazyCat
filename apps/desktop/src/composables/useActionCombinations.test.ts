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
  ActionCombinationSummary,
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

function combinationSummary(
  latestRunStatus?: ActionCombinationSummary["latestRunStatus"],
): ActionCombinationSummary {
  return {
    id: 7,
    name: "开发环境",
    executionMode: "serial",
    stepCount: 2,
    latestRunStatus,
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
    listenMock
      .mockReset()
      .mockImplementation(async (name: string, handler: (event: { payload: unknown }) => void) => {
        listeners.set(name, handler);
        return vi.fn();
      });
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

  it("clears cached targets synchronously when the action changes", async () => {
    const second = deferred<{ targets: ActionCombinationTarget[] }>();
    invokeMock
      .mockResolvedValueOnce({ targets: [target("hosts")] })
      .mockReturnValueOnce(second.promise);
    const state = useActionCombinations();
    await state.loadStepTargets("step-1", "hosts.activate");
    expect(state.stepTargets.value.get("step-1")?.[0].id).toBe("hosts");

    const loading = state.loadStepTargets("step-1", "browser_profile.launch");

    expect(state.stepTargets.value.has("step-1")).toBe(false);
    second.resolve({ targets: [target("edge")] });
    await loading;
    expect(state.stepTargets.value.get("step-1")?.map((item) => item.id)).toEqual(["edge"]);
  });

  it("restores a running combination from summaries and recent runs", async () => {
    const restored = runningRun("run-restored");
    invokeMock
      .mockResolvedValueOnce({ definitions: [] })
      .mockResolvedValueOnce({
        combinations: [
          {
            id: 7,
            name: "开发环境",
            executionMode: "serial",
            stepCount: 2,
            latestRunStatus: "running",
            updatedAt: "2026-07-26 10:00:00",
          },
        ],
      })
      .mockResolvedValueOnce({ runs: [restored] });
    const state = useActionCombinations({ pollIntervalMs: 10_000 });
    await state.start();
    expect(invokeMock).toHaveBeenCalledWith("tool:action-center:combination-run-list", {
      combinationId: 7,
    });
    expect(state.activeRun.value?.id).toBe("run-restored");
    expect(state.runActive.value).toBe(true);
    state.stop();
  });

  it("does not restore an active run after stop", async () => {
    vi.useFakeTimers();
    const runListResponse = deferred<{ runs: ActionCombinationRunDetail[] }>();
    invokeMock
      .mockResolvedValueOnce({ definitions: [] })
      .mockResolvedValueOnce({
        combinations: [
          {
            id: 7,
            name: "开发环境",
            executionMode: "serial",
            stepCount: 2,
            latestRunStatus: "running",
            updatedAt: "2026-07-26 10:00:00",
          },
        ],
      })
      .mockReturnValueOnce(runListResponse.promise);
    const state = useActionCombinations({ pollIntervalMs: 5 });
    const starting = state.start();
    for (let attempt = 0; attempt < 10 && invokeMock.mock.calls.length < 3; attempt += 1) {
      await Promise.resolve();
    }
    expect(invokeMock).toHaveBeenCalledWith("tool:action-center:combination-run-list", {
      combinationId: 7,
    });

    state.stop();
    runListResponse.resolve({ runs: [runningRun("run-restored")] });
    await starting;

    expect(state.activeRun.value).toBeNull();
    expect(state.runActive.value).toBe(false);
    const callsAfterStop = invokeMock.mock.calls.length;
    await vi.advanceTimersByTimeAsync(20);
    expect(invokeMock).toHaveBeenCalledTimes(callsAfterStop);
  });

  it("keeps listener disposers isolated across stop and restart", async () => {
    const firstRegistration = deferred<() => void>();
    const secondRegistration = deferred<() => void>();
    const firstDispose = vi.fn();
    const secondDispose = vi.fn();
    listenMock
      .mockReturnValueOnce(firstRegistration.promise)
      .mockReturnValueOnce(secondRegistration.promise);
    invokeMock
      .mockResolvedValueOnce({ definitions: [] })
      .mockResolvedValueOnce({ combinations: [] })
      .mockResolvedValueOnce({ definitions: [] })
      .mockResolvedValueOnce({ combinations: [] });
    const state = useActionCombinations();

    const firstStart = state.start();
    state.stop();
    const secondStart = state.start();

    secondRegistration.resolve(secondDispose);
    await secondStart;
    firstRegistration.resolve(firstDispose);
    await firstStart;

    expect(firstDispose).toHaveBeenCalledTimes(1);
    expect(secondDispose).not.toHaveBeenCalled();

    state.stop();
    expect(firstDispose).toHaveBeenCalledTimes(1);
    expect(secondDispose).toHaveBeenCalledTimes(1);
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

  it("keeps the saved identity when read refresh fails and retries as an update", async () => {
    invokeMock
      .mockResolvedValueOnce({ id: 7 })
      .mockRejectedValueOnce(new Error("列表刷新失败"))
      .mockResolvedValueOnce({ id: 7 })
      .mockResolvedValueOnce({ combinations: [combinationSummary()] })
      .mockResolvedValueOnce(combinationDetail(7))
      .mockResolvedValueOnce({ runs: [] });
    const state = useActionCombinations();
    state.createCombination();
    if (!state.draft.value) throw new Error("draft missing");
    state.draft.value.name = "开发环境";

    const firstResult = await state.saveCombination();

    expect(firstResult).toEqual({ id: 7, refreshError: "列表刷新失败" });
    expect(state.draft.value.id).toBe(7);
    expect(state.selectedId.value).toBe(7);
    expect(state.dirty.value).toBe(false);

    const secondResult = await state.saveCombination();
    const saveCalls = invokeMock.mock.calls.filter(
      ([channel]) => channel === "tool:action-center:combination-save",
    );
    expect(secondResult).toEqual({ id: 7 });
    expect(saveCalls).toHaveLength(2);
    expect(saveCalls[0][1]).not.toHaveProperty("id");
    expect(saveCalls[1][1]).toMatchObject({ id: 7 });
    expect(state.draft.value.id).toBe(7);
    expect(state.dirty.value).toBe(false);
  });

  it("keeps the saved identity when detail refresh fails", async () => {
    invokeMock
      .mockResolvedValueOnce({ id: 7 })
      .mockResolvedValueOnce({ combinations: [combinationSummary()] })
      .mockRejectedValueOnce(new Error("详情刷新失败"));
    const state = useActionCombinations();
    state.createCombination();
    if (!state.draft.value) throw new Error("draft missing");
    state.draft.value.name = "开发环境";

    const result = await state.saveCombination();

    expect(result).toEqual({ id: 7, refreshError: "详情刷新失败" });
    expect(state.draft.value.id).toBe(7);
    expect(state.selectedId.value).toBe(7);
    expect(state.dirty.value).toBe(false);
  });

  it("clears a stale selected detail as soon as the write succeeds", async () => {
    invokeMock
      .mockResolvedValueOnce(combinationDetail(7))
      .mockResolvedValueOnce({ runs: [] })
      .mockResolvedValueOnce({ id: 7 })
      .mockRejectedValueOnce(new Error("列表刷新失败"));
    const state = useActionCombinations();
    await state.selectCombination(7);
    if (!state.draft.value) throw new Error("draft missing");
    state.draft.value.name = "已重命名";

    const result = await state.saveCombination();

    expect(result).toEqual({ id: 7, refreshError: "列表刷新失败" });
    expect(state.selectedCombination.value).toBeNull();
    expect(state.draft.value.id).toBe(7);
    expect(state.dirty.value).toBe(false);
  });

  it("applies list structure while preserving the active run status", async () => {
    const listResponse = deferred<{ combinations: ActionCombinationSummary[] }>();
    invokeMock.mockReturnValueOnce(listResponse.promise);
    const state = useActionCombinations({ pollIntervalMs: 10_000 });
    state.combinations.value = [
      combinationSummary(),
      { ...combinationSummary(), id: 8, name: "待删除" },
    ];
    const loading = state.loadCombinations();
    await state.trackRun(runningRun("run-current"));
    listResponse.resolve({
      combinations: [
        { ...combinationSummary("failed"), name: "已重命名" },
        { ...combinationSummary(), id: 9, name: "新增组合" },
      ],
    });
    await loading;
    const summaries = state.combinations.value;
    state.stop();

    expect(summaries.map((summary) => summary.id)).toEqual([7, 9]);
    expect(summaries[0]).toMatchObject({ name: "已重命名", latestRunStatus: "running" });
    expect(summaries[1]).toMatchObject({ id: 9, name: "新增组合" });
    expect(summaries.some((summary) => summary.id === 8)).toBe(false);
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
    state.combinations.value = [combinationSummary()];

    const running = state.runCombination(7);
    state.stop();
    runResponse.resolve(runningRun("run-after-stop"));
    await running;

    expect(state.activeRun.value).toBeNull();
    expect(state.runActive.value).toBe(false);
    expect(state.combinations.value[0].latestRunStatus).toBeUndefined();
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
    state.combinations.value = [combinationSummary("running")];
    await state.trackRun(runningRun("run-1"));

    emitRunUpdate({ runId: "run-1", status: "succeeded" });
    await Promise.resolve();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("tool:action-center:combination-run-get", {
      runId: "run-1",
    });
    expect(state.activeRun.value?.status).toBe("succeeded");
    expect(state.combinations.value[0].latestRunStatus).toBe("succeeded");

    const callsAfterTerminal = invokeMock.mock.calls.length;
    await vi.advanceTimersByTimeAsync(20);
    expect(invokeMock).toHaveBeenCalledTimes(callsAfterTerminal);
    state.stop();
  });

  it("updates the summary when polling reaches a terminal run", async () => {
    vi.useFakeTimers();
    const succeeded = { ...runningRun("run-poll"), status: "succeeded" as const };
    invokeMock.mockResolvedValueOnce(succeeded);
    const state = useActionCombinations({ pollIntervalMs: 5 });
    state.combinations.value = [combinationSummary("running")];
    await state.trackRun(runningRun("run-poll"));

    await vi.advanceTimersByTimeAsync(5);

    expect(state.activeRun.value?.status).toBe("succeeded");
    expect(state.combinations.value[0].latestRunStatus).toBe("succeeded");
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
