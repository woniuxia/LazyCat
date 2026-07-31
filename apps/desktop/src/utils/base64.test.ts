import { describe, expect, it } from "vitest";

import { detectBase64Kind, resolveBase64DecodeKind } from "./base64";

describe("base64", () => {
  it("识别明确的 Standard 输入", () => {
    expect(detectBase64Kind("Zg==")).toBe("standard");
    expect(detectBase64Kind("/+wgVQA=")).toBe("standard");
    expect(detectBase64Kind("Zm9v")).toBe("ambiguous");
  });

  it("识别明确的 URL-safe 无 padding 输入", () => {
    expect(detectBase64Kind("_-wgVQA")).toBe("url-safe");
    expect(detectBase64Kind("Zg")).toBe("url-safe");
    expect(detectBase64Kind("Zm8")).toBe("url-safe");
  });

  it("识别共享字符集且双侧都可解码的歧义输入", () => {
    expect(detectBase64Kind("test")).toBe("ambiguous");
    expect(detectBase64Kind("TWFu")).toBe("ambiguous");
  });

  it("拒绝 trailing bits 非法的看似合法输入", () => {
    expect(detectBase64Kind("AB==")).toBe("none");
    expect(detectBase64Kind("ABC=")).toBe("none");
    expect(detectBase64Kind("AB")).toBe("none");
    expect(detectBase64Kind("ABC")).toBe("none");
  });

  it("固定普通短文本的判型", () => {
    expect(detectBase64Kind("hello")).toBe("none");
    expect(detectBase64Kind("test")).toBe("ambiguous");
  });

  it("带空白时不自动识别", () => {
    expect(detectBase64Kind(" Zg==")).toBe("none");
    expect(detectBase64Kind("Zg== ")).toBe("none");
    expect(detectBase64Kind("Zg==\n")).toBe("none");
    expect(detectBase64Kind("Z g==")).toBe("none");
  });

  it("混用两套专有字符时返回 none", () => {
    expect(detectBase64Kind("ab+/cd-_")).toBe("none");
  });

  it("空字符串和全空白字符串返回 none", () => {
    expect(detectBase64Kind("")).toBe("none");
    expect(detectBase64Kind("   ")).toBe("none");
    expect(detectBase64Kind("\t\r\n")).toBe("none");
  });

  it("带 padding 的 URL-safe 风格输入返回 none", () => {
    expect(detectBase64Kind("_-wgVQA=")).toBe("none");
  });

  it("含 -/_ 且长度余 1 时返回 none", () => {
    expect(detectBase64Kind("abcd-")).toBe("none");
  });

  it("明确类型的解码决策优先自动识别", () => {
    expect(
      resolveBase64DecodeKind({
        detectedKind: "standard",
        manualChoice: "url-safe",
        currentKind: "url-safe",
      }),
    ).toBe("standard");

    expect(
      resolveBase64DecodeKind({
        detectedKind: "url-safe",
        manualChoice: "standard",
        currentKind: "standard",
      }),
    ).toBe("url-safe");
  });

  it("歧义输入的解码决策优先 manualChoice，否则回退 Standard", () => {
    expect(
      resolveBase64DecodeKind({
        detectedKind: "ambiguous",
        manualChoice: "url-safe",
        currentKind: "standard",
      }),
    ).toBe("url-safe");

    expect(
      resolveBase64DecodeKind({
        detectedKind: "ambiguous",
        manualChoice: null,
        currentKind: "url-safe",
      }),
    ).toBe("standard");
  });

  it("none 输入的解码决策沿用当前显示类型", () => {
    expect(
      resolveBase64DecodeKind({
        detectedKind: "none",
        manualChoice: "standard",
        currentKind: "url-safe",
      }),
    ).toBe("url-safe");
  });
});
