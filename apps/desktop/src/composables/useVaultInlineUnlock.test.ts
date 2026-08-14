import { nextTick } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useVaultInlineUnlock } from "./useVaultInlineUnlock";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("../bridge/tauri", () => ({ invokeToolByChannel: invokeMock }));

describe("useVaultInlineUnlock", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("requests an explicit unlock only when the application Vault session is locked", async () => {
    invokeMock.mockResolvedValueOnce({ setup: true, unlocked: false });
    const unlock = useVaultInlineUnlock();
    const resume = vi.fn(async () => undefined);

    await expect(unlock.requireUnlocked("生产服务器", resume)).resolves.toBe(false);

    expect(invokeMock).toHaveBeenCalledWith("tool:vault:status", {});
    expect(unlock.visible.value).toBe(true);
    expect(unlock.credentialLabel.value).toBe("生产服务器");
    expect(resume).not.toHaveBeenCalled();
  });

  it("clears a wrong password, reports it inline, and allows a corrected retry", async () => {
    invokeMock
      .mockRejectedValueOnce(new Error("wrong_password"))
      .mockResolvedValueOnce({ unlocked: true });
    const unlock = useVaultInlineUnlock();
    const resume = vi.fn(async () => undefined);
    unlock.request("生产服务器", resume);
    unlock.masterPassword.value = "wrong";

    await unlock.submit();

    expect(unlock.masterPassword.value).toBe("");
    expect(unlock.error.value).toBe("主密码错误，请重试");
    expect(unlock.visible.value).toBe(true);
    const focusAfterFailure = unlock.focusNonce.value;

    unlock.masterPassword.value = "correct";
    await unlock.submit();

    expect(invokeMock).toHaveBeenNthCalledWith(2, "tool:vault:unlock", {
      masterPassword: "correct",
    });
    expect(unlock.masterPassword.value).toBe("");
    expect(unlock.visible.value).toBe(false);
    expect(unlock.focusNonce.value).toBeGreaterThanOrEqual(focusAfterFailure);
    expect(resume).toHaveBeenCalledTimes(1);
  });

  it("prevents duplicate calls without letting an old response clear a new dialog secret", async () => {
    let resolveUnlock!: () => void;
    invokeMock.mockImplementationOnce(
      () => new Promise<void>((resolve) => (resolveUnlock = resolve)),
    );
    const unlock = useVaultInlineUnlock();
    const resume = vi.fn(async () => undefined);
    unlock.request("生产服务器", resume);
    unlock.masterPassword.value = "secret";

    const first = unlock.submit();
    const second = unlock.submit();
    expect(invokeMock).toHaveBeenCalledTimes(1);

    unlock.reset();
    unlock.request("备用服务器", async () => undefined);
    unlock.masterPassword.value = "new-secret";
    resolveUnlock();
    await Promise.all([first, second]);
    await nextTick();

    expect(unlock.masterPassword.value).toBe("new-secret");
    expect(unlock.visible.value).toBe(true);
    expect(resume).not.toHaveBeenCalled();
  });

  it("exposes setup and infrastructure failures instead of treating them as bad passwords", async () => {
    invokeMock.mockResolvedValueOnce({ setup: false, unlocked: false });
    const unlock = useVaultInlineUnlock();
    await expect(unlock.requireUnlocked("凭据", async () => undefined)).rejects.toThrow(
      "vault_not_initialized",
    );

    invokeMock.mockRejectedValueOnce(new Error("vault session unavailable"));
    unlock.request("凭据", async () => undefined);
    unlock.masterPassword.value = "secret";
    await unlock.submit();
    expect(unlock.error.value).toBe("vault session unavailable");
  });
});
