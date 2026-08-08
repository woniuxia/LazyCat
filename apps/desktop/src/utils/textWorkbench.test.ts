import { describe, expect, it } from "vitest";
import {
  detectMonacoLanguage,
  fileNameFromPath,
  filePathsMatch,
  summarizeDiff,
} from "./textWorkbench";

describe("textWorkbench", () => {
  it("detects Monaco language from Windows and Unix paths", () => {
    expect(detectMonacoLanguage("C:\\work\\demo.ts")).toBe("typescript");
    expect(detectMonacoLanguage("/tmp/config.yaml")).toBe("yaml");
    expect(detectMonacoLanguage("README.unknown")).toBe("plaintext");
    expect(fileNameFromPath("C:\\work\\demo.ts")).toBe("demo.ts");
  });

  it("matches local file paths across separators, case and trailing slashes", () => {
    expect(filePathsMatch(" C:/Logs/Error.log ", "c:\\logs\\error.log\\")).toBe(true);
    expect(filePathsMatch("C:\\logs\\error.log", "C:\\logs\\other.log")).toBe(false);
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
