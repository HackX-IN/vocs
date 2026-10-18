/**
 * SOURCE OF TRUTH KEYWORDS: HistoryPeriod, HistoryStatus, periodStart, PERIOD_OPTIONS,
 *   STATUS_OPTIONS, dictation_filters, local_midnight
 * WHAT:  The two filters on the Dictations list — how far back, and which
 *        outcome — with the labels they are shown under and the instant a
 *        period starts.
 * WHY:   "Today" and "this week" begin at the USER's midnight, in the user's time
 *        zone, which only the webview knows, so the period is turned into a start
 *        instant here and the backend filters on that number. The week is the
 *        last seven days rather than the calendar week, which would make
 *        Monday morning's "this week" an empty list.
 * WHERE: HistoryView (state), HistoryFilters (controls), use-history (request).
 */

import type { HistoryStatus } from "@/lib/bindings";
import type { SegmentOption } from "@/components/global";

export type HistoryPeriod = "all" | "today" | "week" | "month";

export const PERIOD_OPTIONS: readonly SegmentOption<HistoryPeriod>[] = [
  { value: "all", label: "All time" },
  { value: "today", label: "Today" },
  { value: "week", label: "7 days" },
  { value: "month", label: "30 days" },
];

export const STATUS_OPTIONS: readonly SegmentOption<HistoryStatus>[] = [
  { value: "ALL", label: "All" },
  { value: "EDITED", label: "Edited" },
  { value: "FAILED", label: "Failed" },
];

const DAY_MS = 24 * 60 * 60 * 1000;

/** The instant a period starts, or null for no lower bound. */
export function periodStart(period: HistoryPeriod, now: Date = new Date()): number | null {
  const midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  switch (period) {
    case "all":
      return null;
    case "today":
      return midnight;
    case "week":
      return midnight - 6 * DAY_MS;
    case "month":
      return midnight - 29 * DAY_MS;
  }
}
