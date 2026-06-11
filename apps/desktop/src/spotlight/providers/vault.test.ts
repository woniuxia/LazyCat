import { describe, expect, it, vi } from "vitest";

// 桩掉测试环境不可达的 Tauri 依赖，并斩断 registry -> tool provider 的重依赖链
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => undefined),
  convertFileSrc: (s: string) => s,
}));
vi.mock("../../bridge/tauri", () => ({
  invokeToolByChannel: vi.fn(async () => null),
}));
vi.mock("../../utils/vaultClipboard", () => ({
  writeSecretToClipboard: vi.fn(async () => undefined),
  scheduleClipboardClear: vi.fn(),
}));
vi.mock("../registry", () => ({
  registerProvider: vi.fn(),
}));

import { buildItem, buildSubtitle, type VaultMetaEntry } from "./vault";
import { toPinyinInitials } from "../../utils/fuzzy-match";

function entry(overrides: Partial<VaultMetaEntry> = {}): VaultMetaEntry {
  return {
    id: 1,
    category: "database",
    title: "生产数据库",
    environment: "生产",
    viewCount: 0,
    copyCount: 0,
    plainFields: null,
    tags: [],
    createdAt: "",
    updatedAt: "",
    ...overrides,
  };
}

describe("vault provider buildItem", () => {
  it("plainFields.account 进入搜索索引，权重 1.1 且带拼音首字母", () => {
    const item = buildItem(entry({ plainFields: { account: "root@主库" } }), false);
    const field = item.searchFields.find((f) => f.text === "root@主库");
    expect(field).toBeDefined();
    expect(field!.weight).toBe(1.1);
    expect(field!.initials).toBe(toPinyinInitials("root@主库"));
  });

  it("非密码明文字段按设计权重进入索引，空串字段被过滤", () => {
    const item = buildItem(
      entry({
        plainFields: {
          account: "pg",
          address: "10.0.0.8",
          dbName: "orders",
          schema: "",
          dbType: "PostgreSQL",
          notes: "备库只读",
        },
      }),
      true,
    );
    const weightByText = Object.fromEntries(item.searchFields.map((f) => [f.text, f.weight]));
    expect(weightByText["pg"]).toBe(1.1);
    expect(weightByText["10.0.0.8"]).toBe(0.8);
    expect(weightByText["orders"]).toBe(0.8);
    expect(weightByText["PostgreSQL"]).toBe(0.6);
    expect(weightByText["备库只读"]).toBe(0.5);
    expect(item.searchFields.every((f) => f.text.length > 0)).toBe(true);
  });

  it("plainFields 为 null（未迁移）时字段集合与现状一致（空串过滤除外）", () => {
    const item = buildItem(entry({ title: "测试", environment: "", tags: ["a"] }), false);
    expect(item.searchFields.map((f) => [f.text, f.weight])).toEqual([
      ["测试", 1.2],
      ["a", 1.0],
      ["数据库", 0.6],
    ]);
  });

  it("payload.account 透传，无账号时为空串", () => {
    const withAccount = buildItem(entry({ plainFields: { account: "root" } }), true);
    expect(withAccount.payload?.account).toBe("root");
    const without = buildItem(entry(), true);
    expect(without.payload?.account).toBe("");
  });
});

describe("vault provider buildSubtitle", () => {
  it("副标题为 分类 · 环境 · 账号", () => {
    const subtitle = buildSubtitle(
      entry({
        category: "app",
        environment: "生产",
        tags: ["t1"],
        plainFields: { account: "admin@x.com" },
      }),
    );
    expect(subtitle).toBe("应用 · 生产 · admin@x.com");
  });

  it("账号为空时省略该段，标签不再展示", () => {
    const subtitle = buildSubtitle(entry({ category: "app", environment: "生产", tags: ["t1"] }));
    expect(subtitle).toBe("应用 · 生产");
  });

  it("环境为空时省略该段", () => {
    const subtitle = buildSubtitle(
      entry({ category: "server", environment: "", plainFields: { account: "root" } }),
    );
    expect(subtitle).toBe("服务器 · root");
  });
});
