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

export const VAULT_HARD_LOCK_MINUTES = [5, 10, 15, 30, 60] as const;
export const VAULT_SENSITIVE_HIDE_MINUTES = [1, 2, 5] as const;

export type VaultHardLockMinutes = typeof VAULT_HARD_LOCK_MINUTES[number];
export type VaultSensitiveHideMinutes = typeof VAULT_SENSITIVE_HIDE_MINUTES[number];

export interface VaultLockSettings {
  sensitiveHideMinutes: VaultSensitiveHideMinutes;
  activityLockEnabled: boolean;
  activityLockMinutes: VaultHardLockMinutes;
  systemIdleLockEnabled: boolean;
  systemIdleLockMinutes: VaultHardLockMinutes;
}

export const VAULT_LOCK_SETTING_KEYS = {
  sensitiveHideMinutes: "vault_sensitive_hide_minutes",
  activityLockEnabled: "vault_activity_lock_enabled",
  activityLockMinutes: "vault_activity_lock_minutes",
  systemIdleLockEnabled: "vault_system_idle_lock_enabled",
  systemIdleLockMinutes: "vault_system_idle_lock_minutes",
} as const;

const LEGACY_LOCK_DEFAULTS: Record<
  VaultLockProfile,
  Pick<VaultLockSettings, "sensitiveHideMinutes" | "activityLockMinutes">
> = {
  strict: { sensitiveHideMinutes: 1, activityLockMinutes: 10 },
  balanced: { sensitiveHideMinutes: 2, activityLockMinutes: 30 },
  convenient: { sensitiveHideMinutes: 5, activityLockMinutes: 60 },
};

function parseBoolean(raw: string | undefined, fallback: boolean): boolean {
  if (raw === "true") return true;
  if (raw === "false") return false;
  return fallback;
}

function parseAllowed<T extends readonly number[]>(
  raw: string | undefined,
  allowed: T,
  fallback: T[number],
): T[number] {
  const value = Number(raw);
  return allowed.includes(value as T[number]) ? value as T[number] : fallback;
}

export function resolveVaultLockSettings(
  raw: Record<string, string | undefined>,
): VaultLockSettings {
  const legacy = LEGACY_LOCK_DEFAULTS[normalizeVaultLockProfile(raw.vault_lock_profile)];
  return {
    sensitiveHideMinutes: parseAllowed(
      raw.vault_sensitive_hide_minutes,
      VAULT_SENSITIVE_HIDE_MINUTES,
      legacy.sensitiveHideMinutes,
    ),
    activityLockEnabled: parseBoolean(raw.vault_activity_lock_enabled, true),
    activityLockMinutes: parseAllowed(
      raw.vault_activity_lock_minutes,
      VAULT_HARD_LOCK_MINUTES,
      legacy.activityLockMinutes,
    ),
    systemIdleLockEnabled: parseBoolean(raw.vault_system_idle_lock_enabled, true),
    systemIdleLockMinutes: parseAllowed(
      raw.vault_system_idle_lock_minutes,
      VAULT_HARD_LOCK_MINUTES,
      15,
    ),
  };
}

export function toVaultLockRuntimePolicy(settings: VaultLockSettings) {
  return {
    hideSensitiveAfterSecs: settings.sensitiveHideMinutes * 60,
    activityLockEnabled: settings.activityLockEnabled,
    activityLockAfterSecs: settings.activityLockMinutes * 60,
  };
}

export function summarizeVaultHardLockRules(settings: VaultLockSettings): string {
  if (settings.activityLockEnabled && settings.systemIdleLockEnabled) {
    return "任一条件达到后即锁定";
  }
  if (settings.activityLockEnabled) {
    return `Vault 无活动 ${settings.activityLockMinutes} 分钟后锁定`;
  }
  if (settings.systemIdleLockEnabled) {
    return `电脑无操作 ${settings.systemIdleLockMinutes} 分钟后锁定`;
  }
  return "仅手动或关闭到托盘时锁定";
}
