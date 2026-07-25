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
  });

  it.each([
    ['{"port":8080}', "json"],
    ["<html><body>demo</body></html>", "html"],
    ["SELECT * FROM users WHERE id = 1", "sql"],
    ["public class Demo { private int id; }", "java"],
    ["普通临时参考文字", "plaintext"],
  ])("maps clipboard content %s to %s", (text, language) => {
    expect(detectClipboardMonacoLanguage(text)).toBe(language);
  });

  it("accepts non-empty text", () => {
    expect(validateReferenceCardText("  demo  ")).toEqual({ ok: true });
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
