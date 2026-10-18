/**
 * SOURCE OF TRUTH KEYWORDS: calendarWindow, fillCalendar, localDateKey, CalendarDay,
 *   sparse-activity, zero-filled-days
 * WHAT:  Turns the backend's sparse per-day activity (only days that had a
 *        session) into a continuous run of calendar days ending today, with
 *        zeros for the days nothing happened.
 * WHY:   A chart drawn from the sparse rows puts Monday next to Friday and
 *        calls it a streak — the old line chart did exactly that. A habit is a
 *        shape in TIME, so the quiet days have to be there. This is presentation
 *        only (the backend still reports only measured days), and it works in
 *        LOCAL dates to match how services/stats.rs buckets them: a UTC key here
 *        would shift an evening session onto the wrong column for anyone west
 *        of Greenwich. Dates are stepped with setDate rather than by adding
 *        86 400 000 ms, which is wrong across a daylight-saving change.
 * WHERE: The Stats view's activity chart and streak card.
 */

import type { ActivityDay } from "@/lib/bindings";

export interface CalendarDay {
  /** Local date as YYYY-MM-DD. */
  date: string;
  session_count: number;
  word_count: number;
}

/** A Date as the local YYYY-MM-DD the backend uses. */
export function localDateKey(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

/**
 * `length` consecutive days ending on `today`, or — when `length` is null —
 * from the first recorded day through today. Never fewer than two days.
 */
export function fillCalendar(
  days: readonly ActivityDay[],
  length: number | null,
  today: Date = new Date(),
): CalendarDay[] {
  const byDate = new Map(days.map((day) => [day.date, day]));
  const end = new Date(today.getFullYear(), today.getMonth(), today.getDate());

  let start: Date;
  if (length === null) {
    const first = days[0]?.date;
    start = first ? new Date(`${first}T00:00:00`) : new Date(end);
  } else {
    start = new Date(end);
    start.setDate(start.getDate() - (length - 1));
  }

  const out: CalendarDay[] = [];
  for (const cursor = new Date(start); cursor <= end; cursor.setDate(cursor.getDate() + 1)) {
    const key = localDateKey(cursor);
    out.push({ date: key, session_count: byDate.get(key)?.session_count ?? 0, word_count: byDate.get(key)?.word_count ?? 0 });
  }
  return out.length >= 2 ? out : [{ date: localDateKey(start), session_count: 0, word_count: 0 }, ...out];
}
