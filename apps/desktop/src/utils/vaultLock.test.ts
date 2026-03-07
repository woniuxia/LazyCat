import { describe, expect, it } from "vitest";
import { getVaultLockPolicy, normalizeVaultLockProfile } from "./vaultLock";

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
});
