import { describe, expect, it } from "vitest";
import { formatByteSize, formatDurationMs, formatRelativeTime } from "./format";

describe("formatByteSize", () => {
  it("非法与非正值返回 0 B", () => {
    expect(formatByteSize(0)).toBe("0 B");
    expect(formatByteSize(-10)).toBe("0 B");
    expect(formatByteSize(Number.NaN)).toBe("0 B");
    expect(formatByteSize(Number.POSITIVE_INFINITY)).toBe("0 B");
  });

  it("按 1024 进位并保留一位小数", () => {
    expect(formatByteSize(512)).toBe("512 B");
    expect(formatByteSize(1536)).toBe("1.5 KB");
    expect(formatByteSize(2 * 1024 * 1024)).toBe("2 MB");
    expect(formatByteSize(3.5 * 1024 * 1024 * 1024)).toBe("3.5 GB");
  });

  it("超大值封顶 GB 单位", () => {
    expect(formatByteSize(5 * 1024 ** 4)).toBe("5120 GB");
  });
});

describe("formatDurationMs", () => {
  it("非法与负值返回 0 ms", () => {
    expect(formatDurationMs(Number.NaN)).toBe("0 ms");
    expect(formatDurationMs(-5)).toBe("0 ms");
  });

  it("小于 1 秒显示毫秒", () => {
    expect(formatDurationMs(0)).toBe("0 ms");
    expect(formatDurationMs(356)).toBe("356 ms");
    expect(formatDurationMs(999)).toBe("999 ms");
  });

  it("1 秒到 1 分钟显示秒并保留一位小数", () => {
    expect(formatDurationMs(1000)).toBe("1 s");
    expect(formatDurationMs(1400)).toBe("1.4 s");
    expect(formatDurationMs(59949)).toBe("59.9 s");
  });

  it("超过 1 分钟显示分秒", () => {
    expect(formatDurationMs(60000)).toBe("1 m");
    expect(formatDurationMs(65000)).toBe("1 m 5 s");
    expect(formatDurationMs(125000)).toBe("2 m 5 s");
  });
});

describe("formatRelativeTime", () => {
  const now = new Date(2026, 6, 10, 12, 0, 0);
  const iso = (d: Date) => d.toISOString();

  it("非法输入原样返回", () => {
    expect(formatRelativeTime("not-a-date", now)).toBe("not-a-date");
    expect(formatRelativeTime("", now)).toBe("");
  });

  it("60 秒内与未来时间显示刚刚", () => {
    expect(formatRelativeTime(iso(new Date(2026, 6, 10, 11, 59, 30)), now)).toBe("刚刚");
    expect(formatRelativeTime(iso(new Date(2026, 6, 10, 12, 0, 30)), now)).toBe("刚刚");
  });

  it("1 小时内显示 N 分钟前", () => {
    expect(formatRelativeTime(iso(new Date(2026, 6, 10, 11, 58, 0)), now)).toBe("2 分钟前");
    expect(formatRelativeTime(iso(new Date(2026, 6, 10, 11, 1, 0)), now)).toBe("59 分钟前");
  });

  it("24 小时内显示 N 小时前", () => {
    expect(formatRelativeTime(iso(new Date(2026, 6, 10, 9, 0, 0)), now)).toBe("3 小时前");
    expect(formatRelativeTime(iso(new Date(2026, 6, 9, 13, 0, 0)), now)).toBe("23 小时前");
  });

  it("昨天显示 昨天 HH:mm", () => {
    expect(formatRelativeTime(iso(new Date(2026, 6, 9, 8, 5, 0)), now)).toBe("昨天 08:05");
  });

  it("同年更早显示 MM-DD HH:mm", () => {
    expect(formatRelativeTime(iso(new Date(2026, 5, 1, 9, 30, 0)), now)).toBe("06-01 09:30");
  });

  it("跨年显示 YYYY-MM-DD", () => {
    expect(formatRelativeTime(iso(new Date(2025, 11, 31, 9, 30, 0)), now)).toBe("2025-12-31");
  });
});
