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
 *        design, so it only records a TARGET; a requestAnimationFrame loop moves
 *        the bars toward it — quick to rise, slow to fall, and a gentle idle
 *        ripple between words — with one DOM transform per bar per frame and zero
 *        React work, inside the pill's frame budget while whisper decodes on
 *        another thread. Smoothing lives in the loop, not in a CSS transition:
 *        a transition restarts on every write and so lags the signal it follows. rms is preferred over peak because peak pins to the top
 *        on any transient. The scaling lives in lib/audio-level.ts, shared with
 *        the Settings microphone check, so both draw the identical curve.
 * WHERE: The capsule pill's RECORDING and ARMING states. Wraps
 *        components/global/waveform with 3 bars, centre-aligned; bar size and
 *        gap come from the --pill-bar-* tokens.
 */

import { useEffect, useRef, useState } from "react";
import { useReducedMotion } from "framer-motion";
import { events } from "@/lib/bindings";
import { toDisplayLevel, useLevelCurve } from "@/lib/audio-level";
import { readNumberToken } from "@/lib/motion";
import { useTauriEvent } from "@/lib/use-event";
import { Waveform, type WaveformHandle } from "@/components/global";

/** Resting heights as a share of the full bar: short, tall, medium. */
const REST_SHAPE = [0.3, 0.5, 0.38] as const;
/** How much of the previous levels each flank borrows, so the bars do not move
 *  in lock-step — the flanks trail the centre, which is what makes it breathe. */
const FLANK_WEIGHT = [0.7, 0.85] as const;
const BAR_COUNT = REST_SHAPE.length;

/**
 * WHAT:  The tokens that shape the motion, read once.
 * WHY:   The animation loop runs every frame; a style lookup per frame would cost
 *        more than the animation. See --pill-bar-attack and its neighbours in
 *        tokens.css for what each number means.
 */
function readDynamics() {
  return {
    attack: readNumberToken("--pill-bar-attack"),
    release: readNumberToken("--pill-bar-release"),
    idleAmplitude: readNumberToken("--pill-bar-idle-amplitude"),
    idlePeriodMs: readNumberToken("--pill-bar-idle-period-ms"),
  };
}

export function PillBars({ className }: { className?: string }) {
  const bars = useRef<WaveformHandle>(null);
  const curve = useLevelCurve();
  const [dynamics] = useState(readDynamics);
  const reduced = useReducedMotion() === true;
  const previous = useRef<[number, number]>([0, 0]);
  /** Where the audio says each bar should be. Written by the event, read by the loop. */
  const target = useRef<number[]>([...REST_SHAPE]);

  useTauriEvent(events.audioLevelChanged, (payload) => {
    const rms = payload.level.rms ?? payload.level.peak ?? 0;
    const level = toDisplayLevel(rms, curve);
    const [last, beforeLast] = previous.current;
    previous.current = [level, last];

    target.current = [
      Math.max(REST_SHAPE[0], last * FLANK_WEIGHT[0]),
      Math.max(REST_SHAPE[1], level),
      Math.max(REST_SHAPE[2], beforeLast * FLANK_WEIGHT[1]),
    ];
  });

  // One frame loop, one DOM write per frame. Each bar chases its target with a
  // fast attack and a slow release, and between words a slow shimmer keeps the
  // bars alive, a third of a cycle apart so they ripple. Under reduced motion
  // the shimmer is off and the bars simply follow the level.
  useEffect(() => {
    const shown = [...REST_SHAPE];
    const { attack, release, idleAmplitude, idlePeriodMs } = dynamics;
    let frame = 0;

    const tick = (now: number) => {
      for (let bar = 0; bar < BAR_COUNT; bar += 1) {
        const phase = (now / idlePeriodMs + bar / BAR_COUNT) * Math.PI * 2;
        const resting = REST_SHAPE[bar] + (reduced ? 0 : idleAmplitude * Math.sin(phase));
        const wanted = Math.max(resting, target.current[bar] ?? resting);
        shown[bar] += (wanted - shown[bar]) * (wanted > shown[bar] ? attack : release);
      }
      bars.current?.set(shown);
      frame = requestAnimationFrame(tick);
    };

    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [dynamics, reduced]);

  return (
    <Waveform
      ref={bars}
      bars={BAR_COUNT}
      align="center"
      className={`[--waveform-bar-width:var(--pill-bar-width)] [--waveform-bar-gap:var(--pill-bar-gap)] [--waveform-bar-max:var(--pill-bar-max)] ${className ?? ""}`}
    />
  );
}
