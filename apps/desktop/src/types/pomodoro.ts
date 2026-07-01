import type { PomodoroConfig } from "../utils/pomodoroSchedule";

export type PomodoroSessionStatus = "prompted" | "running" | "skipped" | "stopped" | "completed";

export interface PomodoroSession {
  date: string;
  status: PomodoroSessionStatus;
  startedAt?: string | null;
  stoppedAt?: string | null;
  promptedAt?: string | null;
}

export interface PomodoroState {
  config: PomodoroConfig;
  session?: PomodoroSession | null;
  now: string;
}
