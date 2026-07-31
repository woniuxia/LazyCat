import { describe, expect, it } from "vitest";
import { buildClipboardPathSuggestion, detectClipboardPath } from "./clipboard-detect";

describe("clipboard-detect", () => {
  it("识别 Windows 文件路径并返回 reveal=true", () => {
    expect(detectClipboardPath("C:\\Windows\\notepad.exe")).toEqual({
      path: "C:\\Windows\\notepad.exe",
      reveal: true,
    });
  });

  it("识别 Windows 目录样式路径并保留 reveal=true", () => {
    expect(detectClipboardPath("C:\\Windows")).toEqual({
      path: "C:\\Windows",
      reveal: true,
    });
  });

  it("支持外层引号", () => {
    expect(detectClipboardPath('"C:\\Windows\\notepad.exe"')).toEqual({
      path: "C:\\Windows\\notepad.exe",
      reveal: true,
    });
  });

  it("支持 file URI", () => {
    expect(detectClipboardPath("file:///C:/Windows/notepad.exe")).toEqual({
      path: "C:\\Windows\\notepad.exe",
      reveal: true,
    });
  });

  it("支持 UNC 路径", () => {
    expect(detectClipboardPath("\\\\server\\share\\demo.txt")).toEqual({
      path: "\\\\server\\share\\demo.txt",
      reveal: true,
    });
  });

  it("拒绝普通文本、多行文本和环境变量路径", () => {
    expect(detectClipboardPath("hello world")).toBeNull();
    expect(detectClipboardPath("C:\\Windows\\a.txt\nC:\\Windows\\b.txt")).toBeNull();
    expect(detectClipboardPath("%USERPROFILE%\\Desktop\\a.txt")).toBeNull();
  });

  it("目录样式路径建议显示为目录路径", () => {
    expect(
      buildClipboardPathSuggestion({
        path: "C:\\Windows",
        reveal: true,
      }),
    ).toEqual({
      type: "path",
      label: "目录路径",
      preview: "C:\\Windows",
      actions: [
        {
          kind: "open-path",
          label: "直接打开",
          path: "C:\\Windows",
          reveal: true,
        },
      ],
    });
  });
});
