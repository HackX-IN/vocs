/**
 * SOURCE OF TRUTH KEYWORDS: BootScreen, useBoot, boot-animation, startup-splash, dot-wordmark,
 *   boot-min-ms
 * WHAT:  The startup screen: the mark settles in, three bars breathe like the pill's,
 *        and the wordmark assembles letter by letter in the display face, over a
 *        fading dot lattice. `useBoot` decides how long it stays.
 * WHY:   A window that paints its chrome first and fills in a beat later looks
 *        unfinished, and the dashboard waits on the registry before it can draw
 *        anything true. The screen covers that wait with the product's own
 *        vocabulary (the mark, the bars, the dot-matrix face) so the first frame
 *        is already the app.
 *
 *        `useBoot` holds it for at least --boot-min-ms even when the data is
 *        instant: a screen that flashes for 80ms is a flicker, not an
 *        introduction, so the animation is given the time to finish once. It
 *        never holds longer than the work — once `ready` is true and the minimum
 *        has passed it fades out (--motion-duration-medium) and unmounts.
 *
 *        Decoration only: aria-hidden, and under prefers-reduced-motion the
 *        animations are off, so what remains is a still mark and the word.
 * WHERE: Dashboard and Onboarding, over their content.
 */

import { useEffect, useState } from "react";
import { readDurationMs } from "@/lib/motion";
import { cn } from "@/lib/utils";
import { Mark } from "../mark";

const WORD = "VOCS";

/** Visible until `ready` AND the minimum time has passed, then fading, then gone. */
export function useBoot(ready: boolean): { visible: boolean; leaving: boolean } {
  const [minElapsed, setMinElapsed] = useState(false);
  const [gone, setGone] = useState(false);

  useEffect(() => {
    const handle = window.setTimeout(() => setMinElapsed(true), readDurationMs("--boot-min-ms"));
    return () => window.clearTimeout(handle);
  }, []);

  const leaving = ready && minElapsed;

  useEffect(() => {
    if (!leaving) return;
    const handle = window.setTimeout(() => setGone(true), readDurationMs("--motion-duration-medium"));
    return () => window.clearTimeout(handle);
  }, [leaving]);

  return { visible: !gone, leaving };
}

export function BootScreen({ leaving }: { leaving: boolean }) {
  return (
    <div
      aria-hidden="true"
      data-tauri-drag-region
      className={cn(
        "boot-screen absolute inset-0 z-50 flex flex-col items-center justify-center gap-6 transition-opacity duration-[var(--motion-duration-medium)]",
        leaving ? "pointer-events-none opacity-0" : "opacity-100",
      )}
    >
      <span className="dot-grid dot-grid-fade boot-lattice pointer-events-none absolute inset-0" />

      <Mark size="lg" className="boot-mark relative text-text-primary" />

      <span className="boot-bars relative flex h-6 items-center gap-1.5 text-signal">
        {[0, 1, 2].map((index) => (
          <span
            key={index}
            style={{ animationDelay: `${index * 140}ms` }}
            className="boot-bar h-full w-[var(--pill-bar-width)] rounded-pill bg-current"
          />
        ))}
      </span>

      <p className="font-display text-title relative flex text-text-primary">
        {WORD.split("").map((letter, index) => (
          <span key={letter} style={{ animationDelay: `${300 + index * 90}ms` }} className="boot-letter">
            {letter}
          </span>
        ))}
      </p>
    </div>
  );
}
