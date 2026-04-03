import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  formatPmDateForDisplay,
  formatPmDateRangeForDisplay,
  getPmDateRangeValue,
  hasPmDateSchedule,
  isPmItemOverdue,
  normalizePmDateRangeForDraft,
  normalizePmDateString,
  parsePmDateAtLocalEnd,
  parsePmDateAtLocalStart,
} from "./pmDate";

describe("pmDate", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 3, 5, 10, 0, 0, 0));
  });

  it("归一化 YYYY-MM-DD 与带时间部分的历史值", () => {
    expect(normalizePmDateString("2026-04-02")).toBe("2026-04-02");
    expect(normalizePmDateString("2026-04-02T08:30:00.000Z")).toBe("2026-04-02");
    expect(normalizePmDateString(" 2026-04-02 09:00:00 ")).toBe("2026-04-02");
  });

  it("拒绝非法日期字符串", () => {
    expect(normalizePmDateString("")).toBeNull();
    expect(normalizePmDateString("2026/04/02")).toBeNull();
    expect(normalizePmDateString("0000-00-00")).toBeNull();
    expect(normalizePmDateString("2026-02-30")).toBeNull();
    expect(normalizePmDateString("invalid")).toBeNull();
  });

  it("按本地日历语义构造起止时间", () => {
    const start = parsePmDateAtLocalStart("2026-04-02T08:30:00.000Z");
    const end = parsePmDateAtLocalEnd("2026-04-02");

    expect(start).not.toBeNull();
    expect(start?.getFullYear()).toBe(2026);
    expect(start?.getMonth()).toBe(3);
    expect(start?.getDate()).toBe(2);
    expect(start?.getHours()).toBe(0);
    expect(end?.getHours()).toBe(23);
    expect(end?.getMinutes()).toBe(59);
    expect(end?.getSeconds()).toBe(59);
    expect(end?.getMilliseconds()).toBe(999);
  });

  it("把单边日期和倒序日期归一成完整区间", () => {
    expect(normalizePmDateRangeForDraft("2026-04-08", null)).toEqual({
      startAt: "2026-04-08",
      endAt: "2026-04-08",
    });
    expect(normalizePmDateRangeForDraft(null, "2026-04-09")).toEqual({
      startAt: "2026-04-09",
      endAt: "2026-04-09",
    });
    expect(normalizePmDateRangeForDraft("2026-04-12", "2026-04-03")).toEqual({
      startAt: "2026-04-03",
      endAt: "2026-04-12",
    });
    expect(normalizePmDateRangeForDraft("invalid", null)).toEqual({
      startAt: null,
      endAt: null,
    });
  });

  it("为日期范围控件提供稳定值", () => {
    expect(getPmDateRangeValue("2026-04-06", "2026-04-08")).toEqual(["2026-04-06", "2026-04-08"]);
    expect(getPmDateRangeValue(null, "2026-04-08")).toEqual(["2026-04-08", "2026-04-08"]);
    expect(getPmDateRangeValue("invalid", null)).toBeNull();
  });

  it("按完整或短格式展示日期与区间", () => {
    expect(formatPmDateForDisplay("2026-04-03T09:00:00.000Z")).toBe("2026-04-03");
    expect(formatPmDateForDisplay("2026-04-03", "short")).toBe("04-03");
    expect(formatPmDateRangeForDisplay("2026-04-03", "2026-04-05")).toBe("2026-04-03 ~ 2026-04-05");
    expect(formatPmDateRangeForDisplay("2026-04-03", null)).toBe("2026-04-03");
    expect(formatPmDateRangeForDisplay(null, null, { emptyText: "" })).toBe("");
  });

  it("只把有合法日期的事项视为已排期", () => {
    expect(hasPmDateSchedule("2026-04-03", null)).toBe(true);
    expect(hasPmDateSchedule("invalid", null)).toBe(false);
    expect(hasPmDateSchedule(null, null)).toBe(false);
  });

  it("逾期判断只看合法截止日期且以当天结束为边界", () => {
    expect(isPmItemOverdue({ endAt: "2026-04-04", status: "in_progress" })).toBe(true);
    expect(isPmItemOverdue({ endAt: "2026-04-05", status: "in_progress" })).toBe(false);
    expect(isPmItemOverdue({ endAt: "invalid", status: "in_progress" })).toBe(false);
    expect(isPmItemOverdue({ endAt: "2026-04-04", status: "done" })).toBe(false);
  });
});
