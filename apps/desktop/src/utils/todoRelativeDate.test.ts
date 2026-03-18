import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { formatTodoRelativeDateTimeLabel } from "./todoRelativeDate";

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(2026, 2, 18, 12, 0, 0));
});

afterEach(() => {
  vi.useRealTimers();
});

describe("todoRelativeDate", () => {
  it("formats today, tomorrow and yesterday with highest priority", () => {
    expect(formatTodoRelativeDateTimeLabel("2026-03-18T09:00:00")).toBe("今天 09:00");
    expect(formatTodoRelativeDateTimeLabel("2026-03-19T09:05:00")).toBe("明天 09:05");
    expect(formatTodoRelativeDateTimeLabel("2026-03-17T18:30:00")).toBe("昨天 18:30");
  });

  it("formats dates in the same natural week as weekday labels", () => {
    expect(formatTodoRelativeDateTimeLabel("2026-03-20T08:15:00")).toBe("周五 08:15");
    expect(formatTodoRelativeDateTimeLabel("2026-03-16T09:00:00")).toBe("周一 09:00");
  });

  it("formats next-week dates with a 下 prefix even when fewer than seven days away", () => {
    expect(formatTodoRelativeDateTimeLabel("2026-03-23T09:30:00")).toBe("下周一 09:30");
    expect(formatTodoRelativeDateTimeLabel("2026-03-25T09:30:00")).toBe("下周三 09:30");
  });

  it("formats previous-week dates with an 上 prefix", () => {
    expect(formatTodoRelativeDateTimeLabel("2026-03-13T09:00:00")).toBe("上周五 09:00");
  });

  it("falls back to absolute dates beyond adjacent weeks", () => {
    expect(formatTodoRelativeDateTimeLabel("2026-04-10T09:00:00")).toBe("4月10日 09:00");
    expect(formatTodoRelativeDateTimeLabel("2027-01-02T09:00:00")).toBe("2027/01/02 09:00");
  });

  it("treats date-only strings as local dates instead of UTC-shifted instants", () => {
    expect(formatTodoRelativeDateTimeLabel("2026-03-23")).toBe("下周一 00:00");
  });

  it("keeps tomorrow and yesterday ahead of cross-week labels", () => {
    const sunday = new Date(2026, 2, 22, 12, 0, 0);
    expect(formatTodoRelativeDateTimeLabel("2026-03-23T09:00:00", sunday)).toBe("明天 09:00");

    const monday = new Date(2026, 2, 23, 12, 0, 0);
    expect(formatTodoRelativeDateTimeLabel("2026-03-22T09:00:00", monday)).toBe("昨天 09:00");
  });

  it("returns an empty string for invalid input", () => {
    expect(formatTodoRelativeDateTimeLabel()).toBe("");
    expect(formatTodoRelativeDateTimeLabel("not-a-date")).toBe("");
  });
});
