export interface CalendarDay {
  date: Date;
  day: number;
  isCurrentMonth: boolean;
  isToday: boolean;
  dateKey: string; // YYYY-MM-DD format for grouping
}

export function getCalendarDays(year: number, month: number): CalendarDay[] {
  const firstDay = new Date(year, month, 1);
  const lastDay = new Date(year, month + 1, 0);

  // Monday=0 .. Sunday=6 (ISO weekday)
  let startWeekday = firstDay.getDay() - 1;
  if (startWeekday < 0) startWeekday = 6;

  const today = new Date();
  const todayKey = formatDateKey(today);

  const days: CalendarDay[] = [];

  // Previous month padding
  for (let i = startWeekday - 1; i >= 0; i--) {
    const d = new Date(year, month, -i);
    days.push(makeDay(d, false, todayKey));
  }

  // Current month
  for (let d = 1; d <= lastDay.getDate(); d++) {
    const date = new Date(year, month, d);
    days.push(makeDay(date, true, todayKey));
  }

  // Next month padding (fill to 6 rows = 42 cells, or at least full last week)
  const remaining = 42 - days.length;
  for (let d = 1; d <= remaining; d++) {
    const date = new Date(year, month + 1, d);
    days.push(makeDay(date, false, todayKey));
  }

  return days;
}

function makeDay(date: Date, isCurrentMonth: boolean, todayKey: string): CalendarDay {
  const dateKey = formatDateKey(date);
  return {
    date,
    day: date.getDate(),
    isCurrentMonth,
    isToday: dateKey === todayKey,
    dateKey,
  };
}

export function formatDateKey(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function prevMonth(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth() - 1, 1);
}

export function nextMonth(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth() + 1, 1);
}

export function formatMonthLabel(date: Date): string {
  return `${date.getFullYear()}年${date.getMonth() + 1}月`;
}
