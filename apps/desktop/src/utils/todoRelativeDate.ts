const WEEKDAY_LABELS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"] as const;

function pad2(value: number) {
  return String(value).padStart(2, "0");
}

function parseTodoDateTime(value: string): Date | null {
  const trimmed = value.trim();
  if (!trimmed) return null;

  const dateOnlyMatch = /^(\d{4})-(\d{2})-(\d{2})$/.exec(trimmed);
  if (dateOnlyMatch) {
    const [, yearText, monthText, dayText] = dateOnlyMatch;
    return new Date(Number(yearText), Number(monthText) - 1, Number(dayText));
  }

  const date = new Date(trimmed);
  return Number.isNaN(date.getTime()) ? null : date;
}

function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

function isSameDay(left: Date, right: Date): boolean {
  return left.getTime() === right.getTime();
}

function getWeekStart(date: Date): Date {
  const day = date.getDay();
  const weekStart = startOfDay(date);
  weekStart.setDate(weekStart.getDate() - (day === 0 ? 6 : day - 1));
  return weekStart;
}

function isSameWeek(leftDay: Date, rightDay: Date): boolean {
  return isSameDay(getWeekStart(leftDay), getWeekStart(rightDay));
}

function formatTime(date: Date): string {
  return `${pad2(date.getHours())}:${pad2(date.getMinutes())}`;
}

function formatDateFallback(date: Date, now: Date): string {
  const month = date.getMonth() + 1;
  const day = date.getDate();
  const time = formatTime(date);
  if (date.getFullYear() === now.getFullYear()) {
    return `${month}月${day}日 ${time}`;
  }
  return `${date.getFullYear()}/${pad2(month)}/${pad2(day)} ${time}`;
}

export function formatTodoRelativeDateTimeLabel(
  time?: string | null,
  now: Date = new Date(),
): string {
  if (!time) return "";

  const date = parseTodoDateTime(time);
  if (!date) return "";

  const today = startOfDay(now);
  const itemDay = startOfDay(date);
  const tomorrow = addDays(today, 1);
  const yesterday = addDays(today, -1);
  const timeLabel = formatTime(date);

  if (isSameDay(itemDay, today)) return `今天 ${timeLabel}`;
  if (isSameDay(itemDay, tomorrow)) return `明天 ${timeLabel}`;
  if (isSameDay(itemDay, yesterday)) return `昨天 ${timeLabel}`;

  const weekdayLabel = WEEKDAY_LABELS[date.getDay()];
  const currentWeekStart = getWeekStart(today);
  const nextWeekStart = addDays(currentWeekStart, 7);
  const previousWeekStart = addDays(currentWeekStart, -7);

  if (isSameWeek(itemDay, today)) return `${weekdayLabel} ${timeLabel}`;
  if (isSameWeek(itemDay, nextWeekStart)) return `下${weekdayLabel} ${timeLabel}`;
  if (isSameWeek(itemDay, previousWeekStart)) return `上${weekdayLabel} ${timeLabel}`;

  return formatDateFallback(date, now);
}
