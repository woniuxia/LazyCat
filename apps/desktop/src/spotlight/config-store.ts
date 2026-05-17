import { invokeToolByChannel } from "../bridge/tauri";
import { listDescriptors } from "./registry";
import { QUICK_COMMAND_DESCRIPTORS } from "./quick-commands";
import type {
  ProviderDescriptor,
  QuickCommandDescriptor,
  QuickCommandId,
  ResolvedProvider,
  SpotlightConfig,
  SpotlightConfigProviderOverride,
  SpotlightConfigQuickCommandOverride,
  SpotlightProviderId,
  SpotlightView,
} from "./types";

const STORAGE_KEY = "spotlight_config_v1";
const BACKUP_KEY = "spotlight_config_v1.backup";
const CHANGE_EVENT = "spotlight-config-changed";

export const RESERVED_TOKENS = new Set(["+", "calc"]);
export const ALIAS_PATTERN = /^[a-zA-Z0-9_-]{1,16}$/;
const MAX_ALIAS_LENGTH = 16;

/* ───────────────────────── 纯函数(可测试) ───────────────────────── */

export function buildDefaultConfig(): SpotlightConfig {
  return { version: 1, providers: {}, quickCommands: {} };
}

export function normalizeAliases(input: string[] | undefined): string[] {
  if (!Array.isArray(input)) return [];
  const seen = new Set<string>();
  const result: string[] = [];
  for (const raw of input) {
    if (typeof raw !== "string") continue;
    const v = raw.trim().toLowerCase();
    if (!v || seen.has(v)) continue;
    seen.add(v);
    result.push(v);
  }
  return result;
}

export interface AliasConflict {
  alias: string;
  reason: string;
}

export interface AliasValidationResult {
  ok: boolean;
  conflicts: AliasConflict[];
  normalized: string[];
}

export function validateAliasesPure(
  input: string[],
  exceptId: SpotlightProviderId,
  descriptors: ProviderDescriptor[],
  config: SpotlightConfig,
): AliasValidationResult {
  const normalized = normalizeAliases(input);
  const conflicts: AliasConflict[] = [];

  // 收集其它 provider 当前使用中的 alias
  const others = new Map<string, string>(); // alias -> providerName
  for (const d of descriptors) {
    if (d.id === exceptId) continue;
    const override = config.providers[d.id];
    const aliases = override?.aliases
      ? normalizeAliases(override.aliases)
      : d.defaultAliases.map((a) => a.toLowerCase());
    for (const a of aliases) {
      if (!others.has(a)) others.set(a, d.name);
    }
  }

  for (const alias of normalized) {
    if (RESERVED_TOKENS.has(alias)) {
      conflicts.push({ alias, reason: `"${alias}" 是保留 token` });
      continue;
    }
    if (!ALIAS_PATTERN.test(alias)) {
      conflicts.push({
        alias,
        reason: `仅允许 1-${MAX_ALIAS_LENGTH} 位字母数字与 _ -`,
      });
      continue;
    }
    const owner = others.get(alias);
    if (owner) {
      conflicts.push({ alias, reason: `已被「${owner}」占用` });
    }
  }

  return { ok: conflicts.length === 0, conflicts, normalized };
}

export function mergeView(
  descriptors: ProviderDescriptor[],
  quickCommands: QuickCommandDescriptor[],
  config: SpotlightConfig,
): SpotlightView {
  const providers: ResolvedProvider[] = descriptors.map((d) => {
    const override = config.providers[d.id] ?? {};
    const enabled = override.enabled ?? d.defaultEnabled;
    const aliases = override.aliases
      ? normalizeAliases(override.aliases)
      : d.defaultAliases.map((a) => a.toLowerCase());
    return Object.assign({}, d, { enabled, aliases });
  });

  const aliasMap = new Map<string, SpotlightProviderId>();
  for (const p of providers) {
    if (!p.enabled) continue;
    for (const a of p.aliases) {
      if (!aliasMap.has(a)) aliasMap.set(a, p.id);
    }
  }

  const enabledQuickCommands = new Set<QuickCommandId>();
  for (const qc of quickCommands) {
    const override = config.quickCommands[qc.id] ?? {};
    if (override.enabled ?? qc.defaultEnabled) enabledQuickCommands.add(qc.id);
  }

  return { providers, aliasMap, enabledQuickCommands, quickCommands };
}

export function sanitizeConfig(
  raw: unknown,
  descriptors: ProviderDescriptor[],
  quickCommands: QuickCommandDescriptor[],
): SpotlightConfig | null {
  if (!raw || typeof raw !== "object") return null;
  const obj = raw as Record<string, unknown>;
  if (obj.version !== 1) return null;

  const providerIds = new Set(descriptors.map((d) => d.id));
  const quickIds = new Set(quickCommands.map((q) => q.id));

  const providers: SpotlightConfig["providers"] = {};
  const rawProviders =
    obj.providers && typeof obj.providers === "object"
      ? (obj.providers as Record<string, unknown>)
      : {};
  for (const [k, v] of Object.entries(rawProviders)) {
    if (!providerIds.has(k as SpotlightProviderId)) continue;
    if (!v || typeof v !== "object") continue;
    const o = v as Record<string, unknown>;
    const override: SpotlightConfigProviderOverride = {};
    if (typeof o.enabled === "boolean") override.enabled = o.enabled;
    if (Array.isArray(o.aliases)) override.aliases = normalizeAliases(o.aliases as string[]);
    providers[k as SpotlightProviderId] = override;
  }

  const qc: SpotlightConfig["quickCommands"] = {};
  const rawQc =
    obj.quickCommands && typeof obj.quickCommands === "object"
      ? (obj.quickCommands as Record<string, unknown>)
      : {};
  for (const [k, v] of Object.entries(rawQc)) {
    if (!quickIds.has(k as QuickCommandId)) continue;
    if (!v || typeof v !== "object") continue;
    const o = v as Record<string, unknown>;
    const override: SpotlightConfigQuickCommandOverride = {};
    if (typeof o.enabled === "boolean") override.enabled = o.enabled;
    qc[k as QuickCommandId] = override;
  }

  return { version: 1, providers, quickCommands: qc };
}

/* ───────────────────────── 持久化(可注入,便于测试) ───────────────────────── */

export interface ConfigPersistence {
  read: (key: string) => Promise<string | null>;
  write: (key: string, value: string) => Promise<void>;
}

const defaultPersistence: ConfigPersistence = {
  async read(key) {
    try {
      const v = (await invokeToolByChannel("tool:settings:get", { key })) as
        | { value?: string | null }
        | string
        | null;
      if (v == null) return null;
      if (typeof v === "string") return v;
      return v.value ?? null;
    } catch {
      return null;
    }
  },
  async write(key, value) {
    await invokeToolByChannel("tool:settings:set", { key, value });
  },
};

/* ───────────────────────── 模块单例 ───────────────────────── */

let persistence: ConfigPersistence = defaultPersistence;
let cachedConfig: SpotlightConfig = buildDefaultConfig();
let currentView: SpotlightView | null = null;
let lastLoadError: string | null = null;
let loadPromise: Promise<SpotlightView> | null = null;
let inFlightSave: Promise<void> | null = null;
const subscribers = new Set<(view: SpotlightView) => void>();

export function __setPersistenceForTests(p: ConfigPersistence): void {
  persistence = p;
}

export function __resetForTests(): void {
  persistence = defaultPersistence;
  cachedConfig = buildDefaultConfig();
  currentView = null;
  lastLoadError = null;
  loadPromise = null;
  inFlightSave = null;
  subscribers.clear();
  stopListening();
}

function descriptors(): ProviderDescriptor[] {
  return listDescriptors(true);
}

function recomputeView(): SpotlightView {
  currentView = mergeView(descriptors(), QUICK_COMMAND_DESCRIPTORS, cachedConfig);
  return currentView;
}

async function doLoad(): Promise<SpotlightView> {
  lastLoadError = null;
  let raw: string | null = null;
  try {
    raw = await persistence.read(STORAGE_KEY);
  } catch (err) {
    lastLoadError = err instanceof Error ? err.message : String(err);
  }

  if (raw != null) {
    try {
      const parsed = JSON.parse(raw);
      const sanitized = sanitizeConfig(parsed, descriptors(), QUICK_COMMAND_DESCRIPTORS);
      if (sanitized) {
        cachedConfig = sanitized;
      } else {
        lastLoadError = "配置版本不兼容";
        await backupRaw(raw);
        cachedConfig = buildDefaultConfig();
      }
    } catch (err) {
      lastLoadError = err instanceof Error ? err.message : String(err);
      await backupRaw(raw);
      cachedConfig = buildDefaultConfig();
    }
  } else {
    cachedConfig = buildDefaultConfig();
  }

  return recomputeView();
}

async function backupRaw(raw: string): Promise<void> {
  try {
    await persistence.write(BACKUP_KEY, raw);
  } catch {
    /* 备份失败不阻塞主流程 */
  }
}

export async function ensureLoaded(forceReload = false): Promise<SpotlightView> {
  if (!forceReload && currentView) return currentView;
  if (!forceReload && loadPromise) return loadPromise;
  loadPromise = doLoad().finally(() => {
    loadPromise = null;
  });
  return loadPromise;
}

export function getView(): SpotlightView {
  if (!currentView) return recomputeView();
  return currentView;
}

export function getConfig(): SpotlightConfig {
  return cachedConfig;
}

export function getLastLoadError(): string | null {
  return lastLoadError;
}

export function validateAliases(
  input: string[],
  exceptId: SpotlightProviderId,
): AliasValidationResult {
  return validateAliasesPure(input, exceptId, descriptors(), cachedConfig);
}

export async function saveConfig(next: SpotlightConfig): Promise<void> {
  if (inFlightSave) {
    await inFlightSave;
  }
  const sanitized =
    sanitizeConfig(next, descriptors(), QUICK_COMMAND_DESCRIPTORS) ?? buildDefaultConfig();
  const prev = cachedConfig;
  cachedConfig = sanitized;
  recomputeView();
  for (const cb of subscribers) {
    try {
      cb(currentView!);
    } catch {
      /* 订阅者异常隔离 */
    }
  }
  const task = (async () => {
    try {
      await persistence.write(STORAGE_KEY, JSON.stringify(sanitized));
      // 写盘成功后跨窗口广播;主窗口与 Spotlight 窗口都能监听
      try {
        const { emit } = await import("@tauri-apps/api/event");
        await emit(CHANGE_EVENT, { version: 1 });
      } catch {
        /* 广播失败不阻塞,下一次呼出 ensureLoaded(true) 兜底 */
      }
    } catch (err) {
      cachedConfig = prev;
      recomputeView();
      throw err;
    }
  })();
  // inFlightSave 仅用于串行化(下一次 saveConfig 等待它结束),
  // 这里 swallow 拒绝避免产生未处理的 promise rejection;
  // 真正的错误仍会通过 await task 抛给调用方。
  const guard = task.catch(() => undefined);
  inFlightSave = guard;
  guard.finally(() => {
    if (inFlightSave === guard) inFlightSave = null;
  });
  await task;
}

export function subscribe(cb: (view: SpotlightView) => void): () => void {
  subscribers.add(cb);
  return () => subscribers.delete(cb);
}

/* ───────────────────────── 跨窗口广播监听 ───────────────────────── */

let unlistenChange: (() => void) | null = null;
let listenStarted = false;

export async function startListening(): Promise<void> {
  if (listenStarted) return;
  listenStarted = true;
  try {
    const { listen } = await import("@tauri-apps/api/event");
    unlistenChange = await listen(CHANGE_EVENT, async () => {
      await ensureLoaded(true);
      for (const cb of subscribers) {
        try {
          cb(currentView!);
        } catch {
          /* 订阅者异常隔离 */
        }
      }
    });
  } catch {
    // 非 Tauri 环境(单测)
    listenStarted = false;
  }
}

export function stopListening(): void {
  unlistenChange?.();
  unlistenChange = null;
  listenStarted = false;
}
