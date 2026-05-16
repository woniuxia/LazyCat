import { describe, expect, it } from "vitest";
import { parseSpotlightQuery, dropScopePrefix } from "./spotlight-query";

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
