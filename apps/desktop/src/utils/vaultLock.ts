export type VaultLockProfile = "strict" | "balanced" | "convenient";

export type VaultLockState = "unlocked" | "locked";

export interface VaultLockPolicy {
  profile: VaultLockProfile;
  label: string;
  description: string;
  hideSensitiveMs: number;
  hardLockMs: number;
}

const LOCK_POLICIES: Record<VaultLockProfile, VaultLockPolicy> = {
  strict: {
    profile: "strict",
    label: "严格",
    description: "更快隐藏敏感信息，并缩短自动硬锁等待时间。",
    hideSensitiveMs: 60_000,
    hardLockMs: 600_000,
  },
  balanced: {
    profile: "balanced",
    label: "平衡",
    description: "默认推荐，兼顾日常使用流畅度与离开设备后的安全性。",
    hideSensitiveMs: 120_000,
    hardLockMs: 1_800_000,
  },
  convenient: {
    profile: "convenient",
    label: "便捷",
    description: "减少频繁打断，但仍保留自动保护。",
    hideSensitiveMs: 300_000,
    hardLockMs: 3_600_000,
  },
};

export function normalizeVaultLockProfile(value?: string | null): VaultLockProfile {
  if (value === "strict" || value === "balanced" || value === "convenient") {
    return value;
  }
  return "balanced";
}

export function getVaultLockPolicy(value?: string | null): VaultLockPolicy {
  return LOCK_POLICIES[normalizeVaultLockProfile(value)];
}
