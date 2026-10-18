/**
 * SOURCE OF TRUTH KEYWORDS: LanguageBreakdown, LanguageCount, DotMeter,
 *   no-chart-chrome, tabular-nums
 * WHAT:  The language breakdown: a label, a dot meter, and its count, one row
 *        each.
 * WHY:   No chart library. Once the doctrine in docs/04 §11 is applied what is
 *        left of a horizontal bar chart is a row of dots with a share lit, and
 *        the shared DotMeter draws that. The value sits at the end of its own
 *        row rather than inside a hover tooltip: a tooltip hides the number
 *        until you go looking for it, in a panel whose whole job is showing
 *        numbers.
 * WHERE: The Stats view's languages card. Consumes StatsSummary.languages.
 */

import { DotMeter } from "@/components/global";
import { formatCount, formatLanguage } from "@/lib/format";
import type { LanguageCount } from "@/lib/bindings";

/** Enough rows to be informative; the tail is noise in a language breakdown. */
const MAX_ROWS = 5;

export function LanguageBreakdown({ languages }: { languages: readonly LanguageCount[] }) {
  const rows = [...languages].sort((a, b) => b.session_count - a.session_count).slice(0, MAX_ROWS);
  if (rows.length === 0) return null;

  const busiest = Math.max(...rows.map((row) => row.session_count), 1);

  return (
    <ul className="flex flex-col">
      {rows.map((row) => (
        <li key={row.language} className="flex h-[var(--chart-row-height)] items-center gap-3">
          <span className="w-[var(--chart-label-gutter)] shrink-0 truncate text-label text-text-secondary">
            {formatLanguage(row.language)}
          </span>
          <DotMeter
            value={row.session_count / busiest}
            label={`${formatLanguage(row.language)}: ${row.session_count} sessions`}
            className="min-w-0 flex-1"
          />
          <span className="w-10 shrink-0 text-right text-label tabular-nums text-text-tertiary">
            {formatCount(row.session_count)}
          </span>
        </li>
      ))}
    </ul>
  );
}
