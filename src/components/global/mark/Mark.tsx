/**
 * SOURCE OF TRUTH KEYWORDS: Mark, MarkProps, MarkSize, identity-mark, whisper-head,
 *   quote-marks, pill-logo, mark-size-md
 * WHAT:  The app's identity mark: the head-shaped blob with two quote marks cut
 *        out of it, drawn inline as one solid colour.
 * WHY:   The same drawing as assets/mark.svg and the app icon — a head that is
 *        whispering — so the dock icon, the menu-bar glyph, the pill and every
 *        in-app logo read as one object. The quotes are HOLES (an SVG mask), not
 *        a second fill, so the mark takes whatever colour its context gives it
 *        through `currentColor`: ink in the rail, the pill's pale grey on black.
 *        Ids come from useId because two marks on one page (the rail and the
 *        onboarding tour) would otherwise share a mask id and the second would
 *        resolve to the first's geometry.
 *
 *        `look` makes it glance left and right now and then (global.css,
 *        .mark-look-*): the quotes, being holes, slide across the head while the
 *        head leans into the look. It is for the rail only — the pill's mark
 *        breathes instead, and two kinds of motion on one logo would be noise.
 *
 *        Keep the path in step with assets/mark.svg — change both or neither.
 * WHERE: The top of the dashboard rail, the pill, onboarding's first screen.
 *        The menu-bar and bundle icons are rendered from the SVG assets.
 */

import { useId } from "react";
import { cn } from "@/lib/utils";

export type MarkSize = "sm" | "md" | "lg";

export interface MarkProps {
  size?: MarkSize;
  /** Glance around on a slow loop. Off by default; the rail turns it on. */
  look?: boolean;
  /** The accessible name. Null renders the mark as decoration, which is right
   *  wherever the product name is already written next to it. */
  label?: string | null;
  className?: string;
}

const SIZE_CLASS: Readonly<Record<MarkSize, string>> = {
  sm: "w-[var(--mark-size-sm)]",
  md: "w-[var(--mark-size-md)]",
  lg: "w-[var(--mark-size-lg)]",
};

const QUOTE = "M0 0H38C72 0 100 28 100 64V120H6V62H30C52 62 52 38 30 38H2Z";
const HEAD = "M-6 46.5L0 45.5C12 39.5 30 26.5 47 26C63 25.5 82 36 87.4 52.5C90 60 90.8 75 91 105H-6Z";

export function Mark({ size = "md", look = false, label = null, className }: MarkProps) {
  const id = useId();
  const tile = `${id}-tile`;
  const holes = `${id}-holes`;
  const quote = `${id}-quote`;

  return (
    <svg
      role={label ? "img" : undefined}
      aria-label={label ?? undefined}
      aria-hidden={label ? undefined : "true"}
      viewBox="0 24.5 91.2 75.5"
      fill="currentColor"
      className={cn("h-auto shrink-0", SIZE_CLASS[size], className)}
    >
      <defs>
        <clipPath id={tile}>
          <rect x="0" y="0" width="91.2" height="100" rx="22.4" />
        </clipPath>
        <path id={quote} d={QUOTE} />
        <mask id={holes} maskUnits="userSpaceOnUse" x="0" y="0" width="100" height="100">
          <rect width="100" height="100" fill="#fff" />
          <g fill="#000" className={look ? "mark-look-eyes" : undefined}>
            <use href={`#${quote}`} transform="translate(44.3 43.2) rotate(-4) scale(0.126)" />
            <use href={`#${quote}`} transform="translate(60.9 42) rotate(-4) scale(0.126)" />
          </g>
        </mask>
      </defs>
      <g clipPath={`url(#${tile})`}>
        <path mask={`url(#${holes})`} d={HEAD} className={look ? "mark-look-head" : undefined} />
      </g>
    </svg>
  );
}
