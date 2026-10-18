/**
 * SOURCE OF TRUTH KEYWORDS: groupByDay, HistoryEntry, day-groups, day-header,
 *   flat-list-with-headers
 * WHAT:  Turns the newest-first session feed into one flat list of entries — a
 *        header for each day, then that day's sessions.
 * WHY:   FLAT, because the list is virtualised on a uniform row height: a header
 *        is a row like any other (the list steps over it with `isSelectable`),
 *        and nesting would make the visible range a tree walk instead of
 *        arithmetic. Grouping is by LOCAL calendar day, the same rule the stats
 *        use, and consecutive — the feed is already sorted, so one pass is
 *        enough and a day that appears twice (it cannot) would simply get two
 *        headers rather than hiding rows.
 *
 *        Each header carries the day's count so "Today · 3" is free; the count
 *        reflects the sessions LOADED so far, which for a paginated feed is
 *        right for every day but possibly the oldest one on screen.
 * WHERE: HistoryView, which hands the result to DataList.
 */

import { dayStart, formatDayHeading } from "@/lib/format";
import type { SessionSummary } from "@/lib/bindings";

export type HistoryEntry =
  | { kind: "header"; key: string; label: string; count: number }
  | { kind: "session"; key: string; session: SessionSummary };

export function groupByDay(sessions: readonly SessionSummary[], now: number = Date.now()): HistoryEntry[] {
  const entries: HistoryEntry[] = [];
  let currentDay: number | null = null;
  let header: Extract<HistoryEntry, { kind: "header" }> | null = null;

  for (const session of sessions) {
    const day = dayStart(session.started_at_ms);
    if (day !== currentDay) {
      currentDay = day;
      header = { kind: "header", key: `day-${day}`, label: formatDayHeading(session.started_at_ms, now), count: 0 };
      entries.push(header);
    }
    if (header) header.count += 1;
    entries.push({ kind: "session", key: session.id, session });
  }
  return entries;
}
