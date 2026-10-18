/**
 * SOURCE OF TRUTH KEYWORDS: PillWaveform, audioLevelChanged, WaveformHandle,
 *   displayLevel, AudioLevel
 * WHAT:  Subscribes to audio-level-changed, maps the audio's RMS onto the
 *        waveform's visual range, and pushes it straight into the Waveform's
 *        imperative handle.
 * WHY:   The event fires far faster than React should re-render, and the payload
 *        is explicitly droppable — a missed frame is invisible, queueing them
 *        would add latency to the very thing being drawn. Pushing through the
 *        ref means a level arriving costs one DOM transform write and zero
 *        React work, so the pill stays inside its frame budget while whisper is
 *        decoding on another thread. rms is preferred over peak because peak
 *        pins to the top on any transient and the bars stop meaning anything.
 *        The scaling lives here rather than in the Waveform because it is
 *        knowledge about SPEECH, not about drawing: the component's contract is
 *        a 0..1 display level, and anything feeding it something else has to
 *        say so at its own boundary.
 * WHERE: The pill's RECORDING state. Wraps components/global/waveform; curve
 *        values are tokens (docs/04 §7) and the mapping lives in lib/audio-level.ts
 *        so the Settings microphone check draws the identical curve.
 */

import { useRef } from "react";
import { events } from "@/lib/bindings";
import { toDisplayLevel, useLevelCurve } from "@/lib/audio-level";
import { useTauriEvent } from "@/lib/use-event";
import { Waveform, type WaveformHandle } from "@/components/global";

export function PillWaveform({ className }: { className?: string }) {
  const waveform = useRef<WaveformHandle>(null);
  const curve = useLevelCurve();

  useTauriEvent(events.audioLevelChanged, (payload) => {
    const rms = payload.level.rms ?? payload.level.peak ?? 0;
    waveform.current?.push(toDisplayLevel(rms, curve));
  });

  return <Waveform ref={waveform} className={className} />;
}
