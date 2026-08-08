import { describe, expect, it } from "vitest";
import { parseExceptionStack } from "./exceptionStack";

describe("parseExceptionStack", () => {
  it("parses Node/V8 frames and preserves a frame without a column", () => {
    const raw = [
      "TypeError: Cannot read properties of undefined",
      "    at loadUser (C:\\work\\app.ts:12:7)",
      "    at bootstrap (/srv/app.js:7)",
    ].join("\n");

    const result = parseExceptionStack(raw);

    expect(result.ok).toBe(true);
    expect(result.format).toBe("javascript");
    expect(result.detection).toBe("javascript");
    expect(result.rootException).toEqual({
      type: "TypeError",
      message: "Cannot read properties of undefined",
      lineNumber: 1,
    });
    expect(result.frames).toMatchObject([
      {
        functionName: "loadUser",
        filePath: "C:\\work\\app.ts",
        line: 12,
        column: 7,
      },
      {
        functionName: "bootstrap",
        filePath: "/srv/app.js",
        line: 7,
        column: null,
      },
    ]);
    expect(result.summary).toContain("TypeError: Cannot read properties of undefined");
    expect(result.summary).toContain("loadUser (C:\\work\\app.ts:12:7)");
  });

  it("parses browser frames and reports the selected format", () => {
    const result = parseExceptionStack(
      [
        "Uncaught ReferenceError: missingValue is not defined",
        "render@https://app.test/main.js:4:9",
      ].join("\n"),
    );

    expect(result.ok).toBe(true);
    expect(result.format).toBe("javascript");
    expect(result.frames[0]).toMatchObject({
      functionName: "render",
      filePath: "https://app.test/main.js",
      line: 4,
      column: 9,
    });
  });

  it("parses anonymous browser frames and bare async V8 frames", () => {
    const result = parseExceptionStack(
      [
        "Error: browser and node failure",
        "@https://app.test/main.js:4:9",
        "    at async file:///app.mjs:1:2",
      ].join("\n"),
    );

    expect(result.ok).toBe(true);
    expect(result.frames).toMatchObject([
      {
        functionName: "",
        filePath: "https://app.test/main.js",
        line: 4,
        column: 9,
      },
      {
        functionName: "",
        filePath: "file:///app.mjs",
        line: 1,
        column: 2,
      },
    ]);
  });

  it("does not confuse dotted JavaScript calls with Java frames", () => {
    const result = parseExceptionStack(
      ["Error: browser failure", "    at namespace.load (/srv/app.js:4:9)"].join("\n"),
    );

    expect(result.ok).toBe(true);
    expect(result.format).toBe("javascript");
    expect(result.frames[0]).toMatchObject({
      functionName: "namespace.load",
      filePath: "/srv/app.js",
      line: 4,
      column: 9,
    });
  });

  it("parses Java causes and common-frame omission markers", () => {
    const raw = [
      'Exception in thread "main" java.lang.IllegalStateException: top failure',
      "\tat com.example.Main.run(Main.java:42)",
      "Caused by: java.io.IOException: disk failure",
      "\tat com.example.Store.read(Store.java:18)",
      "\t... 3 more",
    ].join("\n");

    const result = parseExceptionStack(raw);

    expect(result.ok).toBe(true);
    expect(result.format).toBe("java");
    expect(result.rootException).toMatchObject({
      type: "java.lang.IllegalStateException",
      message: "top failure",
    });
    expect(result.causes).toEqual([
      {
        type: "java.io.IOException",
        message: "disk failure",
        lineNumber: 3,
      },
    ]);
    expect(result.frames).toMatchObject([
      { functionName: "com.example.Main.run", filePath: "Main.java", line: 42 },
      { functionName: "com.example.Store.read", filePath: "Store.java", line: 18 },
    ]);
    expect(result.abbreviatedFrameCount).toBe(3);
    expect(result.omissionMarkers).toEqual([{ count: 3, lineNumber: 5, raw: "\t... 3 more" }]);
  });

  it("selects the final five frames globally and restores source order", () => {
    const frames = Array.from(
      { length: 7 },
      (_, index) => `    at fn${index} (/tmp/${index}.js:${index + 1}:2)`,
    );
    const result = parseExceptionStack(["Error: many frames", ...frames].join("\n"));

    expect(result.ok).toBe(true);
    expect(result.frames.map((frame) => frame.functionName)).toEqual([
      "fn2",
      "fn3",
      "fn4",
      "fn5",
      "fn6",
    ]);
    expect(result.omittedFrameCount).toBe(2);
    expect(result.summary).toContain("省略 2 个可识别帧");
  });

  it("allows a manual format override for ambiguous input", () => {
    const raw = "Exception: generic failure";
    const automatic = parseExceptionStack(raw);
    const manual = parseExceptionStack(raw, "java");

    expect(automatic.ok).toBe(false);
    expect(automatic.detection).toBe("ambiguous");
    expect(manual.ok).toBe(true);
    expect(manual.format).toBe("java");
    expect(manual.formatSource).toBe("manual");
  });

  it("keeps useful partial results and unrecognized lines", () => {
    const result = parseExceptionStack(
      ["TypeError: bad input", "context from a logger", "    at parse (/tmp/app.js:3:1)"].join(
        "\n",
      ),
    );

    expect(result.ok).toBe(true);
    expect(result.unrecognizedLines).toEqual([{ lineNumber: 2, text: "context from a logger" }]);
  });

  it("rejects multiple roots and completely unsupported input explicitly", () => {
    const multipleRoots = parseExceptionStack(
      ["Error: first", "    at first (/tmp/first.js:1:1)", "Error: second"].join("\n"),
    );
    const unsupported = parseExceptionStack("Traceback (most recent call last):\n  unknown");

    expect(multipleRoots.ok).toBe(false);
    expect(multipleRoots.rootException).toBeNull();
    expect(multipleRoots.frames).toEqual([]);
    expect(
      multipleRoots.diagnostics.some((message) => message.includes("输入包含多个独立的根异常")),
    ).toBe(true);
    expect(unsupported.ok).toBe(false);
    expect(unsupported.detection).toBe("unsupported");
    expect(unsupported.summary).toBe("");
  });
});
