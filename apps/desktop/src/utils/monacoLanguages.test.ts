import { describe, expect, it } from "vitest";
import {
  MAX_REFERENCE_CARD_TEXT_BYTES,
  MONACO_LANGUAGE_EXTENSIONS,
  MONACO_LANGUAGE_OPTIONS,
  detectClipboardMonacoLanguage,
  validateReferenceCardText,
} from "./monacoLanguages";

describe("Monaco language catalog", () => {
  it("keeps the shared language list and snippet extensions", () => {
    expect(MONACO_LANGUAGE_OPTIONS).toContain("plaintext");
    expect(MONACO_LANGUAGE_OPTIONS).toContain("typescript");
    expect(MONACO_LANGUAGE_EXTENSIONS.typescript).toBe("ts");
    expect(MONACO_LANGUAGE_EXTENSIONS.plaintext).toBe("txt");
    expect(Object.keys(MONACO_LANGUAGE_EXTENSIONS).sort()).toEqual([...MONACO_LANGUAGE_OPTIONS].sort());
  });

  it.each([
    ['{"port":8080}', "json"],
    ["<html><body>demo</body></html>", "html"],
    ['<?xml version="1.0"?><note><message>demo</message></note>', "xml"],
    ["SELECT * FROM users WHERE id = 1", "sql"],
    ["public class Demo { private int id; }", "java"],
    ["1710000000", "plaintext"],
    ["普通临时参考文字", "plaintext"],
  ])("maps clipboard content %s to %s", (text, language) => {
    expect(detectClipboardMonacoLanguage(text)).toBe(language);
  });

  it("falls back to plaintext beyond the clipboard detection length limit", () => {
    const text = "a".repeat(100_001);
    expect(detectClipboardMonacoLanguage(text)).toBe("plaintext");
  });

  it("accepts non-empty text", () => {
    expect(validateReferenceCardText("  demo  ")).toEqual({ ok: true });
  });

  it("accepts exactly 8 MiB of ASCII text and rejects one byte more", () => {
    expect(validateReferenceCardText("a".repeat(MAX_REFERENCE_CARD_TEXT_BYTES))).toEqual({ ok: true });
    expect(validateReferenceCardText("a".repeat(MAX_REFERENCE_CARD_TEXT_BYTES + 1))).toEqual({
      ok: false,
      message: "参考文本不能超过 8 MiB",
    });
  });

  it("rejects whitespace-only text", () => {
    expect(validateReferenceCardText(" \r\n ")).toEqual({ ok: false, message: "剪贴板中没有可用文本" });
  });

  it("rejects text whose UTF-8 byte length exceeds 8 MiB", () => {
    const oversizedText = "中".repeat(Math.floor(MAX_REFERENCE_CARD_TEXT_BYTES / 3) + 1);
    expect(oversizedText.length).toBeLessThan(MAX_REFERENCE_CARD_TEXT_BYTES);
    expect(validateReferenceCardText(oversizedText)).toEqual({
      ok: false,
      message: "参考文本不能超过 8 MiB",
    });
  });
});
