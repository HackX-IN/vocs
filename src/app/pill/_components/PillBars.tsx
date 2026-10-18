/**
 * SOURCE OF TRUTH KEYWORDS: PillBars, audioLevelChanged, WaveformHandle, equaliser,
 *   REST_SHAPE, displayLevel, pill-bars, three-bars
 * WHAT:  The capsule's three bars: subscribes to audio-level-changed and drives a
 *        centre-weighted three-bar equaliser straight through the Waveform's
 *        imperative handle.
 * WHY:   The reference capsule shows three bars — short, tall, medium — and that
 *        asymmetric shape is what reads as "listening" at a glance. So the bars
 *        are not a scrolling history (which would put the newest sample at the
 *        edge) but an equaliser: the centre bar follows the current level, the
 *        flanks follow the last two with their own weights, and every bar is
 *        floored at its resting height so silence shows the signature shape
 *        rather than three dots. The floor is what keeps the pill alive-looking
 *        during a pause — a flat line reads as "it stopped listening" (docs/04
 *        §7).
 *
 *        The event fires far faster than React should render and is droppable by
 *        design, so levels go through the ref: one DOM transform per bar, zero
 *        React work, inside the pill's frame budget while whisper decodes on
 *        another thread. rms is preferred over peak because peak pins to the top
 *        on any transient. The scaling lives in lib/audio-level.ts, shared with
 *        the Settings microphone check, so both draw the identical curve.
 * WHERE: The capsule pill's RECORDING and ARMING states. Wraps
 *        components/global/waveform with 3 bars, centre-aligned; bar size and
 *        gap come from the --pill-bar-* tokens.
 */

import { useEffect, useRef } from "react";
import { events } from "@/lib/bindings";
import { toDisplayLevel, useLevelCurve } from "@/lib/audio-level";
import { useTauriEvent } from "@/lib/use-event";
import { Waveform, type WaveformHandle } from "@/components/global";

/** Resting heights as a share of the full bar: short, tall, medium. */
const REST_SHAPE = [0.38, 0.72, 0.52] as const;
/** How much of the previous levels each flank borrows, so the bars do not move
 *  in lock-step — the flanks trail the centre, which is what makes it breathe. */
const FLANK_WEIGHT = [0.7, 0.85] as const;

export function PillBars({ className }: { className?: string }) {
  const bars = useRef<WaveformHandle>(null);
  const curve = useLevelCurve();
  const previous = useRef<[number, number]>([0, 0]);

  // Start at the resting shape rather than at the floor.
  useEffect(() => {
    bars.current?.set(REST_SHAPE);
  }, []);

  useTauriEvent(events.audioLevelChanged, (payload) => {
    const rms = payload.level.rms ?? payload.level.peak ?? 0;
    const level = toDisplayLevel(rms, curve);
    const [last, beforeLast] = previous.current;
    previous.current = [level, last];

    bars.current?.set([
      Math.max(REST_SHAPE[0], last * FLANK_WEIGHT[0]),
      Math.max(REST_SHAPE[1], level),
      Math.max(REST_SHAPE[2], beforeLast * FLANK_WEIGHT[1]),
    ]);
  });

  return (
    <Waveform
      ref={bars}
      bars={3}
      align="center"
      className={`[--waveform-bar-width:var(--pill-bar-width)] [--waveform-bar-gap:var(--pill-bar-gap)] [--waveform-bar-max:var(--pill-bar-max)] ${className ?? ""}`}
    />
  );
}
