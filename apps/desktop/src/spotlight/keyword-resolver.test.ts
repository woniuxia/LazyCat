import { describe, expect, it, vi, beforeEach } from "vitest";
import {
  resolveKeywordInvocation,
  isKeywordItem,
  buildKeywordItemActions,
} from "./keyword-resolver";
import type { KeywordCommandDescriptor, SpotlightItem } from "./types";

// 把 @tauri-apps/api/core 和 bridge/tauri 模块 stub 掉,这两个在测试环境不可达。
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => undefined),
  convertFileSrc: (s: string) => s,
}));
vi.mock("../bridge/tauri", () => ({
  invokeToolByChannel: vi.fn(async () => null),
}));
vi.mock("../utils/vaultClipboard", () => ({
  writeSecretToClipboard: vi.fn(async () => undefined),
  scheduleClipboardClear: vi.fn(),
}));

const uuidCmd: KeywordCommandDescriptor = {
  id: "uuid",
  keyword: "uuid",
  name: "UUID",
  description: "",
  kind: "show-value",
  origin: "builtin",
  valueProducer: "uuid-v4",
  defaultEnabled: true,
};

const tsCmd: KeywordCommandDescriptor = {
  id: "ts",
  keyword: "ts",
  name: "TS",
  description: "",
  kind: "show-value",
  origin: "builtin",
  valueProducer: "timestamp-now",
  defaultEnabled: true,
};

const jwtCmd: KeywordCommandDescriptor = {
  id: "jwt",
  keyword: "jwt",
  name: "JWT",
  description: "",
  kind: "open-tool",
  origin: "builtin",
  toolId: "jwt",
  forwardArgs: true,
  defaultEnabled: true,
};

describe("isKeywordItem", () => {
  it("returns true for items with __keyword__ providerId", () => {
    const item: SpotlightItem = {
      providerId: "__keyword__",
      itemId: "x",
      title: "x",
      searchFields: [],
    };
    expect(isKeywordItem(item)).toBe(true);
  });

  it("returns false for normal provider items", () => {
    const item: SpotlightItem = {
      providerId: "tool",
      itemId: "base64",
      title: "Base64",
      searchFields: [],
    };
    expect(isKeywordItem(item)).toBe(false);
  });
});

describe("resolveKeywordInvocation - show-value producers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("uuid producer returns 5 unique UUIDs", async () => {
    const items = await resolveKeywordInvocation({
      kind: "keyword",
      command: uuidCmd,
      args: "",
    });
    expect(items).toHaveLength(5);
    const values = items.map((i) => (i.payload as { value?: string }).value ?? "");
    const unique = new Set(values);
    expect(unique.size).toBe(5);
    for (const v of values) {
      expect(v).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i);
    }
  });

  it("uuid items carry copyable value payload", async () => {
    const [first] = await resolveKeywordInvocation({
      kind: "keyword",
      command: uuidCmd,
      args: "",
    });
    expect(first.providerId).toBe("__keyword__");
    expect((first.payload as { __keyword?: boolean }).__keyword).toBe(true);
    expect((first.payload as { keywordItemKind?: string }).keywordItemKind).toBe(
      "show-value",
    );
  });

  it("timestamp producer returns 5 representations", async () => {
    const items = await resolveKeywordInvocation({
      kind: "keyword",
      command: tsCmd,
      args: "",
    });
    expect(items).toHaveLength(5);
    const subtitles = items.map((i) => i.subtitle);
    expect(subtitles).toContain("Unix 秒");
    expect(subtitles).toContain("Unix 毫秒");
    expect(subtitles).toContain("ISO 8601");
    expect(subtitles).toContain("RFC 3339");
    expect(subtitles).toContain("本地友好");
  });
});

describe("resolveKeywordInvocation - open-tool", () => {
  it("returns a single suggestion item with args forwarded", async () => {
    const items = await resolveKeywordInvocation({
      kind: "keyword",
      command: jwtCmd,
      args: "eyJhbGciOiJIUzI1NiJ9.payload.sig",
    });
    expect(items).toHaveLength(1);
    expect(items[0].title).toContain("JWT");
    expect((items[0].payload as { toolId?: string }).toolId).toBe("jwt");
    expect((items[0].payload as { text?: string }).text).toBe(
      "eyJhbGciOiJIUzI1NiJ9.payload.sig",
    );
  });

  it("forwards empty args when no args provided", async () => {
    const items = await resolveKeywordInvocation({
      kind: "keyword",
      command: jwtCmd,
      args: "",
    });
    expect((items[0].payload as { text?: string }).text).toBe("");
  });

  it("does not forward args when forwardArgs=false", async () => {
    const items = await resolveKeywordInvocation({
      kind: "keyword",
      command: { ...jwtCmd, forwardArgs: false },
      args: "abc",
    });
    expect((items[0].payload as { text?: string }).text).toBe("");
  });
});

describe("buildKeywordItemActions", () => {
  it("returns copy action for show-value items", () => {
    const item: SpotlightItem = {
      providerId: "__keyword__",
      itemId: "x",
      title: "x",
      searchFields: [],
      payload: { __keyword: true, keywordItemKind: "show-value", value: "abc" },
    };
    const actions = buildKeywordItemActions(item);
    expect(actions).toHaveLength(1);
    expect(actions[0].id).toBe("copy");
  });

  it("returns open action for open-tool items", () => {
    const item: SpotlightItem = {
      providerId: "__keyword__",
      itemId: "x",
      title: "x",
      searchFields: [],
      payload: {
        __keyword: true,
        keywordItemKind: "open-tool",
        toolId: "base64",
        text: "abc",
      },
    };
    const actions = buildKeywordItemActions(item);
    expect(actions).toHaveLength(1);
    expect(actions[0].id).toBe("open");
  });

  it("returns vault-related actions for vault-entry items", () => {
    const item: SpotlightItem = {
      providerId: "__keyword__",
      itemId: "x",
      title: "x",
      searchFields: [],
      payload: {
        __keyword: true,
        keywordItemKind: "vault-entry",
        entryId: 1,
        title: "x",
        unlocked: false,
      },
    };
    const actions = buildKeywordItemActions(item);
    expect(actions.map((a) => a.id)).toEqual(["copy_password", "open_vault"]);
  });

  it("returns snippet-related actions for snippet-entry items", () => {
    const item: SpotlightItem = {
      providerId: "__keyword__",
      itemId: "x",
      title: "x",
      searchFields: [],
      payload: {
        __keyword: true,
        keywordItemKind: "snippet-entry",
        entryId: 1,
        title: "x",
        defaultCode: "",
      },
    };
    const actions = buildKeywordItemActions(item);
    expect(actions.map((a) => a.id)).toEqual(["copy_code", "open_snippets"]);
  });

  it("returns empty actions for hint items", () => {
    const item: SpotlightItem = {
      providerId: "__keyword__",
      itemId: "x",
      title: "x",
      searchFields: [],
      payload: { __keyword: true, keywordItemKind: "hint" },
    };
    expect(buildKeywordItemActions(item)).toEqual([]);
  });

  it("returns empty actions for non-keyword items", () => {
    const item: SpotlightItem = {
      providerId: "tool",
      itemId: "base64",
      title: "Base64",
      searchFields: [],
    };
    expect(buildKeywordItemActions(item)).toEqual([]);
  });
});
