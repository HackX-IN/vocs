/**
 * SOURCE OF TRUTH KEYWORDS: StatsView, StatsSummary, getStats, hero-stat,
 *   LanguageBreakdown, ActivityChart, LatencyPanel, instrument-grid, numbered-cards
 * WHAT:  The stats view as a panel of numbered instrument cards: time saved
 *        with the activity chart, the streak, three headline counts, the
 *        language breakdown and the latency table.
 * WHY:   One get_stats call fills the whole page — six commands would be six
 *        chances to render half a page, which is why the Rust side returns one
 *        shape. Nothing here computes a statistic: every number is served,
 *        because a frontend that derives "time saved" is a second opinion about
 *        the product's central claim.
 *
 *        The cards are numbered in reading order and the numbers are assigned
 *        here, by position, because only this view knows which cards it shows —
 *        the streak card disappears for a brand-new user and the rest must not
 *        keep a gap. The hero card is the only one with the dot lattice behind
 *        it (docs/04 §1.2): it holds the figure the view exists to show.
 * WHERE: Rendered by Dashboard.tsx for the registry's "stats" route.
 */

import { commands, type HotkeyBinding, type MetricDef } from "@/lib/bindings";
import { useCommand } from "@/lib/ipc";
import { formatCompactDuration, formatCount } from "@/lib/format";
import { Card, ScrollArea, StatCard } from "@/components/global";
import type { DictationMode } from "@/lib/dictation-mode";
import { NoTranscriptionsYet } from "../_components/NoTranscriptionsYet";
import { ViewState } from "../_components/ViewState";
import { ActivityChart, useActivityRange } from "./_components/ActivityChart";
import { LatencyPanel } from "./_components/LatencyPanel";
import { StreakChart } from "./_components/StreakChart";
import { LanguageBreakdown } from "./_components/LanguageBreakdown";

export interface StatsViewProps {
  /** Registry metric declarations — they decide what the latency panel shows. */
  metrics: readonly MetricDef[];
  /** For the zero state, which is the same one History shows. */
  hotkey: HotkeyBinding | null;
  mode: DictationMode;
}

export function StatsView({ metrics, hotkey, mode }: StatsViewProps) {
  const stats = useCommand(commands.getStats, []);
  const { range, picker } = useActivityRange();

  return (
    <ScrollArea centered contentClassName="px-[var(--page-padding-x)] pb-8">
      <ViewState state={stats} onRetry={stats.reload}>
        {(data) =>
          data.total_sessions === 0 ? (
            <NoTranscriptionsYet hotkey={hotkey} mode={mode} />
          ) : (
            <div className="grid grid-cols-3 gap-4">
              <StatCard
                index="01"
                label="Time saved"
                size="hero"
                tone="hero"
                className="col-span-2"
                value={formatCompactDuration(data.time_saved_ms)}
                caption={
                  data.speaking_wpm !== null && data.baseline_typing_wpm !== null
                    ? `${Math.round(data.speaking_wpm)} wpm spoken vs ${Math.round(data.baseline_typing_wpm)} typed`
                    : undefined
                }
                actions={picker}
                chart={<ActivityChart days={data.activity} range={range} />}
              />

              <StatCard
                index="02"
                label="Streak"
                value={formatCount(data.current_streak_days)}
                unit={data.current_streak_days === 1 ? "day" : "days"}
                caption={`${formatCount(data.sessions_this_week)} sessions this week`}
                chart={<StreakChart days={data.activity} />}
              />

              <StatCard index="03" label="Transcriptions" value={formatCount(data.total_sessions)} />
              <StatCard
                index="04"
                label="Words"
                value={formatCount(data.total_words)}
                caption={`${formatCount(data.words_this_week)} this week`}
              />
              <StatCard index="05" label="Time spoken" value={formatCompactDuration(data.total_speaking_ms)} />

              {data.languages.length > 0 ? (
                <Card index="06" title="Languages" className="col-span-1">
                  <LanguageBreakdown languages={data.languages} />
                </Card>
              ) : null}

              <Card
                index={data.languages.length > 0 ? "07" : "06"}
                title="Latency"
                flush
                className={data.languages.length > 0 ? "col-span-2" : "col-span-3"}
              >
                <div className="px-[var(--card-padding)] pt-3 pb-[var(--card-padding)]">
                  <LatencyPanel metrics={metrics} latency={data.latency} />
                </div>
              </Card>
            </div>
          )
        }
      </ViewState>
    </ScrollArea>
  );
}
