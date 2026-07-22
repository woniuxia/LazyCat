import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("../bridge/tauri", () => ({ invokeToolByChannel: invokeMock }));

import { useReleasePackageUploadPreflight } from "./useReleasePackageUploadPreflight";

describe("useReleasePackageUploadPreflight", () => {
  beforeEach(() => invokeMock.mockReset());

  it("probes, trusts, checks, and resets without retaining authentication secrets", async () => {
    invokeMock
      .mockResolvedValueOnce({
        probeToken: "probe-1",
        host: "server.example",
        port: 22,
        keyType: "ed25519",
        fingerprintSha256: "SHA256:key",
        trust: "unknown",
      })
      .mockResolvedValueOnce({
        probeToken: "probe-2",
        host: "server.example",
        port: 22,
        keyType: "ed25519",
        fingerprintSha256: "SHA256:key",
        trust: "trusted",
      })
      .mockResolvedValueOnce({
        preflightToken: "preflight-1",
        expiresAt: "2026-07-22T12:00:00Z",
        targets: [],
      });
    const preflight = useReleasePackageUploadPreflight();

    await preflight.probe(7);
    expect(invokeMock).toHaveBeenNthCalledWith(1, "tool:release-package:remote-probe", {
      projectId: 7,
    });
    await preflight.trustHost(true);
    expect(invokeMock).toHaveBeenNthCalledWith(2, "tool:release-package:host-trust", {
      probeToken: "probe-1",
      replaceExisting: true,
    });
    await preflight.check({ projectId: 7, targets: ["frontend"], password: "secret" });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "tool:release-package:remote-preflight", {
      projectId: 7,
      targets: ["frontend"],
      probeToken: "probe-2",
      password: "secret",
    });
    expect(preflight.preflightToken.value).toBe("preflight-1");
    expect(Object.keys(preflight)).not.toContain("password");
    expect(Object.keys(preflight)).not.toContain("privateKeyPassphrase");

    preflight.reset();
    expect(preflight.probeResult.value).toBeNull();
    expect(preflight.preflightResult.value).toBeNull();
    expect(preflight.preflightToken.value).toBe("");
  });

  it("clears an accepted token when a later check fails", async () => {
    invokeMock
      .mockResolvedValueOnce({
        probeToken: "probe-1",
        host: "server.example",
        port: 22,
        keyType: "ed25519",
        fingerprintSha256: "SHA256:key",
        trust: "trusted",
      })
      .mockResolvedValueOnce({
        preflightToken: "preflight-1",
        expiresAt: "2026-07-22T12:00:00Z",
        targets: [],
      })
      .mockRejectedValueOnce(new Error("认证失败"));
    const preflight = useReleasePackageUploadPreflight();
    await preflight.probe(7);
    await preflight.check({ projectId: 7, targets: ["frontend"], password: "secret" });

    await expect(
      preflight.check({ projectId: 7, targets: ["frontend"], password: "wrong" }),
    ).rejects.toThrow("认证失败");
    expect(preflight.preflightToken.value).toBe("");
    expect(preflight.preflightResult.value).toBeNull();
    expect(preflight.checking.value).toBe(false);
  });
});
