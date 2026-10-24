/**
 * SOURCE OF TRUTH KEYWORDS: PageTransition, PageTransitionProps, page-enter,
 *   route-transition, keyed-fade
 * WHAT:  A wrapper that plays the app's one arrival animation — a short fade and
 *        rise — every time its `id` changes.
 * WHY:   Navigation used to swap content in a single frame, which reads as a
 *        flash, not a change of place. Keying the wrapper on the id remounts it,
 *        and a remount restarts a pure CSS animation, so there is no animation
 *        state to hold and no JS frame spent. The animation lives in global.css
 *        (`.page-enter`, driven by --motion-duration-medium and
 *        --page-enter-distance) so every surface that wants it shares one curve.
 *        It is deliberately an ENTER only: an exit animation would hold the old
 *        page on screen while the new one loads, which makes the app feel slower
 *        than the swap it dresses up.
 * WHERE: Dashboard (per route) and the Settings page body (per section). The
 *        History list reuses the same class directly, because it hides rather
 *        than unmounts.
 */

import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

export interface PageTransitionProps {
  /** Changing this replays the animation. */
  id: string;
  /** Layout for the wrapper; the animation itself needs none. */
  className?: string;
  children: ReactNode;
}

export function PageTransition({ id, className, children }: PageTransitionProps) {
  return (
    <div key={id} className={cn("page-enter", className)}>
      {children}
    </div>
  );
}
