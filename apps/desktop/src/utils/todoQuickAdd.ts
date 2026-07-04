import type { TodoPriority } from "../types";
import { combineLocalDateTime, DEFAULT_TIME } from "./todoSchedule";

export type QuickAddDateChoice =
  | { kind: "today" }
  | { kind: "tomorrow" }
  | { kind: "date"; date: string }
  | null;

export interface QuickAddInput {
  title: string;
  dateChoice: QuickAddDateChoice;
  priorityOverride: TodoPriority | null;
}

export interface QuickAddContext {
  typeId: number | null;
  projectId: number | null;
  priorityDefault: TodoPriority;
}

function pad(value: number) {
  return String(value).padStart(2, "0");
}

function toLocalDateString(date: Date) {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function resolveEventAt(choice: QuickAddDateChoice, now: Date): string | null {
  if (!choice) return null;
  if (choice.kind === "today") {
    // 分钟溢出交给 Date 构造器进位，23:58 自然跨到次日 00:00
    const next = new Date(
      now.getFullYear(),
      now.getMonth(),
      now.getDate(),
      now.getHours(),
      Math.floor(now.getMinutes() / 5) * 5 + 5,
    );
    return next.toISOString();
  }
  if (choice.kind === "tomorrow") {
    const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
    return combineLocalDateTime(toLocalDateString(tomorrow), DEFAULT_TIME);
  }
  return combineLocalDateTime(choice.date, DEFAULT_TIME);
}

export function buildQuickAddPayload(
  input: QuickAddInput,
  context: QuickAddContext,
  now = new Date(),
): Record<string, unknown> | null {
  const title = input.title.trim();
  if (!title) return null;

  const payload: Record<string, unknown> = {
    title,
    priority: input.priorityOverride ?? context.priorityDefault,
    reminderPresets: ["none"],
  };
  if (context.typeId !== null) payload.typeId = context.typeId;
  if (context.projectId !== null) payload.projectId = context.projectId;

  const eventAt = resolveEventAt(input.dateChoice, now);
  if (eventAt) payload.eventAt = eventAt;

  return payload;
}
