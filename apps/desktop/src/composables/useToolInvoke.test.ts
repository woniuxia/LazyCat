import { beforeEach, describe, expect, it, vi, type Mock } from "vitest";

vi.mock("../bridge/tauri", () => ({
  invokeToolByChannel: vi.fn(),
}));

vi.mock("element-plus", () => ({
  ElMessage: {
    error: vi.fn(),
  },
}));

import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import { useToolInvoke } from "./useToolInvoke";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((innerResolve, innerReject) => {
    resolve = innerResolve;
    reject = innerReject;
  });
  return { promise, resolve, reject };
}

const invokeMock = vi.mocked(invokeToolByChannel);
const messageErrorMock = ElMessage.error as unknown as Mock;

beforeEach(() => {
  vi.resetAllMocks();
});

describe("useToolInvoke", () => {
  it("invokeWithLoading returns data and toggles loading around the request", async () => {
    const pending = deferred<{ ok: boolean }>();
    invokeMock.mockReturnValueOnce(pending.promise);
    const { loading, invokeWithLoading } = useToolInvoke();

    const result = invokeWithLoading<{ ok: boolean }>("tool:test:list", { page: 1 });

    expect(loading.value).toBe(true);
    pending.resolve({ ok: true });
    await expect(result).resolves.toEqual({ ok: true });
    expect(loading.value).toBe(false);
    expect(invokeMock).toHaveBeenCalledWith("tool:test:list", { page: 1 });
  });

  it("invokeWithLoading reports errors by default and returns undefined", async () => {
    invokeMock.mockRejectedValueOnce(new Error("boom"));
    const { invokeWithLoading } = useToolInvoke();

    const result = await invokeWithLoading("tool:test:save", {});

    expect(result).toBeUndefined();
    expect(messageErrorMock).toHaveBeenCalledWith("boom");
  });

  it("invokeWithLoading prefixes error messages when requested", async () => {
    invokeMock.mockRejectedValueOnce(new Error("boom"));
    const { invokeWithLoading } = useToolInvoke();

    const result = await invokeWithLoading("tool:test:save", {}, { errorPrefix: "保存失败：" });

    expect(result).toBeUndefined();
    expect(messageErrorMock).toHaveBeenCalledWith("保存失败：boom");
  });

  it("invokeSilent returns undefined without reporting errors", async () => {
    invokeMock.mockRejectedValueOnce(new Error("background failed"));
    const { invokeSilent } = useToolInvoke();

    const result = await invokeSilent("tool:test:preload", {});

    expect(result).toBeUndefined();
    expect(messageErrorMock).not.toHaveBeenCalled();
  });

  it("keeps the existing two-argument invokeWithLoading call shape", async () => {
    invokeMock.mockResolvedValueOnce("ok");
    const { invokeWithLoading } = useToolInvoke();

    const result = await invokeWithLoading<string>("tool:test:legacy", { id: "1" });

    expect(result).toBe("ok");
    expect(invokeMock).toHaveBeenCalledWith("tool:test:legacy", { id: "1" });
    expect(messageErrorMock).not.toHaveBeenCalled();
  });
});
