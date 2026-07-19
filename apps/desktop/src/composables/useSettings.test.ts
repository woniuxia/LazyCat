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
});
