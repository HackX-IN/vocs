/**
 * SOURCE OF TRUTH KEYWORDS: DotColumns, DotColumnsProps, dot-matrix-chart, dot-row-pitch,
 *   dot-radius, instrument-chart, useElementBox
 * WHAT:  A column chart drawn as a lattice of dots: one column per value, the
 *        lit dots rising from the floor in proportion to the value, the rest
 *        dim.
 * WHY:   This is the chart form of the interface's dot-matrix voice (docs/04
 *        §1, §11) and it keeps every rule the old line chart kept: no axes, no
 *        gridlines, no frame, no legend, no load animation and no chart library — a dot is a <circle> and the whole chart is a few
 *        hundred of them. Hovering a column lights it and raises a ChartTooltip —
 *        the only place the numbers are reachable, since there is no axis.
 *        The dim dots are not chrome: they are what makes the
 *        columns read as a column of fixed height, so "half full" is visible
 *        without a scale.
 *
 *        Geometry is measured in real pixels (useElementBox) rather than drawn
 *        into a stretched viewBox, which would turn every circle into an
 *        ellipse. The vertical scale is 0-to-busiest and never stated; a value
 *        above zero always lights at least one dot, because "something
 *        happened" and "nothing happened" must not look the same.
 *
 *        The newest column is accented. Colour here carries one fact — where the
 *        series ends — and no other.
 * WHERE: The Stats view (activity, streak). Any future small time series.
 */

import { useMemo, useState, type ReactNode } from "react";
import { useElementBox } from "@/lib/use-element-box";
import { readPxToken } from "@/lib/motion";
import { cn } from "@/lib/utils";
import { ChartTooltip } from "./ChartTooltip";

export interface DotColumnsProps {
  /** One value per column, oldest first. */
  values: readonly number[];
  /** How many dots tall a full column is. */
  rows?: number;
  /** Names the chart for assistive tech: "Sessions per day, Sep 1 to Sep 30". */
  label: string;
  /** What to say about column `index` while it is hovered. Omit for no tooltip. */
  tooltip?: (index: number) => ReactNode;
  className?: string;
}

const DEFAULT_ROWS = 7;

export function DotColumns({ values, rows = DEFAULT_ROWS, label, tooltip, className }: DotColumnsProps) {
  const [ref, box] = useElementBox();
  const [hovered, setHovered] = useState<number | null>(null);
  const [rowPitch] = useState(() => readPxToken("--dot-row-pitch"));
  const [maxRadius] = useState(() => readPxToken("--dot-radius"));

  const dots = useMemo(() => {
    if (!box || values.length === 0) return [];
    const busiest = Math.max(...values, 1);
    const pitch = box.width / values.length;
    const radius = Math.min(maxRadius, pitch * 0.34);
    const out: { key: string; column: number; cx: number; cy: number; r: number; lit: boolean; newest: boolean }[] = [];

    values.forEach((value, column) => {
      const litCount = value <= 0 ? 0 : Math.max(1, Math.round((value / busiest) * rows));
      for (let row = 0; row < rows; row += 1) {
        out.push({
          key: `${column}-${row}`,
          column,
          cx: pitch * (column + 0.5),
          cy: box.height - rowPitch * (row + 0.5),
          r: radius,
          lit: row < litCount,
          newest: column === values.length - 1,
        });
      }
    });
    return out;
  }, [box, values, rows, rowPitch, maxRadius]);

  const pitch = box && values.length > 0 ? box.width / values.length : 0;

  return (
    <div
      ref={ref}
      style={{ height: rows * rowPitch }}
      className={cn("relative w-full", className)}
      onPointerMove={
        tooltip && pitch > 0
          ? (event) => {
              const left = event.currentTarget.getBoundingClientRect().left;
              const column = Math.floor((event.clientX - left) / pitch);
              setHovered(Math.min(Math.max(column, 0), values.length - 1));
            }
          : undefined
      }
      onPointerLeave={() => setHovered(null)}
    >
      {box ? (
        <svg role="img" aria-label={label} width={box.width} height={box.height} className="block overflow-visible">
          {hovered !== null ? (
            <rect
              x={pitch * hovered}
              y={-rowPitch / 2}
              width={pitch}
              height={box.height + rowPitch}
              rx={pitch / 2}
              className="fill-sunken-strong opacity-60"
            />
          ) : null}
          {dots.map((dot) => (
            <circle
              key={dot.key}
              cx={dot.cx}
              cy={dot.cy}
              r={dot.r}
              className={
                dot.lit
                  ? dot.newest || dot.column === hovered
                    ? "fill-accent"
                    : "fill-text-primary opacity-70"
                  : "fill-text-tertiary opacity-30"
              }
            />
          ))}
        </svg>
      ) : null}
      {tooltip && box && hovered !== null ? (
        <ChartTooltip x={pitch * (hovered + 0.5)} containerWidth={box.width}>
          {tooltip(hovered)}
        </ChartTooltip>
      ) : null}
    </div>
  );
}
