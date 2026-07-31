import { describe, expect, it } from "vitest";
import {
  getVaultLockPolicy,
  normalizeVaultLockProfile,
  resolveVaultLockSettings,
  summarizeVaultHardLockRules,
  toVaultLockRuntimePolicy,
} from "./vaultLock";

describe("vaultLock", () => {
  it("falls back to balanced profile", () => {
    expect(normalizeVaultLockProfile(undefined)).toBe("balanced");
    expect(normalizeVaultLockProfile("unexpected")).toBe("balanced");
  });

  it("returns the expected balanced policy", () => {
    const policy = getVaultLockPolicy("balanced");

    expect(policy.profile).toBe("balanced");
    expect(policy.hideSensitiveMs).toBe(120_000);
    expect(policy.hardLockMs).toBe(1_800_000);
  });

  it("supports strict and convenient policies", () => {
    expect(getVaultLockPolicy("strict").hideSensitiveMs).toBe(60_000);
    expect(getVaultLockPolicy("strict").hardLockMs).toBe(600_000);
    expect(getVaultLockPolicy("convenient").hideSensitiveMs).toBe(300_000);
    expect(getVaultLockPolicy("convenient").hardLockMs).toBe(3_600_000);
  });

  it("migrates balanced and enables system idle lock by default", () => {
    expect(resolveVaultLockSettings({ vault_lock_profile: "balanced" })).toEqual({
      sensitiveHideMinutes: 2,
      activityLockEnabled: true,
      activityLockMinutes: 30,
      systemIdleLockEnabled: true,
      systemIdleLockMinutes: 15,
    });
  });

  it("prefers explicit settings and normalizes illegal values", () => {
    expect(
      resolveVaultLockSettings({
        vault_lock_profile: "strict",
        vault_sensitive_hide_minutes: "5",
        vault_activity_lock_enabled: "false",
        vault_activity_lock_minutes: "60",
        vault_system_idle_lock_enabled: "true",
        vault_system_idle_lock_minutes: "30",
      }),
    ).toMatchObject({
      sensitiveHideMinutes: 5,
      activityLockEnabled: false,
      activityLockMinutes: 60,
      systemIdleLockMinutes: 30,
    });
    expect(
      resolveVaultLockSettings({
        vault_activity_lock_minutes: "999",
        vault_system_idle_lock_enabled: "invalid",
      }),
    ).toMatchObject({
      activityLockMinutes: 30,
      systemIdleLockEnabled: true,
    });
  });

  it("builds runtime policy and OR summaries", () => {
    const settings = resolveVaultLockSettings({});
    expect(toVaultLockRuntimePolicy(settings)).toEqual({
      hideSensitiveAfterSecs: 120,
      activityLockEnabled: true,
      activityLockAfterSecs: 1_800,
    });
    expect(summarizeVaultHardLockRules(settings)).toBe("任一条件达到后即锁定");
    expect(
      summarizeVaultHardLockRules({
        ...settings,
        activityLockEnabled: false,
        systemIdleLockEnabled: false,
      }),
    ).toBe("仅手动或关闭到托盘时锁定");
  });

  it("keeps masking while activity hard lock is disabled", () => {
    const settings = resolveVaultLockSettings({
      vault_activity_lock_enabled: "false",
    });
    expect(toVaultLockRuntimePolicy(settings)).toEqual({
      hideSensitiveAfterSecs: 120,
      activityLockEnabled: false,
      activityLockAfterSecs: 1_800,
    });
  });
});
