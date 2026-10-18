/**
 * SOURCE OF TRUTH KEYWORDS: ActivityChart, ActivityRange, activity-dots, no-chart-chrome,
 *   DotColumns, segmented-range
 * WHAT:  Sessions per day as a dot-matrix column chart, with a quiet date label
 *        at each end and a 30d / 90d / All range picker.
 * WHY:   The shape of the habit is the one thing the hero number cannot show —
 *        which is why it is drawn over the real calendar, zero days included,
 *        and not over the sparse rows the backend sends.
 *        It is drawn by the shared DotColumns primitive, so it inherits every
 *        chart rule of docs/04 §11 — no axes, gridlines, frame, legend, load
 *        animation or chart library — and this file holds only what is
 *        specific to activity: the range windows and the date labels.
 *
 *        Counts are plotted raw. A trailing mean would draw a prettier shape
 *        and would be a number this app did not measure.
 *
 *        The range is chosen with a segmented control, never a dropdown: the
 *        options are three, mutually exclusive, and a select would hide two of
 *        them behind a click.
 * WHERE: The Stats view's hero card. Consumes StatsSummary.activity.
 */

import { useMemo, useState } from "react";
import { DotColumns, Eyebrow, SegmentedControl, type SegmentOption } from "@/components/global";
import { formatChartDate, formatCount } from "@/lib/format";
import { fillCalendar, type CalendarDay } from "@/lib/calendar-window";
import type { ActivityDay } from "@/lib/bindings";

export type ActivityRange = "30" | "90" | "all";

/** Data windows, not design values — days of history, so they live here. */
const RANGE_DAYS: Readonly<Record<ActivityRange, number | null>> = {
  "30": 30,
  "90": 90,
  all: null,
};

const RANGE_OPTIONS: readonly SegmentOption<ActivityRange>[] = [
  { value: "30", label: "30d" },
  { value: "90", label: "90d" },
  { value: "all", label: "All" },
];

/** The range picker is rendered by the caller (it lives in the card header). */
export function useActivityRange() {
  const [range, setRange] = useState<ActivityRange>("30");
  const picker = <SegmentedControl label="Range" options={RANGE_OPTIONS} value={range} onChange={setRange} />;
  return { range, picker };
}

/** The tooltip body for one day: its date, then what was done on it. */
export function DayReadout({ day }: { day: CalendarDay | undefined }) {
  if (!day) return null;
  return (
    <div className="flex flex-col gap-0.5">
      <Eyebrow>{formatChartDate(day.date)}</Eyebrow>
      <p className="text-label tabular-nums text-text-primary">
        {day.session_count === 0
          ? "No dictations"
          : `${formatCount(day.session_count)} ${day.session_count === 1 ? "dictation" : "dictations"}`}
      </p>
      {day.word_count > 0 ? (
        <p className="text-caption tabular-nums text-text-secondary">{formatCount(day.word_count)} words</p>
      ) : null}
    </div>
  );
}

export function ActivityChart({ days, range }: { days: readonly ActivityDay[]; range: ActivityRange }) {
  // The backend sends only the days that had a session; the chart needs every
  // day in the window, quiet ones included.
  const visible = useMemo(() => fillCalendar(days, RANGE_DAYS[range]), [days, range]);

  const first = formatChartDate(visible[0].date);
  const last = formatChartDate(visible[visible.length - 1].date);

  return (
    <div className="flex flex-col gap-2">
      <DotColumns
        values={visible.map((day) => day.session_count)}
        label={`Sessions per day, ${first} to ${last}`}
        tooltip={(index) => <DayReadout day={visible[index]} />}
      />
      <div className="flex items-center justify-between text-caption tabular-nums text-text-tertiary">
        <span>{first}</span>
        <span>{last}</span>
      </div>
    </div>
  );
}
