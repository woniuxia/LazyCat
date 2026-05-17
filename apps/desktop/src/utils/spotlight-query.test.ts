import { describe, expect, it } from "vitest";
import { parseSpotlightQuery, dropScopePrefix, parseQuickCommand } from "./spotlight-query";
import type { QuickCommandId, SpotlightProviderId } from "../spotlight/types";

const ALL_QC: Set<QuickCommandId> = new Set(["todo-create", "calc"]);

describe("parseSpotlightQuery", () => {
  it("returns null scope for empty input", () => {
    expect(parseSpotlightQuery("")).toEqual({ scope: null, query: "" });
    expect(parseSpotlightQuery("   ")).toEqual({ scope: null, query: "" });
  });

  it("returns null scope for single token without space", () => {
    expect(parseSpotlightQuery("t")).toEqual({ scope: null, query: "t" });
    expect(parseSpotlightQuery("vault")).toEqual({ scope: null, query: "vault" });
  });

  it("recognizes short scope prefixes", () => {
    expect(parseSpotlightQuery("t 客户")).toEqual({ scope: "todo", query: "客户" });
    expect(parseSpotlightQuery("v aws")).toEqual({ scope: "vault", query: "aws" });
    expect(parseSpotlightQuery("h dev")).toEqual({ scope: "hosts", query: "dev" });
    expect(parseSpotlightQuery("p 升级")).toEqual({ scope: "pm", query: "升级" });
  });

  it("recognizes long scope prefixes", () => {
    expect(parseSpotlightQuery("todo 客户")).toEqual({ scope: "todo", query: "客户" });
    expect(parseSpotlightQuery("vault aws")).toEqual({ scope: "vault", query: "aws" });
    expect(parseSpotlightQuery("hosts dev")).toEqual({ scope: "hosts", query: "dev" });
  });

  it("falls back to no scope for unknown prefix", () => {
    expect(parseSpotlightQuery("xyz hello")).toEqual({ scope: null, query: "xyz hello" });
  });

  it("treats trailing space after prefix as empty scoped query", () => {
    expect(parseSpotlightQuery("v ")).toEqual({ scope: "vault", query: "" });
  });

  it("preserves inner whitespace in scoped query", () => {
    expect(parseSpotlightQuery("t 客户   会议")).toEqual({ scope: "todo", query: "客户   会议" });
  });

  it("is case-insensitive on the prefix only", () => {
    expect(parseSpotlightQuery("V aws")).toEqual({ scope: "vault", query: "aws" });
    expect(parseSpotlightQuery("T 客户")).toEqual({ scope: "todo", query: "客户" });
  });
});

describe("dropScopePrefix", () => {
  it("strips a recognized scope prefix", () => {
    expect(dropScopePrefix("v aws")).toBe("aws");
    expect(dropScopePrefix("todo 周三")).toBe("周三");
  });

  it("returns original input when no scope is matched", () => {
    expect(dropScopePrefix("xyz hello")).toBe("xyz hello");
    expect(dropScopePrefix("vault")).toBe("vault");
  });
});

describe("parseQuickCommand", () => {
  it("recognizes the + prefix with text", () => {
    expect(parseQuickCommand("+ 写周报")).toEqual({ kind: "todo-create", text: "写周报" });
  });

  it("returns empty text when + prefix is followed by only whitespace", () => {
    expect(parseQuickCommand("+ ")).toEqual({ kind: "todo-create", text: "" });
    expect(parseQuickCommand("+    ")).toEqual({ kind: "todo-create", text: "" });
  });

  it("requires a space after the plus sign", () => {
    expect(parseQuickCommand("+1")).toBeNull();
    expect(parseQuickCommand("+xxx")).toBeNull();
  });

  it("tolerates leading whitespace", () => {
    expect(parseQuickCommand("  + 任务")).toEqual({ kind: "todo-create", text: "任务" });
  });

  it("returns null for non-matching inputs", () => {
    expect(parseQuickCommand("hello")).toBeNull();
    expect(parseQuickCommand("")).toBeNull();
    expect(parseQuickCommand("t 任务")).toBeNull();
  });

  it("trims trailing whitespace in the text", () => {
    expect(parseQuickCommand("+ 周报   ")).toEqual({ kind: "todo-create", text: "周报" });
  });

  it("recognizes calc + space + expression", () => {
    expect(parseQuickCommand("calc 1+2")).toEqual({ kind: "calc", text: "1+2" });
    expect(parseQuickCommand("calc 23.7%*100")).toEqual({ kind: "calc", text: "23.7%*100" });
  });

  it("accepts empty calc body", () => {
    expect(parseQuickCommand("calc ")).toEqual({ kind: "calc", text: "" });
    expect(parseQuickCommand("calc    ")).toEqual({ kind: "calc", text: "" });
  });

  it("requires whitespace after calc", () => {
    expect(parseQuickCommand("calc")).toBeNull();
    expect(parseQuickCommand("calculator")).toBeNull();
    expect(parseQuickCommand("calc1+2")).toBeNull();
  });

  it("is case-insensitive on calc prefix", () => {
    expect(parseQuickCommand("Calc 1+2")).toEqual({ kind: "calc", text: "1+2" });
    expect(parseQuickCommand("CALC 1+2")).toEqual({ kind: "calc", text: "1+2" });
  });

  it("tolerates leading whitespace before calc", () => {
    expect(parseQuickCommand("  calc 1+2")).toEqual({ kind: "calc", text: "1+2" });
  });
});

describe("parseSpotlightQuery with custom aliasMap", () => {
  it("honors a custom alias", () => {
    const map = new Map<string, SpotlightProviderId>([["q", "todo"]]);
    expect(parseSpotlightQuery("q 周报", map)).toEqual({ scope: "todo", query: "周报" });
  });

  it("rejects default prefix when custom map omits it", () => {
    const map = new Map<string, SpotlightProviderId>([["q", "todo"]]);
    expect(parseSpotlightQuery("t 周报", map)).toEqual({ scope: null, query: "t 周报" });
  });
});

describe("parseQuickCommand with enabledIds", () => {
  it("disables todo-create when not in the set", () => {
    expect(parseQuickCommand("+ 写周报", new Set<QuickCommandId>(["calc"]))).toBeNull();
  });

  it("disables calc when not in the set", () => {
    expect(parseQuickCommand("calc 1+2", new Set<QuickCommandId>(["todo-create"]))).toBeNull();
  });

  it("allows both when fully enabled", () => {
    expect(parseQuickCommand("+ x", ALL_QC)).toEqual({ kind: "todo-create", text: "x" });
    expect(parseQuickCommand("calc 1", ALL_QC)).toEqual({ kind: "calc", text: "1" });
  });
});
