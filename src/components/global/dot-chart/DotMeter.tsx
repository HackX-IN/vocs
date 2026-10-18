/**
 * SOURCE OF TRUTH KEYWORDS: DotMeter, DotMeterProps, dot-bar, dot-size, instrument-meter
 * WHAT:  A horizontal bar drawn as a row of dots: the first `value` share of
 *        them lit, the rest dim.
 * WHY:   The one-dimensional sibling of DotColumns, for the places that used a
 *        proportional bar — the language breakdown. A fixed number of dots is
 *        the scale: it needs no measuring, so it is plain CSS grid and costs no
 *        layout effect. At least one dot lights for any non-zero value, for the
 *        same reason as in DotColumns.
 * WHERE: The Stats view's language card; the model download readout.
 */

import { cn } from "@/lib/utils";

export interface DotMeterProps {
  /** 0..1 — the share of the dots to light. */
  value: number;
  dots?: number;
  /** Names the meter for assistive tech. */
  label: string;
  className?: string;
}

const DEFAULT_DOTS = 20;

export function DotMeter({ value, dots = DEFAULT_DOTS, label, className }: DotMeterProps) {
  const clamped = Math.min(1, Math.max(0, value));
  const lit = clamped === 0 ? 0 : Math.max(1, Math.round(clamped * dots));

  return (
    <span
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={1}
      aria-valuenow={clamped}
      style={{ gridTemplateColumns: `repeat(${dots}, minmax(0, 1fr))` }}
      className={cn("grid w-full items-center justify-items-center", className)}
    >
      {Array.from({ length: dots }, (_, index) => (
        <span
          key={index}
          className={cn(
            "size-[var(--dot-size)] rounded-full",
            index < lit ? "bg-accent" : "bg-text-tertiary opacity-30",
          )}
        />
      ))}
    </span>
  );
}
