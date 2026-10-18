/**
 * SOURCE OF TRUTH KEYWORDS: TourStep, TourSlide, TOUR_SLIDES, onboarding-tour,
 *   tour-media, onboarding-gif
 * WHAT:  The three-slide tour that opens onboarding: what the app does, in the
 *        order a new user needs it. Logo, a media slot, one line of copy, dots.
 * WHY:   THIS TEACHES, IT DOES NOT SET UP. The setup steps that follow are
 *        derived from backend state — permission granted, model ready — and
 *        that is right for them, because a returning user must not repeat work
 *        they already did. A tour has no backend state to derive from: it is
 *        three pages of reading, so it is the one part of this flow that is
 *        honestly a counter, and pretending otherwise would be architecture for
 *        its own sake.
 *
 *        THE THIRD SLIDE IS THE PRODUCT. Vocs transcribes on the user's own
 *        machine, and nothing in the app said so anywhere — a person could use
 *        it for a week believing their voice was being uploaded. That is the
 *        single most valuable sentence here and it gets its own slide rather
 *        than a clause at the end of another one.
 *
 *        THE MEDIA SLOT IS RESERVED BUT NOT REQUIRED.
 *        Logo, heading and one line is a finished minimal screen without it.
 * WHERE: Rendered by Onboarding before the permission/model/hotkey steps.
 */

import { useState } from "react";
import { ChevronLeft } from "lucide-react";
import type { HotkeyBinding } from "@/lib/bindings";
import type { DictationMode } from "@/lib/dictation-mode";
import { glyphsForBinding } from "@/lib/hotkey";
import { cn } from "@/lib/utils";
import { Button, Keycap } from "@/components/global";
import { TourIllustration, type TourIllustrationKind } from "./TourIllustration";

interface TourSlide {
  /** Derived from the gesture, because slide one teaches it and the two
   *  gestures are opposites — one sentence cannot be true of both. */
  title: (mode: DictationMode) => string;
  /** One line. Anything longer stops being read on a first-run screen. */
  body: (hotkey: HotkeyBinding | null, mode: DictationMode) => React.ReactNode;
  media: TourIllustrationKind;
}

const TOUR_SLIDES: readonly TourSlide[] = [
  {
    // TOGGLE IS THE DEFAULT, and the surprising half of it is that the SAME key
    // stops the recording — a person who has met push-to-talk anywhere else
    // will hold it, let go, and keep recording. So the toggle title says so
    // outright and the sentence spends its second half on stopping.
    //
    // The verbs match the registry's own option copy — "Press to start, press
    // again to stop" and "Hold to record, release to send" — so the words a
    // user meets here are the words they meet again in Settings.
    title: (mode) => (mode === "push_to_talk" ? "Hold to talk" : "Press to start and stop"),
    body: (hotkey, mode) => {
      const key = hotkey ? (
        <Keycap keys={glyphsForBinding(hotkey)} size="sm" className="mx-0.5 inline-flex align-middle" />
      ) : (
        "your shortcut"
      );
      return mode === "push_to_talk" ? (
        <>Hold {key} anywhere and say it. Let go when you are done.</>
      ) : (
        <>Press {key} anywhere and start talking. Press it again when you are done.</>
      );
    },
    media: "shortcut",
  },
  {
    title: () => "It lands where you type",
    body: () => <>Your words paste straight into whatever app you were already in.</>,
    media: "paste",
  },
  {
    title: () => "Nothing leaves your Mac",
    body: () => <>Every word is transcribed on your own machine. No account, no upload.</>,
    media: "private",
  },
];

export const TOUR_LENGTH = TOUR_SLIDES.length;

export function TourStep({
  hotkey,
  mode,
  onDone,
}: {
  hotkey: HotkeyBinding | null;
  mode: DictationMode;
  onDone: () => void;
}) {
  const [index, setIndex] = useState(0);
  const slide = TOUR_SLIDES[index]!;
  const title = slide.title(mode);
  const isLast = index === TOUR_SLIDES.length - 1;

  return (
    <section className="flex h-full flex-col items-center justify-center gap-6 px-8">
      <div className="hairline dot-grid-stage flex h-52 w-full max-w-96 items-center justify-center overflow-hidden rounded-card bg-sunken/40">
        <TourIllustration key={slide.media} kind={slide.media} hotkey={hotkey} />
      </div>

      <header className="flex flex-col items-center gap-2 text-center">
        <h1 className="font-display text-title text-text-primary">{title}</h1>
        <p className="max-w-96 text-body text-text-secondary">
          {slide.body(hotkey, mode)}
        </p>
      </header>

      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={() => setIndex((current) => current - 1)}
          disabled={index === 0}
          aria-label="Previous"
          className="flex size-[var(--control-height)] items-center justify-center rounded-input text-text-secondary transition-opacity hover:bg-sunken disabled:pointer-events-none disabled:opacity-0"
        >
          <ChevronLeft className="size-4" />
        </button>

        <Button variant="primary" onClick={() => (isLast ? onDone() : setIndex((current) => current + 1))}>
          {isLast ? "Set up Vocs" : "Next"}
        </Button>

        {/* Balances the back button so the primary action stays optically
            centred on slide one, where back is hidden. Without it the button
            drifts sideways as you page, which is the kind of movement a
            "no distraction" screen cannot afford. */}
        <span aria-hidden="true" className="size-[var(--control-height)] shrink-0" />
      </div>

      {/* The slider's only progress indicator. Clickable, because a tour the
          user cannot page back through is a slideshow they are trapped in. */}
      <div className="flex items-center gap-2">
        {TOUR_SLIDES.map((item, dotIndex) => (
          <button
            key={item.media}
            type="button"
            aria-label={`Go to ${item.title(mode)}`}
            aria-current={dotIndex === index}
            onClick={() => setIndex(dotIndex)}
            className={cn(
              "size-[var(--step-dot-size)] rounded-pill transition-colors",
              dotIndex === index ? "bg-accent" : "bg-sunken-strong hover:bg-text-tertiary",
            )}
          />
        ))}
      </div>
    </section>
  );
}
