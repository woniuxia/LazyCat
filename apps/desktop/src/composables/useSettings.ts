import { reactive, ref } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";

/** All known localStorage keys used by old versions */
const LEGACY_KEYS = [
  "lazycat:theme:v1",
  "lazycat:hotkey:v1",
  "lazycat:favorites:v1",
  "lazycat:tool-clicks:v1",
  "lazycat:home-top-limit:v1",
  "lazycat:calc-draft-history:v1",
] as const;

/** Maps old localStorage keys to new SQLite setting keys */
const LEGACY_KEY_MAP: Record<string, string> = {
  "lazycat:theme:v1": "theme",
  "lazycat:hotkey:v1": "hotkey",
  "lazycat:favorites:v1": "favorites",
  "lazycat:tool-clicks:v1": "tool_clicks",
  "lazycat:home-top-limit:v1": "home_top_limit",
  "lazycat:calc-draft-history:v1": "calc_draft_history",
};

const settings = reactive<Record<string, string>>({});
const loaded = ref(false);
const loadPromise = ref<Promise<void> | null>(null);

async function loadAll(): Promise<void> {
  try {
    const data = (await invokeToolByChannel("tool:settings:get-all", {})) as
      | Record<string, string>
      | null;
    if (data) {
      Object.assign(settings, data);
    }
  } catch {
    // IPC unavailable (non-Tauri env): fall through, localStorage will be used
  }
}

async function migrateFromLocalStorage(): Promise<void> {
  let migrated = false;
  for (const legacyKey of LEGACY_KEYS) {
    const raw = localStorage.getItem(legacyKey);
    if (raw === null) continue;
    const settingKey = LEGACY_KEY_MAP[legacyKey];
    if (!settingKey) continue;
    // Only migrate if SQLite doesn't already have this key
    if (settings[settingKey] !== undefined) continue;
    settings[settingKey] = raw;
    try {
      await invokeToolByChannel("tool:settings:set", {
        key: settingKey,
        value: raw,
      });
      migrated = true;
    } catch {
      // IPC unavailable, keep in-memory only
    }
  }
  // Clean up old localStorage keys after successful migration
  if (migrated) {
    for (const legacyKey of LEGACY_KEYS) {
      localStorage.removeItem(legacyKey);
    }
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
    // If SQLite is empty, check for legacy localStorage data
    if (Object.keys(settings).length === 0) {
      await migrateFromLocalStorage();
    }
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
  settings[key] = value;
  // Async persist to SQLite, fire-and-forget
  invokeToolByChannel("tool:settings:set", { key, value }).catch(() => {
    // IPC failed, data remains in memory for current session
  });
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
