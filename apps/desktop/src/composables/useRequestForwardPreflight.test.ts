import { describe, expect, it } from "vitest";
import type {
  RequestForwardPreflightResult,
  RequestForwardRuleWriteInput,
} from "../types/request-forward";
import type { RequestForwardSelectionIntentState } from "../utils/requestForward";
import { useRequestForwardPreflight } from "./useRequestForwardPreflight";

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function createPayload(
  overrides: Partial<RequestForwardRuleWriteInput> = {},
): RequestForwardRuleWriteInput {
  return {
    name: "本地服务",
    protocol: "http",
    bindHost: "127.0.0.1",
    listenPort: 8080,
    targetUrl: "http://127.0.0.1:3000",
    targetHost: null,
    targetPort: null,
    captureHttpHeaders: true,
    captureHttpBody: false,
    ...overrides,
  };
}

function createResult(ready: boolean): RequestForwardPreflightResult {
  return {
    checks: [],
    suggestedListenPort: null,
    ready,
  };
}

function createHarness() {
  let intent: RequestForwardSelectionIntentState = {
    selectionToken: 1,
    selectedId: null,
    draft: true,
  };
  let payload = createPayload();
  const requests: Array<Deferred<RequestForwardPreflightResult>> = [];
  const errors: unknown[] = [];
  const preflight = useRequestForwardPreflight({
    currentContext: () => ({ intent, payload }),
    execute: () => {
      const request = deferred<RequestForwardPreflightResult>();
      requests.push(request);
      return request.promise;
    },
    onError: (error) => errors.push(error),
  });

  return {
    ...preflight,
    requests,
    errors,
    setIntent(next: RequestForwardSelectionIntentState) {
      intent = next;
    },
    setPayload(next: RequestForwardRuleWriteInput) {
      payload = next;
    },
  };
}

describe("useRequestForwardPreflight", () => {
  it("applies the current successful result and unlocks loading", async () => {
    const harness = createHarness();

    const run = harness.run();
    expect(harness.loading.value).toBe(true);
    harness.requests[0].resolve(createResult(true));

    await expect(run).resolves.toEqual(createResult(true));
    expect(harness.result.value).toEqual(createResult(true));
    expect(harness.loading.value).toBe(false);
    expect(harness.isAcceptedCurrent()).toBe(true);
  });

  it("discards a successful response after the payload changes", async () => {
    const harness = createHarness();

    const run = harness.run();
    harness.setPayload(createPayload({ listenPort: 9090 }));
    harness.requests[0].resolve(createResult(true));

    await expect(run).resolves.toBeNull();
    expect(harness.result.value).toBeNull();
    expect(harness.loading.value).toBe(false);
    expect(harness.errors).toEqual([]);
  });

  it("suppresses a failed response after the editor intent changes", async () => {
    const harness = createHarness();
    const failure = new Error("旧规则检测失败");

    const run = harness.run();
    harness.setIntent({ selectionToken: 2, selectedId: 42, draft: false });
    harness.requests[0].reject(failure);

    await expect(run).resolves.toBeNull();
    expect(harness.result.value).toBeNull();
    expect(harness.loading.value).toBe(false);
    expect(harness.errors).toEqual([]);
  });

  it("does not let an older concurrent response unlock the latest request", async () => {
    const harness = createHarness();

    const firstRun = harness.run();
    const secondRun = harness.run();
    harness.requests[0].resolve(createResult(false));

    await expect(firstRun).resolves.toBeNull();
    expect(harness.result.value).toBeNull();
    expect(harness.loading.value).toBe(true);

    harness.requests[1].resolve(createResult(true));
    await expect(secondRun).resolves.toEqual(createResult(true));
    expect(harness.result.value).toEqual(createResult(true));
    expect(harness.loading.value).toBe(false);
  });

  it("invalidates an accepted result when any normalized payload field changes", async () => {
    const harness = createHarness();

    const run = harness.run();
    harness.requests[0].resolve(createResult(true));
    await run;
    expect(harness.isAcceptedCurrent()).toBe(true);

    harness.setPayload(createPayload({ captureHttpBody: true }));

    expect(harness.isAcceptedCurrent()).toBe(false);
  });

  it("invalidates an accepted result when any selection intent field changes", async () => {
    const harness = createHarness();

    const run = harness.run();
    harness.requests[0].resolve(createResult(true));
    await run;
    expect(harness.isAcceptedCurrent()).toBe(true);

    harness.setIntent({ selectionToken: 1, selectedId: null, draft: false });

    expect(harness.isAcceptedCurrent()).toBe(false);
  });

  it("leaves no side effects when invalidated before a late response", async () => {
    const harness = createHarness();

    const run = harness.run();
    harness.invalidate();
    expect(harness.loading.value).toBe(false);
    harness.requests[0].resolve(createResult(true));

    await expect(run).resolves.toBeNull();
    expect(harness.result.value).toBeNull();
    expect(harness.loading.value).toBe(false);
    expect(harness.isAcceptedCurrent()).toBe(false);
    expect(harness.errors).toEqual([]);
  });

  it("reports only a current execution failure and unlocks loading", async () => {
    const harness = createHarness();
    const failure = new Error("检测失败");

    const run = harness.run();
    harness.requests[0].reject(failure);

    await expect(run).resolves.toBeNull();
    expect(harness.errors).toEqual([failure]);
    expect(harness.loading.value).toBe(false);
  });
});
