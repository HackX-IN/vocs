/**
 * SOURCE OF TRUTH KEYWORDS: LanguageBreakdown, LanguageCount, DotMeter,
 *   no-chart-chrome, tabular-nums
 * WHAT:  The language breakdown: a label, a dot meter, and its count, one row
 *        each.
 * WHY:   No chart library. Once the doctrine in docs/04 §11 is applied what is
 *        left of a horizontal bar chart is a row of dots with a share lit, and
 *        the shared DotMeter draws that. The count sits at the end of its own
 *        row so it is never hidden; the tooltip adds what the row cannot hold —
 *        the share of all dictations.
 * WHERE: The Stats view's languages card. Consumes StatsSummary.languages.
 */

import { DotMeter, Eyebrow } from "@/components/global";
import { formatCount, formatLanguage } from "@/lib/format";
import type { LanguageCount } from "@/lib/bindings";

/** Enough rows to be informative; the tail is noise in a language breakdown. */
const MAX_ROWS = 5;

export function LanguageBreakdown({ languages }: { languages: readonly LanguageCount[] }) {
  const rows = [...languages].sort((a, b) => b.session_count - a.session_count).slice(0, MAX_ROWS);
  if (rows.length === 0) return null;

  const busiest = Math.max(...rows.map((row) => row.session_count), 1);
  const total = languages.reduce((sum, row) => sum + row.session_count, 0);

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
            tooltip={
              <div className="flex flex-col gap-0.5">
                <Eyebrow>{formatLanguage(row.language)}</Eyebrow>
                <p className="text-label tabular-nums text-text-primary">
                  {formatCount(row.session_count)} {row.session_count === 1 ? "dictation" : "dictations"}
                </p>
                <p className="text-caption tabular-nums text-text-secondary">
                  {Math.round((row.session_count / Math.max(total, 1)) * 100)}% of all
                </p>
              </div>
            }
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
