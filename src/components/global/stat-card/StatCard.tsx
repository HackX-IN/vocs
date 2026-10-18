/**
 * SOURCE OF TRUTH KEYWORDS: StatCard, StatCardProps, StatCardSize, display-figure,
 *   numbered-stat, font-display, tabular-nums
 * WHAT:  One measurement as a card: a numbered eyebrow, the figure in the
 *        display face, an optional unit, a one-line caption and a slot for a
 *        chart beneath it.
 * WHY:   The figure is the loudest thing on the screen on purpose — the
 *        product's whole claim is a measurement — and it is the one place the
 *        dot-matrix face appears besides the page title. It is tabular-nums
 *        because stats update live and a number that reflows as its digits
 *        change reads as cheap. The caption is the sentence explaining where
 *        the number came from ("67 wpm spoken vs 40 typed"), so it is text, not
 *        a coloured delta: whether a figure is good is for the reader to judge,
 *        and the app has no business tinting arithmetic green or red.
 *        Built on Card so the surface, radius and eyebrow stay one definition.
 * WHERE: The Stats view.
 */

import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import { Card, type CardTone } from "../card";

export type StatCardSize = "default" | "hero";

export interface StatCardProps {
  /** "01" — the card's place in its view. */
  index?: string;
  label: string;
  /** Formatted by the caller — formatting rules live in lib/format.ts. */
  value: ReactNode;
  /** Rendered beside the value at label size: "days", "sessions". */
  unit?: ReactNode;
  caption?: ReactNode;
  /** Right-hand side of the header: a range picker, say. */
  actions?: ReactNode;
  /** Sparkline, dot chart — anything, or nothing. */
  chart?: ReactNode;
  size?: StatCardSize;
  tone?: CardTone;
  className?: string;
}

const VALUE_CLASS: Readonly<Record<StatCardSize, string>> = {
  default: "text-title",
  hero: "text-display",
};

export function StatCard({
  index,
  label,
  value,
  unit,
  caption,
  actions,
  chart,
  size = "default",
  tone = "default",
  className,
}: StatCardProps) {
  return (
    <Card index={index} title={label} actions={actions} tone={tone} className={className}>
      <div className="flex items-baseline gap-2">
        <span className={cn("font-display tabular-nums text-text-primary", VALUE_CLASS[size])}>{value}</span>
        {unit ? <span className="text-label text-text-tertiary">{unit}</span> : null}
      </div>

      {caption ? <p className="pt-1 text-label tabular-nums text-text-secondary">{caption}</p> : null}

      {chart ? <div className="pt-4">{chart}</div> : null}
    </Card>
  );
}
