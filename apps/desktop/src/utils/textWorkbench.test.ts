import { describe, expect, it } from "vitest";
import { detectMonacoLanguage, fileNameFromPath, summarizeDiff } from "./textWorkbench";

describe("textWorkbench", () => {
  it("detects Monaco language from Windows and Unix paths", () => {
    expect(detectMonacoLanguage("C:\\work\\demo.ts")).toBe("typescript");
    expect(detectMonacoLanguage("/tmp/config.yaml")).toBe("yaml");
    expect(detectMonacoLanguage("README.unknown")).toBe("plaintext");
    expect(fileNameFromPath("C:\\work\\demo.ts")).toBe("demo.ts");
  });

  it("summarizes inserted, removed and changed lines", () => {
    expect(
      summarizeDiff([
        {
          originalStartLineNumber: 2,
          originalEndLineNumber: 3,
          modifiedStartLineNumber: 2,
          modifiedEndLineNumber: 4,
        },
        {
          originalStartLineNumber: 8,
          originalEndLineNumber: 8,
          modifiedStartLineNumber: 9,
          modifiedEndLineNumber: 0,
        },
      ]),
    ).toEqual({ hunks: 2, addedLines: 1, removedLines: 1, changedLines: 2 });
  });
});
