/**
 * SOURCE OF TRUTH KEYWORDS: HistorySummary, summary-strip, dictations-summary, getStats,
 *   dashed-divider, font-display
 * WHAT:  The strip at the top of the Dictations list: three headline figures —
 *        words this week, the streak, time saved — in the display face, divided
 *        by dashed rules.
 * WHY:   The list opens on a reason to keep going, not on a wall of rows. The
 *        figures are the same ones the Stats page serves (one get_stats call,
 *        nothing derived here) so the two pages can never disagree, and the
 *        strip is a single short card rather than three so it takes one row of
 *        the window and leaves the rest to the dictations. It hides itself for a
 *        brand-new user: three zeros are not encouragement, they are a form —
 *        and that decision is the CALLER's, because the caller also has to give
 *        the list the strip's height, and the two must agree.
 *        The dashed dividers are the instrument-panel detail of docs/04 §1.
 * WHERE: The `lead` slot of the Dictations DataList, so it scrolls away with it.
 */

import type { StatsSummary } from "@/lib/bindings";
import { formatCompactDuration, formatCount } from "@/lib/format";
import { Card, Eyebrow } from "@/components/global";

function Figure({ label, value, unit }: { label: string; value: string; unit?: string }) {
  return (
    <div className="flex min-w-0 flex-1 flex-col gap-1 px-[var(--card-padding)] first:pl-0 last:pr-0">
      <Eyebrow>{label}</Eyebrow>
      <p className="flex items-baseline gap-2">
        <span className="font-display text-title tabular-nums text-text-primary">{value}</span>
        {unit ? <span className="text-label text-text-tertiary">{unit}</span> : null}
      </p>
    </div>
  );
}

export function HistorySummary({ data }: { data: StatsSummary }) {
  return (
    <Card className="mx-auto w-full max-w-[var(--content-max-width)]">
      <div className="flex divide-x divide-dashed divide-hairline">
        <Figure label="This week" value={formatCount(data.words_this_week)} unit="words" />
        <Figure label="Streak" value={formatCount(data.current_streak_days)} unit={data.current_streak_days === 1 ? "day" : "days"} />
        <Figure label="Time saved" value={formatCompactDuration(data.time_saved_ms)} />
      </div>
    </Card>
  );
}
