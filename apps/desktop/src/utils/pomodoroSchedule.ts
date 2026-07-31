export type PomodoroPhaseKind = "idle" | "focus" | "break" | "paused" | "done";

export interface PomodoroConfig {
  enabled: boolean;
  workdayStart: string;
  workdayEnd: string;
  lunchStart: string;
  lunchEnd: string;
  focusMinutes: number;
  shortBreakMinutes: number;
  weekdays: number[];
}

export interface PomodoroPhase {
  kind: PomodoroPhaseKind;
  label: string;
  cycleIndex: number;
  remainingSeconds: number;
  endsAt: string | null;
}

interface DaySegments {
  workStart: Date;
  lunchStart: Date;
  lunchEnd: Date;
  workEnd: Date;
}

export const DEFAULT_POMODORO_CONFIG: PomodoroConfig = {
  enabled: true,
  workdayStart: "08:00",
  workdayEnd: "17:00",
  lunchStart: "12:00",
  lunchEnd: "13:30",
  focusMinutes: 25,
  shortBreakMinutes: 5,
  weekdays: [1, 2, 3, 4, 5],
};

const SECONDS_PER_MINUTE = 60;

export function getPomodoroPhase(
  config: PomodoroConfig,
  sessionStartedAt: string | null | undefined,
  now: Date = new Date(),
): PomodoroPhase {
  const startedAt = sessionStartedAt ? new Date(sessionStartedAt) : null;
  if (!startedAt || Number.isNaN(startedAt.getTime())) {
    return phase("idle", "未开始", 0, 0, null);
  }

  const segments = getDaySegments(config, now);
  if (now >= segments.workEnd) {
    return phase("done", "今日已结束", 0, 0, segments.workEnd);
  }
  if (now < segments.workStart) {
    return phase(
      "idle",
      "等待开始",
      0,
      secondsBetween(now, segments.workStart),
      segments.workStart,
    );
  }
  if (now >= segments.lunchStart && now < segments.lunchEnd) {
    return phase("paused", "午休中", 0, secondsBetween(now, segments.lunchEnd), segments.lunchEnd);
  }

  const activeElapsedSeconds = getActiveElapsedSeconds(config, startedAt, now);
  const focusSeconds = config.focusMinutes * SECONDS_PER_MINUTE;
  const breakSeconds = config.shortBreakMinutes * SECONDS_PER_MINUTE;
  const cycleSeconds = focusSeconds + breakSeconds;

  if (cycleSeconds <= 0 || focusSeconds <= 0 || breakSeconds <= 0) {
    return phase("idle", "配置无效", 0, 0, null);
  }

  const cycleIndex = Math.floor(activeElapsedSeconds / cycleSeconds) + 1;
  const cycleOffset = activeElapsedSeconds % cycleSeconds;
  if (cycleOffset < focusSeconds) {
    const remainingSeconds = focusSeconds - cycleOffset;
    return phase(
      "focus",
      "专注中",
      cycleIndex,
      remainingSeconds,
      addActiveSeconds(config, now, remainingSeconds),
    );
  }

  const remainingSeconds = cycleSeconds - cycleOffset;
  return phase(
    "break",
    "休息中",
    cycleIndex,
    remainingSeconds,
    addActiveSeconds(config, now, remainingSeconds),
  );
}

export function formatPomodoroDuration(totalSeconds: number): string {
  const safeSeconds = Math.max(0, Math.floor(totalSeconds));
  const minutes = Math.floor(safeSeconds / SECONDS_PER_MINUTE);
  const seconds = safeSeconds % SECONDS_PER_MINUTE;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

function phase(
  kind: PomodoroPhaseKind,
  label: string,
  cycleIndex: number,
  remainingSeconds: number,
  endsAt: Date | null,
): PomodoroPhase {
  return {
    kind,
    label,
    cycleIndex,
    remainingSeconds,
    endsAt: endsAt ? endsAt.toISOString() : null,
  };
}

function getActiveElapsedSeconds(config: PomodoroConfig, startedAt: Date, now: Date): number {
  if (now <= startedAt) return 0;
  const segments = getDaySegments(config, now);
  return (
    overlapSeconds(startedAt, now, maxDate(startedAt, segments.workStart), segments.lunchStart) +
    overlapSeconds(startedAt, now, segments.lunchEnd, segments.workEnd)
  );
}

function addActiveSeconds(config: PomodoroConfig, from: Date, seconds: number): Date {
  const segments = getDaySegments(config, from);
  let cursor = new Date(from);
  let remaining = Math.max(0, seconds);

  if (cursor < segments.workStart) {
    cursor = new Date(segments.workStart);
  }
  if (cursor >= segments.lunchStart && cursor < segments.lunchEnd) {
    cursor = new Date(segments.lunchEnd);
  }

  for (const [start, end] of [
    [segments.workStart, segments.lunchStart],
    [segments.lunchEnd, segments.workEnd],
  ] as const) {
    if (cursor >= end) continue;
    if (cursor < start) cursor = new Date(start);
    const available = secondsBetween(cursor, end);
    if (remaining <= available) {
      return new Date(cursor.getTime() + remaining * 1000);
    }
    remaining -= available;
    cursor = new Date(end);
  }

  return segments.workEnd;
}

function overlapSeconds(
  rangeStart: Date,
  rangeEnd: Date,
  segmentStart: Date,
  segmentEnd: Date,
): number {
  const start = maxDate(rangeStart, segmentStart);
  const end = minDate(rangeEnd, segmentEnd);
  if (end <= start) return 0;
  return secondsBetween(start, end);
}

function getDaySegments(config: PomodoroConfig, date: Date): DaySegments {
  return {
    workStart: timeOnDate(date, config.workdayStart),
    workEnd: timeOnDate(date, config.workdayEnd),
    lunchStart: timeOnDate(date, config.lunchStart),
    lunchEnd: timeOnDate(date, config.lunchEnd),
  };
}

function timeOnDate(date: Date, hhmm: string): Date {
  const [hourRaw, minuteRaw] = hhmm.split(":");
  const hour = Number(hourRaw);
  const minute = Number(minuteRaw);
  return new Date(
    date.getFullYear(),
    date.getMonth(),
    date.getDate(),
    Number.isFinite(hour) ? hour : 0,
    Number.isFinite(minute) ? minute : 0,
    0,
    0,
  );
}

function secondsBetween(start: Date, end: Date): number {
  return Math.max(0, Math.floor((end.getTime() - start.getTime()) / 1000));
}

function maxDate(a: Date, b: Date): Date {
  return a > b ? a : b;
}

function minDate(a: Date, b: Date): Date {
  return a < b ? a : b;
}
