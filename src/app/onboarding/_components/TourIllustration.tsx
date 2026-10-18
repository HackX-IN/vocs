/**
 * SOURCE OF TRUTH KEYWORDS: TourIllustration, TourIllustrationKind, tour_stage, onboarding_media
 * WHAT:  The picture on each tour slide, drawn from the app's own parts: the pill
 *        with its breathing bars and the shortcut's keycaps; a document with the
 *        words arriving in it; and a lock inside a ring of dots.
 * WHY:   The slides used to point at three GIFs that were never shipped, so the
 *        tour was a mark and two sentences. These cost no bytes, cannot go
 *        missing, follow the colour scheme, and are made of the same pieces the
 *        person is about to meet — the pill they will see at the bottom of the
 *        screen is the pill on slide one. Decoration only: aria-hidden, and the
 *        motion is the shared boot motion, which respects reduced-motion.
 * WHERE: TourStep.tsx, inside the stage card.
 */

import { Laptop, Lock } from "lucide-react";
import type { HotkeyBinding } from "@/lib/bindings";
import { glyphsForBinding } from "@/lib/hotkey";
import { Keycap, Mark } from "@/components/global";

export type TourIllustrationKind = "shortcut" | "paste" | "private";

export function TourIllustration({ kind, hotkey }: { kind: TourIllustrationKind; hotkey: HotkeyBinding | null }) {
  switch (kind) {
    case "shortcut":
      return (
        <div aria-hidden="true" className="flex flex-col items-center gap-5">
          <div className="flex h-[var(--pill-height)] w-[var(--pill-width)] items-center justify-center gap-[var(--pill-content-gap)] rounded-pill border-[length:var(--pill-ring-width)] border-pill-ring bg-pill-surface px-4 text-pill-ink">
            <Mark size="md" className="animate-breathe" />
            <span className="flex h-[var(--pill-bar-max)] items-center gap-[var(--pill-bar-gap)] text-pill-signal">
              {[0, 1, 2].map((bar) => (
                <span
                  key={bar}
                  style={{ animationDelay: `${bar * 140}ms` }}
                  className="boot-bar h-full w-[var(--pill-bar-width)] rounded-pill bg-current"
                />
              ))}
            </span>
          </div>
          {hotkey ? <Keycap keys={glyphsForBinding(hotkey)} size="sm" /> : null}
        </div>
      );
    case "paste":
      return (
        <div
          aria-hidden="true"
          className="hairline flex w-56 flex-col gap-2 rounded-card bg-elevated p-4 shadow-card"
        >
          <span className="h-2 w-16 rounded-pill bg-sunken-strong" />
          <span className="h-2 w-full rounded-pill bg-sunken" />
          <span className="h-2 w-4/5 rounded-pill bg-sunken" />
          <p className="caption-gradient text-label font-medium">
            Hello — this is what I said
            <span className="boot-caret ml-0.5 inline-block h-3.5 w-px translate-y-0.5 bg-accent" />
          </p>
        </div>
      );
    case "private":
      return (
        <div aria-hidden="true" className="relative flex size-36 items-center justify-center">
          <span className="dot-grid dot-grid-fade absolute inset-0 rounded-full" />
          <span className="hairline absolute inset-4 rounded-full bg-elevated" />
          <Laptop className="relative size-10 text-text-secondary" strokeWidth={1.5} />
          <span className="absolute right-6 bottom-6 flex size-8 items-center justify-center rounded-full bg-accent text-accent-ink">
            <Lock className="size-4" />
          </span>
        </div>
      );
  }
}
