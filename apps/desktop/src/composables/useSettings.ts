import { reactive, ref } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";
import {
  getVaultLockPolicy,
  normalizeVaultLockProfile,
  resolveVaultLockSettings,
  VAULT_LOCK_SETTING_KEYS,
  type VaultLockPolicy,
  type VaultLockProfile,
  type VaultLockSettings,
} from "../utils/vaultLock";

export interface VaultLockProfilePolicy {
  hideSensitiveAfterSecs: number;
  hardLockAfterSecs: number;
}

export const DEFAULT_VAULT_LOCK_PROFILE: VaultLockProfile = "balanced";

const settings = reactive<Record<string, string>>({});
const loaded = ref(false);
const loadPromise = ref<Promise<void> | null>(null);
const settingPersistenceQueues = new Map<string, Promise<void>>();
const committedSettings = new Map<string, string | undefined>();
const settingVersions = new Map<string, number>();
type VaultLockSettingsListener = (value: VaultLockSettings) => void;
const vaultLockSettingsListeners = new Set<VaultLockSettingsListener>();

// 新增：开机自启动相关状态
const autostartEnabled = ref(false);
const autostartMinimized = ref(false);
const closeToTray = ref(true); // 默认开启，保持当前行为

async function loadAll(): Promise<void> {
  try {
    const data = (await invokeToolByChannel("tool:settings:get-all", {})) as Record<
      string,
      string
    > | null;
    if (data) {
      Object.assign(settings, data);
    }
  } catch {
    // IPC unavailable (non-Tauri env): keep in-memory defaults
  }
}

/**
 * Initialize the settings layer. Must be called once at app startup.
 * Returns a promise that resolves when settings are loaded.
 */
export function initSettings(): Promise<void> {
  if (loadPromise.value) return loadPromise.value;
  loadPromise.value = (async () => {
    await loadAll();
    settings.vault_lock_profile = normalizeVaultLockProfile(settings.vault_lock_profile);
    // 加载开机自启动相关设置
    autostartMinimized.value = settings.autostart_minimized === "true";
    closeToTray.value = settings.close_to_tray !== "false"; // 默认 true
    await checkAutostartStatus(); // 查询系统实际状态
    loaded.value = true;
  })();
  return loadPromise.value;
}

/**
 * Get a setting value. Returns undefined if not found.
 */
export function getSetting(key: string): string | undefined {
  return settings[key];
}

export function getVaultLockProfile(): VaultLockProfile {
  return normalizeVaultLockProfile(settings.vault_lock_profile);
}

export function getVaultLockProfilePolicy(
  profile: VaultLockProfile = getVaultLockProfile(),
): VaultLockProfilePolicy {
  const policy: VaultLockPolicy = getVaultLockPolicy(profile);
  return {
    hideSensitiveAfterSecs: Math.round(policy.hideSensitiveMs / 1000),
    hardLockAfterSecs: Math.round(policy.hardLockMs / 1000),
  };
}

/**
 * Get a setting parsed as JSON with a fallback.
 */
export function getSettingJson<T>(key: string, fallback: T): T {
  const raw = settings[key];
  if (raw === undefined) return fallback;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

/**
 * Set a setting value. Writes to in-memory store immediately,
 * then persists to SQLite asynchronously.
 */
export function setSetting(key: string, value: string): void {
  void persistSetting(key, value, false).catch(() => {
    // Preserve the existing fire-and-forget API and in-memory value on failure.
  });
}

export async function setSettingAndWait(key: string, value: string): Promise<void> {
  await persistSetting(key, value, true);
}

function persistSetting(key: string, value: string, rollbackOnFailure: boolean): Promise<void> {
  if (!committedSettings.has(key)) {
    committedSettings.set(key, settings[key]);
  }
  const version = (settingVersions.get(key) ?? 0) + 1;
  settingVersions.set(key, version);
  settings[key] = value;

  const persist = async (): Promise<void> => {
    try {
      await invokeToolByChannel("tool:settings:set", { key, value });
      committedSettings.set(key, value);
    } catch (error) {
      if (rollbackOnFailure && settingVersions.get(key) === version) {
        const committed = committedSettings.get(key);
        if (committed === undefined) delete settings[key];
        else settings[key] = committed;
      }
      throw error;
    }
  };
  const previous = settingPersistenceQueues.get(key);
  const operation = previous ? previous.catch(() => undefined).then(persist) : persist();

  settingPersistenceQueues.set(key, operation);
  void operation.then(
    () => clearSettingPersistenceQueue(key, operation),
    () => clearSettingPersistenceQueue(key, operation),
  );
  return operation;
}

function clearSettingPersistenceQueue(key: string, operation: Promise<void>): void {
  if (settingPersistenceQueues.get(key) === operation) {
    settingPersistenceQueues.delete(key);
  }
}

export function setVaultLockProfile(profile: VaultLockProfile): void {
  const normalizedProfile = normalizeVaultLockProfile(profile);
  setSetting("vault_lock_profile", normalizedProfile);
}

export function getVaultLockSettings(): VaultLockSettings {
  return resolveVaultLockSettings(settings);
}

function getCommittedVaultLockSettings(): VaultLockSettings {
  const committed = { ...settings };
  for (const key of Object.values(VAULT_LOCK_SETTING_KEYS)) {
    if (!committedSettings.has(key)) continue;
    const value = committedSettings.get(key);
    if (value === undefined) delete committed[key];
    else committed[key] = value;
  }
  return resolveVaultLockSettings(committed);
}

export function subscribeVaultLockSettings(listener: VaultLockSettingsListener): () => void {
  vaultLockSettingsListeners.add(listener);
  return () => vaultLockSettingsListeners.delete(listener);
}

export async function setVaultLockSettingAndWait<K extends keyof VaultLockSettings>(
  name: K,
  value: VaultLockSettings[K],
): Promise<void> {
  await setSettingAndWait(VAULT_LOCK_SETTING_KEYS[name], String(value));
  const current = getCommittedVaultLockSettings();
  for (const listener of vaultLockSettingsListeners) {
    listener(current);
  }
}

/**
 * Set a setting with a JSON-serializable value.
 */
export function setSettingJson(key: string, value: unknown): void {
  setSetting(key, JSON.stringify(value));
}

/**
 * Whether settings have been loaded from SQLite.
 */
export function isSettingsLoaded(): boolean {
  return loaded.value;
}

/**
 * 启用开机自启动
 */
export async function enableAutostart(): Promise<void> {
  await invokeToolByChannel("tool:settings:enable-autostart", {});
  autostartEnabled.value = true;
}

/**
 * 禁用开机自启动
 */
export async function disableAutostart(): Promise<void> {
  await invokeToolByChannel("tool:settings:disable-autostart", {});
  autostartEnabled.value = false;
}

/**
 * 查询开机自启动状态
 */
export async function checkAutostartStatus(): Promise<void> {
  try {
    const result = (await invokeToolByChannel("tool:settings:is-autostart-enabled", {})) as {
      enabled: boolean;
    };
    autostartEnabled.value = result.enabled;
  } catch {
    // IPC 失败，保持当前状态
  }
}

/**
 * 设置启动时最小化
 */
export async function setAutostartMinimized(value: boolean): Promise<void> {
  await setSetting("autostart_minimized", value.toString());
  autostartMinimized.value = value;
}

/**
 * 设置关闭时最小化到托盘
 */
export async function setCloseToTray(value: boolean): Promise<void> {
  await setSetting("close_to_tray", value.toString());
  closeToTray.value = value;
}

/**
 * 导出开机自启动相关状态
 */
export function useAutostartSettings() {
  return {
    autostartEnabled,
    autostartMinimized,
    closeToTray,
    enableAutostart,
    disableAutostart,
    checkAutostartStatus,
    setAutostartMinimized,
    setCloseToTray,
  };
}
