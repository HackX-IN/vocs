/**
 * SOURCE OF TRUTH KEYWORDS: useLevelCurve, toDisplayLevel, LevelCurve,
 *   waveform-input-ceiling, waveform-input-gamma, rmsToDisplay
 * WHAT:  Maps a linear RMS reading from the audio thread onto the 0..1 range a
 *        Waveform draws, using the curve declared in tokens.css.
 * WHY:   Two surfaces draw microphone input — the pill while recording and the
 *        Settings meter while checking — and they must agree, or the meter says
 *        "plenty loud" and the pill then shows a flat line. One function and
 *        one pair of tokens (docs/04 §7) is what makes that true by
 *        construction. The tokens are read once: this runs on every audio
 *        frame, and getComputedStyle there would cost a style resolution per
 *        frame.
 * WHERE: PillWaveform.tsx and settings/_components/MicTestPanel.tsx.
 */

import { useState } from "react";
import { readNumberToken } from "./motion";

export interface LevelCurve {
  ceiling: number;
  gamma: number;
}

/** `level = clamp((rms / ceiling) ^ gamma)`, per docs/04 §7. */
export function toDisplayLevel(rms: number, curve: LevelCurve): number {
  const ratio = Math.min(1, Math.max(0, rms / curve.ceiling));
  return Math.pow(ratio, curve.gamma);
}

export function useLevelCurve(): LevelCurve {
  const [curve] = useState<LevelCurve>(() => ({
    ceiling: readNumberToken("--waveform-input-ceiling"),
    gamma: readNumberToken("--waveform-input-gamma"),
  }));
  return curve;
}
