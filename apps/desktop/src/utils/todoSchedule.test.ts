import { describe, expect, it } from "vitest";
import type { TodoRecurrence } from "../types";
import {
  buildSimpleRuleFromPreset,
  combineLocalDateTime,
  deriveRepeatPreset,
  getCreateDraftDefaultDateTime,
  isFiveMinuteDateTime,
  isFiveMinuteTime,
  splitDateTime,
} from "./todoSchedule";

function createRecurrence(overrides: Partial<TodoRecurrence> = {}): TodoRecurrence {
  return {
    startAt: "2026-03-07T01:30:00.000Z",
    ruleMode: "simple",
    rule: { frequency: "daily", interval: 1, time: "09:30" },
    cronExpression: "0 30 9 * * *",
    timezone: "local",
    endMode: "never",
    endValue: null,
    occurrenceIndex: 0,
    active: true,
    ...overrides,
  };
}

describe("todoSchedule", () => {
  it("round-trips local date and time", () => {
    const value = combineLocalDateTime("2026-03-07", "09:30");

    expect(value).toBeTruthy();
    expect(splitDateTime(value).date).toBe("2026-03-07");
    expect(splitDateTime(value).time).toBe("09:30");
  });

  it("validates five-minute granularity", () => {
    expect(isFiveMinuteTime("09:30")).toBe(true);
    expect(isFiveMinuteTime("09:33")).toBe(false);
    expect(isFiveMinuteDateTime("2026-03-07T09:30:00.000Z")).toBe(true);
    expect(isFiveMinuteDateTime("2026-03-07T09:33:00.000Z")).toBe(false);
  });

  it("builds create-dialog defaults for common times", () => {
    expect(getCreateDraftDefaultDateTime(new Date(2026, 2, 7, 14, 23, 0, 0))).toEqual({
      date: "2026-03-08",
      time: "14:25",
    });
    expect(getCreateDraftDefaultDateTime(new Date(2026, 2, 7, 14, 55, 0, 0))).toEqual({
      date: "2026-03-08",
      time: "15:00",
    });
  });

  it("builds create-dialog defaults across midnight", () => {
    expect(getCreateDraftDefaultDateTime(new Date(2026, 2, 7, 23, 57, 0, 0))).toEqual({
      date: "2026-03-08",
      time: "00:00",
    });
    expect(getCreateDraftDefaultDateTime(new Date(2026, 2, 7, 14, 25, 1, 0))).toEqual({
      date: "2026-03-08",
      time: "14:30",
    });
  });

  it("derives common repeat presets from recurrence", () => {
    expect(deriveRepeatPreset(createRecurrence())).toBe("daily");
    expect(
      deriveRepeatPreset(
        createRecurrence({
          rule: { frequency: "weekly", interval: 1, time: "09:30", weekdays: [1, 2, 3, 4, 5] },
        }),
      ),
    ).toBe("workday");
    expect(
      deriveRepeatPreset(
        createRecurrence({
          rule: { frequency: "weekly", interval: 1, time: "09:30", weekdays: [2, 4] },
        }),
      ),
    ).toBe("weekly");
    expect(
      deriveRepeatPreset(
        createRecurrence({
          rule: { frequency: "monthly", interval: 1, time: "09:30", dayOfMonth: 31 },
        }),
      ),
    ).toBe("monthly");
    expect(
      deriveRepeatPreset(
        createRecurrence({
          rule: { frequency: "weekly", interval: 2, time: "09:30", weekdays: [2] },
        }),
      ),
    ).toBe("custom");
    expect(
      deriveRepeatPreset(
        createRecurrence({
          ruleMode: "cron",
          rule: { expression: "0 30 9 */2 * *" },
          cronExpression: "0 30 9 */2 * *",
        }),
      ),
    ).toBe("cron");
  });

  it("builds weekly and monthly defaults from the selected start date", () => {
    expect(
      buildSimpleRuleFromPreset({
        preset: "weekly",
        startDate: "2026-03-07",
        time: "09:30",
      }),
    ).toEqual({
      frequency: "weekly",
      interval: 1,
      time: "09:30",
      weekdays: [6],
    });

    expect(
      buildSimpleRuleFromPreset({
        preset: "monthly",
        startDate: "2026-03-31",
        time: "09:30",
      }),
    ).toEqual({
      frequency: "monthly",
      interval: 1,
      time: "09:30",
      dayOfMonth: 31,
    });
  });

  it("builds the workday shortcut as a weekly rule", () => {
    expect(
      buildSimpleRuleFromPreset({
        preset: "workday",
        startDate: "2026-03-07",
        time: "09:30",
      }),
    ).toEqual({
      frequency: "weekly",
      interval: 1,
      time: "09:30",
      weekdays: [1, 2, 3, 4, 5],
    });
  });
});
