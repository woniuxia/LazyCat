import { describe, expect, it } from "vitest";
import {
  BUILTIN_KEYWORD_COMMANDS,
  resolveKeywordCommands,
  validateCustomKeyword,
  generateCustomKeywordId,
} from "./keyword-commands";
import type { KeywordCommandCustom } from "./types";

describe("BUILTIN_KEYWORD_COMMANDS", () => {
  it("contains 8 built-in keyword commands", () => {
    expect(BUILTIN_KEYWORD_COMMANDS).toHaveLength(8);
  });

  it("has unique keywords and ids", () => {
    const keywords = new Set(BUILTIN_KEYWORD_COMMANDS.map((b) => b.keyword));
    const ids = new Set(BUILTIN_KEYWORD_COMMANDS.map((b) => b.id));
    expect(keywords.size).toBe(BUILTIN_KEYWORD_COMMANDS.length);
    expect(ids.size).toBe(BUILTIN_KEYWORD_COMMANDS.length);
  });

  it("normalizes keywords to lowercase", () => {
    for (const b of BUILTIN_KEYWORD_COMMANDS) {
      expect(b.keyword).toBe(b.keyword.toLowerCase());
    }
  });

  it("includes the expected built-in identifiers", () => {
    const expected = ["ip", "uuid", "ts", "hash", "b64", "jwt", "regex", "color"];
    const actual = BUILTIN_KEYWORD_COMMANDS.map((b) => b.keyword);
    for (const k of expected) {
      expect(actual).toContain(k);
    }
  });
});

describe("validateCustomKeyword", () => {
  const empty: KeywordCommandCustom[] = [];

  it("rejects empty keyword", () => {
    expect(validateCustomKeyword("", { existingCustom: empty }).ok).toBe(false);
    expect(validateCustomKeyword("   ", { existingCustom: empty }).ok).toBe(false);
  });

  it("rejects illegal characters", () => {
    expect(validateCustomKeyword("中文", { existingCustom: empty }).ok).toBe(false);
    expect(validateCustomKeyword("a b", { existingCustom: empty }).ok).toBe(false);
    expect(validateCustomKeyword("a.b", { existingCustom: empty }).ok).toBe(false);
    expect(validateCustomKeyword("a+b", { existingCustom: empty }).ok).toBe(false);
  });

  it("accepts legal characters [a-zA-Z0-9_-]", () => {
    expect(validateCustomKeyword("foo", { existingCustom: empty }).ok).toBe(true);
    expect(validateCustomKeyword("FOO_bar-1", { existingCustom: empty }).ok).toBe(true);
    expect(validateCustomKeyword("a", { existingCustom: empty }).ok).toBe(true);
  });

  it("rejects length > 24", () => {
    const long = "a".repeat(25);
    expect(validateCustomKeyword(long, { existingCustom: empty }).ok).toBe(false);
  });

  it("accepts length 24", () => {
    const fit = "a".repeat(24);
    expect(validateCustomKeyword(fit, { existingCustom: empty }).ok).toBe(true);
  });

  it("rejects collision with builtin keyword", () => {
    expect(validateCustomKeyword("ip", { existingCustom: empty }).ok).toBe(false);
    expect(validateCustomKeyword("IP", { existingCustom: empty }).ok).toBe(false);
    expect(validateCustomKeyword("jwt", { existingCustom: empty }).ok).toBe(false);
  });

  it("rejects collision with enabled custom keyword", () => {
    const existing: KeywordCommandCustom[] = [
      {
        id: "c1",
        keyword: "wifi",
        name: "WiFi",
        description: "",
        kind: "vault-tag",
        targetTag: "wifi",
        enabled: true,
      },
    ];
    expect(validateCustomKeyword("wifi", { existingCustom: existing }).ok).toBe(false);
    expect(validateCustomKeyword("WIFI", { existingCustom: existing }).ok).toBe(false);
  });

  it("allows collision when the existing entry is disabled", () => {
    const existing: KeywordCommandCustom[] = [
      {
        id: "c1",
        keyword: "wifi",
        name: "WiFi",
        description: "",
        kind: "vault-tag",
        targetTag: "wifi",
        enabled: false,
      },
    ];
    expect(validateCustomKeyword("wifi", { existingCustom: existing }).ok).toBe(true);
  });

  it("excludes selfId from collision check", () => {
    const existing: KeywordCommandCustom[] = [
      {
        id: "c1",
        keyword: "wifi",
        name: "WiFi",
        description: "",
        kind: "vault-tag",
        targetTag: "wifi",
        enabled: true,
      },
    ];
    expect(validateCustomKeyword("wifi", { selfId: "c1", existingCustom: existing }).ok).toBe(true);
  });

  it("normalizes the keyword to lowercase", () => {
    const result = validateCustomKeyword("MyKw", { existingCustom: empty });
    expect(result.ok).toBe(true);
    expect(result.normalized).toBe("mykw");
  });

  it("trims leading and trailing whitespace before validation", () => {
    expect(validateCustomKeyword("  foo  ", { existingCustom: empty }).ok).toBe(true);
  });
});

describe("resolveKeywordCommands", () => {
  it("returns all 8 builtins when config is undefined", () => {
    const { commands, index } = resolveKeywordCommands(undefined);
    expect(commands).toHaveLength(8);
    expect(index.size).toBe(8);
  });

  it("returns all 8 builtins when config is empty", () => {
    const { commands } = resolveKeywordCommands({});
    expect(commands).toHaveLength(8);
  });

  it("excludes disabled builtins", () => {
    const { commands, index } = resolveKeywordCommands({
      builtins: { ip: { enabled: false }, uuid: { enabled: false } },
    });
    expect(commands).toHaveLength(6);
    expect(index.has("ip")).toBe(false);
    expect(index.has("uuid")).toBe(false);
    expect(index.has("ts")).toBe(true);
  });

  it("adds enabled custom keyword commands", () => {
    const { commands, index } = resolveKeywordCommands({
      custom: [
        {
          id: "c1",
          keyword: "myapi",
          name: "API",
          description: "",
          kind: "snippet-tag",
          targetTag: "api",
          enabled: true,
        },
      ],
    });
    expect(commands).toHaveLength(9);
    expect(index.has("myapi")).toBe(true);
    expect(index.get("myapi")?.origin).toBe("custom");
  });

  it("skips disabled custom commands", () => {
    const { commands, index } = resolveKeywordCommands({
      custom: [
        {
          id: "c1",
          keyword: "myapi",
          name: "API",
          description: "",
          kind: "snippet-tag",
          targetTag: "api",
          enabled: false,
        },
      ],
    });
    expect(commands).toHaveLength(8);
    expect(index.has("myapi")).toBe(false);
  });

  it("filters out custom entries that conflict with builtins", () => {
    const { commands, index } = resolveKeywordCommands({
      custom: [
        {
          id: "c1",
          keyword: "ip",
          name: "Override",
          description: "",
          kind: "open-tool",
          toolId: "vault",
          forwardArgs: true,
          enabled: true,
        },
      ],
    });
    expect(commands).toHaveLength(8);
    expect(index.get("ip")?.origin).toBe("builtin");
  });

  it("filters out custom entries missing required fields", () => {
    const { commands } = resolveKeywordCommands({
      custom: [
        {
          id: "c1",
          keyword: "missing-tool",
          name: "no tool",
          description: "",
          kind: "open-tool",
          forwardArgs: true,
          enabled: true,
        } as KeywordCommandCustom,
        {
          id: "c2",
          keyword: "missing-tag",
          name: "no tag",
          description: "",
          kind: "vault-tag",
          enabled: true,
        } as KeywordCommandCustom,
      ],
    });
    expect(commands).toHaveLength(8);
  });

  it("first-registered wins for custom-vs-custom keyword collisions", () => {
    const { commands, index } = resolveKeywordCommands({
      custom: [
        {
          id: "c1",
          keyword: "dup",
          name: "First",
          description: "",
          kind: "snippet-tag",
          targetTag: "first",
          enabled: true,
        },
        {
          id: "c2",
          keyword: "dup",
          name: "Second",
          description: "",
          kind: "snippet-tag",
          targetTag: "second",
          enabled: true,
        },
      ],
    });
    expect(commands).toHaveLength(9);
    expect(index.get("dup")?.id).toBe("c1");
  });
});

describe("generateCustomKeywordId", () => {
  it("returns a non-empty string", () => {
    const id = generateCustomKeywordId();
    expect(typeof id).toBe("string");
    expect(id.length).toBeGreaterThan(4);
    expect(id.startsWith("kw-")).toBe(true);
  });

  it("returns unique ids on successive calls", () => {
    const a = generateCustomKeywordId();
    const b = generateCustomKeywordId();
    expect(a).not.toBe(b);
  });
});
