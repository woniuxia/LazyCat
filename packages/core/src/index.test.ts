import { describe, expect, it } from "vitest";
import {
  encodeBase64,
  decodeBase64,
  md5,
  jsonToYaml,
  uniqueLines,
  sortLines,
  dateTimeToTimestamp,
  generateCronExpression
} from "./index";

describe("core utils", () => {
  it("base64 encode/decode", () => {
    const input = "Lazycat";
    const encoded = encodeBase64(input);
    expect(encoded).toBe("TGF6eWNhdA==");
    expect(decodeBase64(encoded)).toBe(input);
  });

  it("md5 hash", () => {
    expect(md5("abc")).toBe("900150983cd24fb0d6963f7d28e17f72");
  });

  it("json to yaml", () => {
    const yaml = jsonToYaml('{"name":"lazycat","ok":true}');
    expect(yaml).toContain("name: lazycat");
    expect(yaml).toContain("ok: true");
  });

  it("line process", () => {
    expect(uniqueLines("a\na\nb")).toBe("a\nb");
    expect(sortLines("c\nb\na")).toBe("a\nb\nc");
  });

  it("datetime to timestamp", () => {
    const ts = dateTimeToTimestamp("2026-01-01T00:00:00.000Z");
    expect(ts.seconds).toBe(1767225600);
    expect(ts.milliseconds).toBe(1767225600000);
  });

  it("cron expression generate", () => {
    expect(
      generateCronExpression({
        second: "0",
        minute: "*/5",
        hour: "*",
        dayOfMonth: "*",
        month: "*",
        dayOfWeek: "*"
      })
    ).toBe("0 */5 * * * *");
  });
});
