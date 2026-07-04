import { describe, expect, it } from "vitest";
import { parseApiWorkbenchKvPaste } from "./apiWorkbenchKvPaste";

describe("parseApiWorkbenchKvPaste", () => {
  it("splits single-line query string by & without url decoding", () => {
    expect(parseApiWorkbenchKvPaste("a=1&b=%E4%B8%AD")).toEqual({
      rows: [
        { enabled: true, key: "a", value: "1" },
        { enabled: true, key: "b", value: "%E4%B8%AD" },
      ],
    });
  });

  it("splits header lines by first colon and trims leading value spaces", () => {
    expect(
      parseApiWorkbenchKvPaste("Content-Type: application/json\nAccept: */*"),
    ).toEqual({
      rows: [
        { enabled: true, key: "Content-Type", value: "application/json" },
        { enabled: true, key: "Accept", value: "*/*" },
      ],
    });
  });

  it("keeps everything after the first colon intact", () => {
    expect(parseApiWorkbenchKvPaste("Referer: https://x.com/a\nAccept: */*")).toEqual({
      rows: [
        { enabled: true, key: "Referer", value: "https://x.com/a" },
        { enabled: true, key: "Accept", value: "*/*" },
      ],
    });
  });

  it("splits multi-line pairs by first equals sign", () => {
    expect(parseApiWorkbenchKvPaste("a=1\nb=2=3")).toEqual({
      rows: [
        { enabled: true, key: "a", value: "1" },
        { enabled: true, key: "b", value: "2=3" },
      ],
    });
  });

  it("returns null for plain single-line text without separators", () => {
    expect(parseApiWorkbenchKvPaste("plain")).toBeNull();
    expect(parseApiWorkbenchKvPaste("")).toBeNull();
    expect(parseApiWorkbenchKvPaste("   ")).toBeNull();
  });

  it("keeps separator-less lines as key-only rows in multi-line input", () => {
    expect(parseApiWorkbenchKvPaste("a=1\noops")).toEqual({
      rows: [
        { enabled: true, key: "a", value: "1" },
        { enabled: true, key: "oops", value: "" },
      ],
    });
  });

  it("skips empty lines and handles crlf endings", () => {
    expect(parseApiWorkbenchKvPaste("a=1\r\n\r\nb=2\r\n")).toEqual({
      rows: [
        { enabled: true, key: "a", value: "1" },
        { enabled: true, key: "b", value: "2" },
      ],
    });
  });
});
