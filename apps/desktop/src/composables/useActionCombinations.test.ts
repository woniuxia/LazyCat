import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, listenMock, listeners } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
}));

vi.mock("../bridge/tauri", () => ({ invokeToolByChannel: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import type {
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

function runningRun(id: string): ActionCombinationRunDetail {
  return {
    id,
    combinationId: 7,
    combinationName: "开发环境",
    executionMode: "serial",
    status: "running",
    createdAt: "2026-07-26 10:00:00",
    startedAt: "2026-07-26 10:00:01",
    steps: [],
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
