/**
 * SOURCE OF TRUTH KEYWORDS: WindowBar, WindowBarProps, titlebar, traffic-lights,
 *   safe-area, data-tauri-drag-region, centred-title, toolbar
 * WHAT:  The strip across the top of a window: leading control on the left,
 *        a centred title, actions on the right — all on one centre line with the
 *        macOS traffic lights.
 * WHY:   Every page and the dictation editor used to draw their own header, so
 *        the title, the back button and the toolbar each sat at a slightly
 *        different height and inset. One bar, three slots, one height
 *        (--titlebar-height) means the lights, the title and the buttons share a
 *        line, and the left cell starts past --titlebar-safe-inset so nothing is
 *        ever drawn under the lights. The centre cell is a grid track rather than
 *        a flex child so the title stays centred on the window however wide the
 *        side slots are.
 *
 *        THE TITLE IS CENTRED ON THE WINDOW, not on what is left of it. The bar
 *        used to pad its left edge by the traffic-light inset (92px) and its
 *        right by the page margin (32px), which moved the middle of the grid —
 *        and so the title — 30px right of the window's centre, visibly off the
 *        centre line of the content below it. Padding is now symmetric and the
 *        traffic-light clearance lives on the leading slot alone, so the two
 *        1fr tracks stay equal and the title sits on the true centre.
 *
 *        The whole bar is the drag region; slot children that are interactive
 *        opt out with data-tauri-drag-region={false}, or a click on a button
 *        would drag the window instead.
 * WHERE: PageShell (routed pages) and DictationEditor (detail toolbar).
 */

import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/utils";

export interface WindowBarProps extends Omit<HTMLAttributes<HTMLElement>, "title"> {
  /** Left slot, after the traffic-light safe area. */
  leading?: ReactNode;
  /** Centre slot. A string is set as the page title. */
  title?: ReactNode;
  /** Right slot. */
  trailing?: ReactNode;
}

export function WindowBar({ leading, title, trailing, className, ...rest }: WindowBarProps) {
  return (
    <header
      data-tauri-drag-region
      {...rest}
      className={cn(
        "absolute inset-x-0 top-0 z-10 grid h-[var(--titlebar-height)] grid-cols-[1fr_auto_1fr] items-center gap-4 px-[var(--page-padding-x)]",
        className,
      )}
    >
      <div
        data-tauri-drag-region={false}
        className="flex min-w-0 items-center justify-self-start pl-[calc(var(--titlebar-safe-inset)-var(--page-padding-x))]"
      >
        {leading}
      </div>
      <div data-tauri-drag-region className="flex min-w-0 items-center justify-center">
        {typeof title === "string" ? (
          <h1 data-tauri-drag-region className="text-heading truncate font-medium text-text-primary">
            {title}
          </h1>
        ) : (
          title
        )}
      </div>
      <div data-tauri-drag-region={false} className="flex min-w-0 items-center gap-2 justify-self-end">
        {trailing}
      </div>
    </header>
  );
}
