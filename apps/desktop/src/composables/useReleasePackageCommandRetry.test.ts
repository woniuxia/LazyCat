import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("../bridge/tauri", () => ({ invokeToolByChannel: invokeMock }));

import { useReleasePackageCommandRetry } from "./useReleasePackageCommandRetry";

const prepareResult = {
  probeToken: "probe-1",
  host: "server.example",
  port: 22,
  keyType: "ed25519",
  fingerprintSha256: "SHA256:key",
  trust: "unknown" as const,
  targets: ["frontend"] as const,
  authType: "private_key" as const,
};

const trustedResult = {
  probeToken: "probe-2",
  host: "server.example",
  port: 22,
  keyType: "ed25519",
  fingerprintSha256: "SHA256:key",
  trust: "trusted" as const,
};

describe("useReleasePackageCommandRetry", () => {
  beforeEach(() => invokeMock.mockReset());

  it("prepares, trusts, preflights, and starts a command retry", async () => {
    invokeMock
      .mockResolvedValueOnce(prepareResult)
      .mockResolvedValueOnce(trustedResult)
      .mockResolvedValueOnce({
        authToken: "auth-1",
        expiresAt: "2026-07-28T12:00:00Z",
      })
      .mockResolvedValueOnce({ runId: "run-2" });
    const retry = useReleasePackageCommandRetry();

    await retry.prepare(7, "command-retry-1");
    expect(invokeMock).toHaveBeenNthCalledWith(
      1,
      "tool:release-package:command-retry-prepare",
      { projectId: 7, retryToken: "command-retry-1" },
    );

    await retry.trustHost(true);
    expect(invokeMock).toHaveBeenNthCalledWith(2, "tool:release-package:host-trust", {
      projectId: 7,
      probeToken: "probe-1",
      replaceExisting: true,
    });

    retry.privateKeyPassphrase.value = "key-passphrase";
    await retry.preflight();
    expect(invokeMock).toHaveBeenNthCalledWith(
      3,
      "tool:release-package:command-retry-preflight",
      {
        projectId: 7,
        retryToken: "command-retry-1",
        probeToken: "probe-2",
        privateKeyPassphrase: "key-passphrase",
      },
    );
    expect(retry.privateKeyPassphrase.value).toBe("");

    retry.privateKeyPassphrase.value = "must-not-survive-start";
    await expect(retry.start()).resolves.toEqual({ runId: "run-2" });
    expect(invokeMock).toHaveBeenNthCalledWith(
      4,
      "tool:release-package:command-retry-start",
      {
        projectId: 7,
        retryToken: "command-retry-1",
        authToken: "auth-1",
      },
    );
    expect(retry.privateKeyPassphrase.value).toBe("");
    expect(retry.authToken.value).toBe("");
  });

  it("clears the private key passphrase when preflight fails", async () => {
    invokeMock
      .mockResolvedValueOnce(prepareResult)
      .mockRejectedValueOnce(new Error("认证失败"));
    const retry = useReleasePackageCommandRetry();
    await retry.prepare(7, "command-retry-1");
    retry.privateKeyPassphrase.value = "wrong-passphrase";

    await expect(retry.preflight()).rejects.toThrow("认证失败");

    expect(retry.privateKeyPassphrase.value).toBe("");
    expect(retry.authToken.value).toBe("");
  });

  it("clears one-time authentication state when start fails", async () => {
    invokeMock
      .mockResolvedValueOnce(prepareResult)
      .mockResolvedValueOnce({
        authToken: "auth-1",
        expiresAt: "2026-07-28T12:00:00Z",
      })
      .mockRejectedValueOnce(new Error("启动失败"));
    const retry = useReleasePackageCommandRetry();
    await retry.prepare(7, "command-retry-1");
    retry.privateKeyPassphrase.value = "key-passphrase";
    await retry.preflight();
    retry.privateKeyPassphrase.value = "must-not-survive-start";

    await expect(retry.start()).rejects.toThrow("启动失败");

    expect(retry.privateKeyPassphrase.value).toBe("");
    expect(retry.authToken.value).toBe("");
  });

  it.each(["discard", "reset"] as const)(
    "exposes remote discard failures from %s after clearing local state",
    async (method) => {
      invokeMock
        .mockResolvedValueOnce(prepareResult)
        .mockRejectedValueOnce(new Error("撤销失败"));
      const retry = useReleasePackageCommandRetry();
      await retry.prepare(7, "command-retry-1");
      retry.authToken.value = "auth-1";
      retry.privateKeyPassphrase.value = "key-passphrase";

      await expect(retry[method]()).rejects.toThrow("撤销失败");

      expect(invokeMock).toHaveBeenNthCalledWith(2, "tool:release-package:remote-discard", {
        probeToken: "probe-1",
        preflightToken: "auth-1",
      });
      expect(retry.prepareResult.value).toBeNull();
      expect(retry.authToken.value).toBe("");
      expect(retry.privateKeyPassphrase.value).toBe("");
      expect(retry.projectId.value).toBeNull();
      expect(retry.retryToken.value).toBe("");
    },
  );

  it("resets local state without invoking the backend when no remote token exists", async () => {
    const retry = useReleasePackageCommandRetry();
    retry.privateKeyPassphrase.value = "key-passphrase";

    await retry.reset();

    expect(invokeMock).not.toHaveBeenCalled();
    expect(retry.privateKeyPassphrase.value).toBe("");
  });
});
