/**
 * SOURCE OF TRUTH KEYWORDS: HistoryFilters, history-filter-bar, period-picker, status-picker
 * WHAT:  The two segmented pickers above the Dictations list: how far back, and
 *        which outcome.
 * WHY:   Both are the axes the user is reasoning along and each has three or four
 *        values, so all of them stay visible (SegmentedControl) rather than being
 *        folded into menus. The filters narrow the list on the server, so they
 *        compose with search and with paging.
 * WHERE: The `lead` of the Dictations list, and again in its no-results state so
 *        a filter that matched nothing can always be undone.
 */

import type { HistoryStatus } from "@/lib/bindings";
import { SegmentedControl } from "@/components/global";
import {
  PERIOD_OPTIONS,
  STATUS_OPTIONS,
  type HistoryPeriod,
} from "../history-filter";

export interface HistoryFiltersProps {
  period: HistoryPeriod;
  status: HistoryStatus;
  onPeriodChange: (period: HistoryPeriod) => void;
  onStatusChange: (status: HistoryStatus) => void;
}

export function HistoryFilters({
  period,
  status,
  onPeriodChange,
  onStatusChange,
}: HistoryFiltersProps) {
  return (
    <div className="mx-auto flex w-full max-w-[var(--content-max-width)] flex-wrap items-center justify-between gap-3 px-3 py-2 mt-4">
      <SegmentedControl
        label="Period"
        options={PERIOD_OPTIONS}
        value={period}
        onChange={onPeriodChange}
      />
      <SegmentedControl
        label="Status"
        options={STATUS_OPTIONS}
        value={status}
        onChange={onStatusChange}
      />
    </div>
  );
}
