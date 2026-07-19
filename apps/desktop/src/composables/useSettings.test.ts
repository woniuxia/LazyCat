import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("../bridge/tauri", () => ({
  invokeToolByChannel: invokeMock,
}));

import { getSetting, setSetting, setSettingAndWait } from "./useSettings";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((innerResolve, innerReject) => {
    resolve = innerResolve;
    reject = innerReject;
  });
  return { promise, resolve, reject };
}

describe("setSettingAndWait", () => {
  beforeEach(() => {
    invokeMock.mockReset().mockResolvedValue({ ok: true });
  });

  it("updates memory immediately but waits for SQLite persistence", async () => {
    const pending = deferred<{ ok: boolean }>();
    invokeMock.mockReturnValueOnce(pending.promise);

    let settled = false;
    const result = setSettingAndWait("release_package.output_root", "D:\\releases");
    void result.then(() => {
      settled = true;
    });

    expect(getSetting("release_package.output_root")).toBe("D:\\releases");
    await Promise.resolve();
    expect(settled).toBe(false);

    pending.resolve({ ok: true });
    await result;
    expect(settled).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("tool:settings:set", {
      key: "release_package.output_root",
      value: "D:\\releases",
    });
  });

  it("restores the previous in-memory value when persistence fails", async () => {
    await setSettingAndWait("release_package.output_root", "D:\\old");
    invokeMock.mockRejectedValueOnce(new Error("write failed"));

    await expect(setSettingAndWait("release_package.output_root", "D:\\new"))
      .rejects.toThrow("write failed");
    expect(getSetting("release_package.output_root")).toBe("D:\\old");
  });

  it("removes a new in-memory value when its first persistence fails", async () => {
    invokeMock.mockRejectedValueOnce(new Error("write failed"));

    await expect(setSettingAndWait("release_package.new_key", "value"))
      .rejects.toThrow("write failed");
    expect(getSetting("release_package.new_key")).toBeUndefined();
  });

  it("keeps setSetting fire-and-forget while sharing persistence behavior", () => {
    invokeMock.mockReturnValueOnce(new Promise(() => undefined));

    expect(setSetting("release_package.output_root", "D:\\background")).toBeUndefined();
    expect(getSetting("release_package.output_root")).toBe("D:\\background");
    expect(invokeMock).toHaveBeenCalledWith("tool:settings:set", {
      key: "release_package.output_root",
      value: "D:\\background",
    });
  });

  it("serializes concurrent writes for the same key", async () => {
    const first = deferred<{ ok: boolean }>();
    const second = deferred<{ ok: boolean }>();
    invokeMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);

    const firstWrite = setSettingAndWait("release_package.serial", "first");
    const secondWrite = setSettingAndWait("release_package.serial", "second");
    await Promise.resolve();
    expect(invokeMock).toHaveBeenCalledTimes(1);

    first.resolve({ ok: true });
    await firstWrite;
    await Promise.resolve();
    expect(invokeMock).toHaveBeenCalledTimes(2);

    second.resolve({ ok: true });
    await secondWrite;
    expect(getSetting("release_package.serial")).toBe("second");
  });

  it("does not let an older failed write overwrite a newer value", async () => {
    const first = deferred<{ ok: boolean }>();
    invokeMock.mockReturnValueOnce(first.promise).mockResolvedValueOnce({ ok: true });

    const firstWrite = setSettingAndWait("release_package.stale_failure", "first");
    const secondWrite = setSettingAndWait("release_package.stale_failure", "second");
    first.reject(new Error("first failed"));

    await expect(firstWrite).rejects.toThrow("first failed");
    await secondWrite;
    expect(getSetting("release_package.stale_failure")).toBe("second");
  });

  it("restores the latest committed value when the newest write fails", async () => {
    invokeMock.mockResolvedValueOnce({ ok: true }).mockRejectedValueOnce(new Error("second failed"));

    const firstWrite = setSettingAndWait("release_package.committed", "first");
    const secondWrite = setSettingAndWait("release_package.committed", "second");
    await firstWrite;
    await expect(secondWrite).rejects.toThrow("second failed");
    expect(getSetting("release_package.committed")).toBe("first");
  });

  it("restores the original value when all concurrent writes fail", async () => {
    invokeMock
      .mockRejectedValueOnce(new Error("first failed"))
      .mockRejectedValueOnce(new Error("second failed"));

    const firstWrite = setSettingAndWait("release_package.all_failed", "first");
    const secondWrite = setSettingAndWait("release_package.all_failed", "second");
    const results = await Promise.allSettled([firstWrite, secondWrite]);

    expect(results.every((result) => result.status === "rejected")).toBe(true);
    expect(getSetting("release_package.all_failed")).toBeUndefined();
  });

  it("keeps the in-memory value when fire-and-forget persistence fails", async () => {
    invokeMock.mockRejectedValueOnce(new Error("background failed"));

    setSetting("release_package.background_failure", "value");
    await Promise.resolve();
    await Promise.resolve();
    expect(getSetting("release_package.background_failure")).toBe("value");
  });
});
