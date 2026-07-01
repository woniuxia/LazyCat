import { describe, expect, it } from "vitest";
import { DEFAULT_POMODORO_CONFIG, getPomodoroPhase } from "./pomodoroSchedule";

describe("pomodoroSchedule", () => {
  it("skips the lunch window when calculating the current phase", () => {
    const sessionStartedAt = "2026-06-29T08:00:00+08:00";

    expect(getPomodoroPhase(DEFAULT_POMODORO_CONFIG, sessionStartedAt, new Date("2026-06-29T08:10:00+08:00"))).toMatchObject({
      kind: "focus",
      cycleIndex: 1,
      remainingSeconds: 15 * 60,
    });

    expect(getPomodoroPhase(DEFAULT_POMODORO_CONFIG, sessionStartedAt, new Date("2026-06-29T12:15:00+08:00"))).toMatchObject({
      kind: "paused",
      remainingSeconds: 75 * 60,
    });

    expect(getPomodoroPhase(DEFAULT_POMODORO_CONFIG, sessionStartedAt, new Date("2026-06-29T13:35:00+08:00"))).toMatchObject({
      kind: "focus",
      cycleIndex: 9,
      remainingSeconds: 20 * 60,
    });
  });

  it("ends the running day at the configured workday end time", () => {
    const sessionStartedAt = "2026-06-29T08:00:00+08:00";

    expect(getPomodoroPhase(DEFAULT_POMODORO_CONFIG, sessionStartedAt, new Date("2026-06-29T17:01:00+08:00"))).toMatchObject({
      kind: "done",
      remainingSeconds: 0,
    });
  });
});
