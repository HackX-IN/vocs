/**
 * SOURCE OF TRUTH KEYWORDS: StreakChart, streak_week, last_seven_days, DayReadout
 * WHAT:  The last seven days of sessions as a small dot-column chart, each column
 *        readable on hover.
 * WHY:   The streak card shows the shape of the week, not just the count, and the
 *        calendar window is filled in here so a quiet day is a dim column rather
 *        than a missing one. Kept out of StatsView so that file stays about the
 *        page's layout.
 * WHERE: The Stats view's streak card.
 */

import { useMemo } from "react";
import { DotColumns } from "@/components/global";
import { fillCalendar } from "@/lib/calendar-window";
import type { ActivityDay } from "@/lib/bindings";
import { DayReadout } from "./ActivityChart";

/** The streak card draws the last week, which is a calendar fact, not a design value. */
const STREAK_DAYS = 7;

export function StreakChart({ days }: { days: readonly ActivityDay[] }) {
  const week = useMemo(() => fillCalendar(days, STREAK_DAYS), [days]);
  return (
    <DotColumns
      values={week.map((day) => day.session_count)}
      label="Sessions in the last seven days"
      tooltip={(index) => <DayReadout day={week[index]} />}
    />
  );
}
