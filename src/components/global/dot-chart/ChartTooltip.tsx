/**
 * SOURCE OF TRUTH KEYWORDS: ChartTooltip, chart-hover, tooltip-card, ChartTooltipProps,
 *   chart-tooltip-half-width
 * WHAT:  The floating readout over a chart: a small elevated card that sits above
 *        the point being hovered and says what it is.
 * WHY:   The dot charts carry no axes and no stated scale (docs/04 §11), so the
 *        numbers have to be reachable by pointing at the data itself — and the
 *        card has to be the same everywhere a chart offers one. It is positioned
 *        by the CHART, in the chart's own pixels, and clamped by
 *        --chart-tooltip-half-width so it never hangs off the card the chart sits
 *        in. It never takes the pointer (pointer-events-none), or moving onto it
 *        would end the hover that put it there.
 * WHERE: DotColumns and DotMeter, inside a `relative` chart container.
 */

import type { ReactNode } from "react";
import { readPxToken } from "@/lib/motion";
import { cn } from "@/lib/utils";

export interface ChartTooltipProps {
  /** Horizontal centre of the hovered point, in the container's pixels. */
  x: number;
  /** Width of the container, for clamping. */
  containerWidth: number;
  children: ReactNode;
  className?: string;
}

export function ChartTooltip({ x, containerWidth, children, className }: ChartTooltipProps) {
  const half = readPxToken("--chart-tooltip-half-width");
  const left = Math.min(Math.max(x, half), Math.max(containerWidth - half, half));

  return (
    <div
      role="tooltip"
      style={{ left }}
      className={cn(
        "material-elevated pointer-events-none absolute bottom-full z-20 mb-3 w-max max-w-[calc(var(--chart-tooltip-half-width)*2)] -translate-x-1/2 rounded-input px-3 py-2",
        className,
      )}
    >
      {children}
    </div>
  );
}
