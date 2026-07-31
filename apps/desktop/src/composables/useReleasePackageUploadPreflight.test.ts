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
      })
      .mockResolvedValueOnce({ ok: true });
    const preflight = useReleasePackageUploadPreflight();

    await preflight.probe(41);
    expect(invokeMock).toHaveBeenNthCalledWith(1, "tool:release-package:remote-probe", {
      environmentId: 41,
    });
    await preflight.trustHost(41, true);
    expect(invokeMock).toHaveBeenNthCalledWith(2, "tool:release-package:host-trust", {
      environmentId: 41,
      probeToken: "probe-1",
      replaceExisting: true,
    });
    await preflight.check({
      environmentId: 41,
      targets: ["frontend"],
      privateKeyPassphrase: "key-passphrase",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "tool:release-package:remote-preflight", {
      environmentId: 41,
      targets: ["frontend"],
      probeToken: "probe-2",
      privateKeyPassphrase: "key-passphrase",
    });
    expect(JSON.stringify(invokeMock.mock.calls)).not.toContain('"password"');
    expect(preflight.preflightToken.value).toBe("preflight-1");
    expect(Object.keys(preflight)).not.toContain("password");
    expect(Object.keys(preflight)).not.toContain("privateKeyPassphrase");

    await preflight.reset();
    expect(invokeMock).toHaveBeenNthCalledWith(4, "tool:release-package:remote-discard", {
      probeToken: "probe-2",
      preflightToken: "preflight-1",
    });
    expect(preflight.probeResult.value).toBeNull();
    expect(preflight.preflightResult.value).toBeNull();
    expect(preflight.preflightToken.value).toBe("");

    await preflight.reset();
    expect(invokeMock).toHaveBeenCalledTimes(4);
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
      .mockResolvedValueOnce({ ok: true })
      .mockRejectedValueOnce(new Error("认证失败"));
    const preflight = useReleasePackageUploadPreflight();
    await preflight.probe(41);
    await preflight.check({
      environmentId: 41,
      targets: ["frontend"],
      privateKeyPassphrase: "secret",
    });

    await expect(
      preflight.check({ environmentId: 41, targets: ["frontend"], privateKeyPassphrase: "wrong" }),
    ).rejects.toThrow("认证失败");
    expect(invokeMock).toHaveBeenNthCalledWith(3, "tool:release-package:remote-discard", {
      preflightToken: "preflight-1",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(4, "tool:release-package:remote-preflight", {
      environmentId: 41,
      targets: ["frontend"],
      probeToken: "probe-1",
      privateKeyPassphrase: "wrong",
    });
    expect(preflight.preflightToken.value).toBe("");
    expect(preflight.preflightResult.value).toBeNull();
    expect(preflight.checking.value).toBe(false);
  });

  it("discards a probe token returned after reset invalidates the request", async () => {
    let resolveProbe!: (value: unknown) => void;
    invokeMock
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveProbe = resolve;
          }),
      )
      .mockResolvedValueOnce({ ok: true });
    const preflight = useReleasePackageUploadPreflight();

    const pendingProbe = preflight.probe(41);
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    await preflight.reset();
    resolveProbe({
      probeToken: "late-probe",
      host: "server.example",
      port: 22,
      keyType: "ed25519",
      fingerprintSha256: "SHA256:key",
      trust: "trusted",
    });

    await expect(pendingProbe).resolves.toBeNull();
    expect(invokeMock).toHaveBeenNthCalledWith(2, "tool:release-package:remote-discard", {
      probeToken: "late-probe",
    });
    expect(preflight.probeResult.value).toBeNull();
  });

  it("does not revive a probe after reset wins during preflight cleanup", async () => {
    let resolveDiscard!: (value: unknown) => void;
    invokeMock
      .mockResolvedValueOnce({
        probeToken: "old-probe",
        host: "server.example",
        port: 22,
        keyType: "ed25519",
        fingerprintSha256: "SHA256:key",
        trust: "trusted",
      })
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveDiscard = resolve;
          }),
      )
      .mockResolvedValueOnce({ ok: true });
    const preflight = useReleasePackageUploadPreflight();
    await preflight.probe(41);

    const pendingProbe = preflight.probe(42);
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));
    const pendingReset = preflight.reset();
    resolveDiscard({ ok: true });
    await pendingReset;

    await expect(pendingProbe).resolves.toBeNull();
    expect(invokeMock).toHaveBeenCalledTimes(2);
    expect(preflight.probeResult.value).toBeNull();
  });

  it("does not revive a check after reset wins during preflight cleanup", async () => {
    let resolveDiscard!: (value: unknown) => void;
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
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveDiscard = resolve;
          }),
      )
      .mockResolvedValueOnce({ ok: true });
    const preflight = useReleasePackageUploadPreflight();
    await preflight.probe(41);
    await preflight.check({
      environmentId: 41,
      targets: ["frontend"],
      privateKeyPassphrase: "secret",
    });

    const pendingCheck = preflight.check({
      environmentId: 41,
      targets: ["frontend"],
      privateKeyPassphrase: "secret",
    });
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(3));
    const pendingReset = preflight.reset();
    resolveDiscard({ ok: true });
    await pendingReset;

    await expect(pendingCheck).resolves.toBeNull();
    expect(invokeMock).toHaveBeenCalledTimes(4);
    expect(preflight.preflightResult.value).toBeNull();
    expect(preflight.preflightToken.value).toBe("");
  });

  it("does not let an older reset completion clear a newer probe result", async () => {
    let resolveDiscard!: (value: unknown) => void;
    invokeMock
      .mockResolvedValueOnce({
        probeToken: "old-probe",
        host: "old.example",
        port: 22,
        keyType: "ed25519",
        fingerprintSha256: "SHA256:old",
        trust: "trusted",
      })
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveDiscard = resolve;
          }),
      )
      .mockResolvedValueOnce({
        probeToken: "new-probe",
        host: "new.example",
        port: 22,
        keyType: "ed25519",
        fingerprintSha256: "SHA256:new",
        trust: "trusted",
      });
    const preflight = useReleasePackageUploadPreflight();
    await preflight.probe(41);

    const pendingReset = preflight.reset();
    await preflight.probe(42);
    resolveDiscard({ ok: true });
    await pendingReset;

    expect(preflight.probeResult.value?.probeToken).toBe("new-probe");
  });

  it("exposes discard failures without restoring local tokens or results", async () => {
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
      .mockRejectedValueOnce(new Error("撤销失败"));
    const preflight = useReleasePackageUploadPreflight();
    await preflight.probe(41);
    await preflight.check({
      environmentId: 41,
      targets: ["frontend"],
      privateKeyPassphrase: "key-passphrase",
    });

    await expect(preflight.reset()).rejects.toThrow("撤销失败");

    expect(invokeMock).toHaveBeenNthCalledWith(3, "tool:release-package:remote-discard", {
      probeToken: "probe-1",
      preflightToken: "preflight-1",
    });
    expect(preflight.probeResult.value).toBeNull();
    expect(preflight.preflightResult.value).toBeNull();
    expect(preflight.preflightToken.value).toBe("");
    expect(preflight.checking.value).toBe(false);
    expect(Object.keys(preflight)).not.toContain("privateKeyPassphrase");

    await preflight.reset();
    expect(invokeMock).toHaveBeenCalledTimes(3);
  });
});
