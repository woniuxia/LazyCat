import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// 避免实际调用 Tauri 通道
vi.mock("../bridge/tauri", () => ({
  invokeToolByChannel: vi.fn(async () => null),
}));

import {
  __resetForTests,
  __setPersistenceForTests,
  buildDefaultConfig,
  ensureLoaded,
  getConfig,
  getLastLoadError,
  getView,
  mergeView,
  normalizeAliases,
  saveConfig,
  sanitizeConfig,
  validateAliases,
  validateAliasesPure,
} from "./config-store";
import { QUICK_COMMAND_DESCRIPTORS } from "./quick-commands";
// 触发其它 provider 注册,使 registry.listDescriptors() 返回完整集合
import "./providers/vault";
import "./providers/hosts";
import "./providers/todo";
import "./providers/pm";
import "./providers/suggestion";
import "./providers/launcher";
import type { ProviderDescriptor, SpotlightConfig } from "./types";

function makeDescriptor(over: Partial<ProviderDescriptor> & { id: ProviderDescriptor["id"] }): ProviderDescriptor {
  return {
    id: over.id,
    name: over.name ?? String(over.id),
    description: over.description ?? "",
    badgeShort: over.badgeShort ?? "X",
    badgeTone: over.badgeTone ?? "primary",
    weight: over.weight ?? 1,
    defaultAliases: over.defaultAliases ?? [],
    defaultEnabled: over.defaultEnabled ?? true,
    hiddenInSettings: over.hiddenInSettings,
    quickCommands: over.quickCommands,
    prefetch: over.prefetch ?? (async () => []),
    defaultAction: over.defaultAction ?? (async () => ({})),
    buildActions: over.buildActions,
    executeAction: over.executeAction,
  };
}

function registerTestProviders() {
  // 通过顶部 import 触发 provider 模块注册;此处占位以表意
}

beforeEach(() => {
  __resetForTests();
  registerTestProviders();
});

afterEach(() => {
  __resetForTests();
});

describe("normalizeAliases", () => {
  it("trims, lowercases, dedupes, drops empty", () => {
    expect(normalizeAliases(["T", "  Todo ", "T", "", " "])).toEqual(["t", "todo"]);
  });

  it("returns empty array for non-array input", () => {
    expect(normalizeAliases(undefined)).toEqual([]);
  });
});

describe("mergeView", () => {
  const todo = makeDescriptor({ id: "todo", name: "任务", defaultAliases: ["t", "todo"] });
  const vault = makeDescriptor({ id: "vault", name: "凭据", defaultAliases: ["v", "vault"] });
  const hidden = makeDescriptor({
    id: "suggestion",
    name: "建议",
    hiddenInSettings: true,
    defaultAliases: [],
  });

  it("uses descriptor defaults when config is empty", () => {
    const view = mergeView([todo, vault], QUICK_COMMAND_DESCRIPTORS, buildDefaultConfig());
    expect(view.providers.find((p) => p.id === "todo")?.enabled).toBe(true);
    expect(view.aliasMap.get("t")).toBe("todo");
    expect(view.aliasMap.get("v")).toBe("vault");
    expect(view.enabledQuickCommands.has("todo-create")).toBe(true);
    expect(view.enabledQuickCommands.has("calc")).toBe(true);
  });

  it("applies provider override and excludes disabled aliases from map", () => {
    const config: SpotlightConfig = {
      version: 1,
      providers: { todo: { enabled: false }, vault: { aliases: ["k"] } },
      quickCommands: {},
    };
    const view = mergeView([todo, vault], QUICK_COMMAND_DESCRIPTORS, config);
    expect(view.providers.find((p) => p.id === "todo")?.enabled).toBe(false);
    expect(view.aliasMap.has("t")).toBe(false); // todo disabled
    expect(view.aliasMap.has("v")).toBe(false); // vault overridden
    expect(view.aliasMap.get("k")).toBe("vault");
  });

  it("hidden providers participate in alias map when enabled", () => {
    const view = mergeView([hidden], QUICK_COMMAND_DESCRIPTORS, buildDefaultConfig());
    expect(view.providers.find((p) => p.id === "suggestion")?.enabled).toBe(true);
  });

  it("applies quick command override", () => {
    const config: SpotlightConfig = {
      version: 1,
      providers: {},
      quickCommands: { calc: { enabled: false } },
    };
    const view = mergeView([todo], QUICK_COMMAND_DESCRIPTORS, config);
    expect(view.enabledQuickCommands.has("calc")).toBe(false);
    expect(view.enabledQuickCommands.has("todo-create")).toBe(true);
  });
});

describe("validateAliasesPure", () => {
  const todo = makeDescriptor({ id: "todo", name: "任务", defaultAliases: ["t", "todo"] });
  const vault = makeDescriptor({ id: "vault", name: "凭据", defaultAliases: ["v", "vault"] });

  it("accepts unique custom aliases", () => {
    const r = validateAliasesPure(["q", "task"], "todo", [todo, vault], buildDefaultConfig());
    expect(r.ok).toBe(true);
    expect(r.normalized).toEqual(["q", "task"]);
  });

  it("rejects cross-provider duplicates", () => {
    const r = validateAliasesPure(["v"], "todo", [todo, vault], buildDefaultConfig());
    expect(r.ok).toBe(false);
    expect(r.conflicts[0].alias).toBe("v");
    expect(r.conflicts[0].reason).toMatch(/凭据/);
  });

  it("rejects reserved tokens", () => {
    const r = validateAliasesPure(["+", "calc", "ok"], "todo", [todo, vault], buildDefaultConfig());
    expect(r.ok).toBe(false);
    expect(r.conflicts.map((c) => c.alias).sort()).toEqual(["+", "calc"]);
  });

  it("rejects invalid characters (chinese, spaces)", () => {
    const r = validateAliasesPure(["中文", "ab cd"], "todo", [todo, vault], buildDefaultConfig());
    expect(r.ok).toBe(false);
    expect(r.conflicts.length).toBe(2);
  });

  it("considers other-provider overrides in conflict check", () => {
    const config: SpotlightConfig = {
      version: 1,
      providers: { vault: { aliases: ["k"] } },
      quickCommands: {},
    };
    // todo wants "k" — should conflict with vault's overridden alias
    const r = validateAliasesPure(["k"], "todo", [todo, vault], config);
    expect(r.ok).toBe(false);
    expect(r.conflicts[0].reason).toMatch(/凭据/);
  });
});

describe("sanitizeConfig", () => {
  const todo = makeDescriptor({ id: "todo", name: "任务" });

  it("returns null for non-object input", () => {
    expect(sanitizeConfig(null, [todo], QUICK_COMMAND_DESCRIPTORS)).toBeNull();
    expect(sanitizeConfig("string", [todo], QUICK_COMMAND_DESCRIPTORS)).toBeNull();
  });

  it("returns null for wrong version", () => {
    expect(sanitizeConfig({ version: 2 }, [todo], QUICK_COMMAND_DESCRIPTORS)).toBeNull();
  });

  it("drops unknown provider keys", () => {
    const r = sanitizeConfig(
      { version: 1, providers: { todo: { enabled: false }, bogus: { enabled: true } } },
      [todo],
      QUICK_COMMAND_DESCRIPTORS,
    );
    expect(r?.providers.todo?.enabled).toBe(false);
    expect((r?.providers as Record<string, unknown>)["bogus"]).toBeUndefined();
  });
});

/* ───── 模块单例 集成行为 ───── */

class FakePersistence {
  store = new Map<string, string>();
  reads: string[] = [];
  writes: Array<[string, string]> = [];
  readError: Error | null = null;
  writeError: Error | null = null;

  async read(key: string): Promise<string | null> {
    this.reads.push(key);
    if (this.readError) throw this.readError;
    return this.store.get(key) ?? null;
  }
  async write(key: string, value: string): Promise<void> {
    this.writes.push([key, value]);
    if (this.writeError) throw this.writeError;
    this.store.set(key, value);
  }
}

describe("config-store singleton", () => {
  it("returns default view when no stored config", async () => {
    const fake = new FakePersistence();
    __setPersistenceForTests(fake);
    const view = await ensureLoaded(true);
    expect(view.providers.length).toBeGreaterThan(0);
    expect(getLastLoadError()).toBeNull();
  });

  it("saves and emits to subscribers", async () => {
    const fake = new FakePersistence();
    __setPersistenceForTests(fake);
    await ensureLoaded(true);
    const events: number[] = [];
    const unsub = (await import("./config-store")).subscribe(() => {
      events.push(1);
    });
    const next: SpotlightConfig = JSON.parse(JSON.stringify(getConfig()));
    next.providers.todo = { enabled: false };
    await saveConfig(next);
    expect(events.length).toBe(1);
    expect(fake.writes.length).toBe(1);
    expect(fake.writes[0][0]).toBe("spotlight_config_v1");
    expect(getView().providers.find((p) => p.id === "todo")?.enabled).toBe(false);
    unsub();
  });

  it("rolls back cached config on write failure", async () => {
    const fake = new FakePersistence();
    __setPersistenceForTests(fake);
    await ensureLoaded(true);
    const prevTodoEnabled = getView().providers.find((p) => p.id === "todo")?.enabled;
    fake.writeError = new Error("disk full");
    const next: SpotlightConfig = JSON.parse(JSON.stringify(getConfig()));
    next.providers.todo = { enabled: false };
    await expect(saveConfig(next)).rejects.toThrow("disk full");
    expect(getView().providers.find((p) => p.id === "todo")?.enabled).toBe(prevTodoEnabled);
  });

  it("falls back to defaults and writes backup when JSON is malformed", async () => {
    const fake = new FakePersistence();
    fake.store.set("spotlight_config_v1", "{not json");
    __setPersistenceForTests(fake);
    await ensureLoaded(true);
    expect(getLastLoadError()).not.toBeNull();
    expect(fake.store.get("spotlight_config_v1.backup")).toBe("{not json");
  });

  it("falls back to defaults when version mismatches and writes backup", async () => {
    const fake = new FakePersistence();
    fake.store.set("spotlight_config_v1", JSON.stringify({ version: 2 }));
    __setPersistenceForTests(fake);
    await ensureLoaded(true);
    expect(getLastLoadError()).toBe("配置版本不兼容");
    expect(fake.store.get("spotlight_config_v1.backup")).toBe(JSON.stringify({ version: 2 }));
  });

  it("validateAliases delegates to pure helper with cached config", async () => {
    const fake = new FakePersistence();
    __setPersistenceForTests(fake);
    await ensureLoaded(true);
    // 任何注册 provider 的别名:vault 默认 "v"
    const r = validateAliases(["v"], "todo");
    expect(r.ok).toBe(false);
  });
});
