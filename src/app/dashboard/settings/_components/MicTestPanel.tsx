/**
 * SOURCE OF TRUTH KEYWORDS: MicTestPanel, startMicTest, stopMicTest,
 *   micTestChanged, mic-check, SILENCE_HINT_MS, MIC_STOP_NOTICE
 * WHAT:  A live input meter under Settings → Recording: start it, talk, watch
 *        the bars move.
 * WHY:   Whether the right microphone is picked and loud enough is a question
 *        the user can only answer by speaking, and until now the only way to
 *        ask it was to dictate something and see what came out. The levels
 *        arrive as events and go straight into the Waveform's imperative
 *        handle, the same path the pill uses, so a 30fps meter costs no React
 *        renders. Rust decides when the check ends (a lost device, a deadline
 *        that stops a forgotten window holding the mic) and this component only
 *        reflects it — it never assumes its own stop request is what ended it.
 *        Changing the input device while listening restarts the check on the
 *        new device, so the meter is never pointed at one the setting no longer
 *        names. If nothing audible arrives for a few seconds the panel says so,
 *        because a flat meter is otherwise indistinguishable from "working, but
 *        you have not spoken yet".
 * WHERE: Rendered by SettingsView in the RECORDING section (EXTRAS).
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AppError, type MicTestStopReason } from "@/lib/bindings";
import { toDisplayLevel, useLevelCurve } from "@/lib/audio-level";
import { unwrapCommand } from "@/lib/ipc";
import { useTauriEvent } from "@/lib/use-event";
import { Button, ErrorSurface, Waveform, type WaveformHandle } from "@/components/global";
import { useSettings } from "../../use-settings";

/** How long the meter may stay flat before we suggest something is wrong. */
const SILENCE_HINT_MS = 3000;
/** RMS under this is room noise, not a voice. */
const AUDIBLE_RMS = 0.01;
const INPUT_DEVICE_KEY = "dictation.input_device";

/** Endings worth telling the user about. A request or a timeout is not news. */
const MIC_STOP_NOTICE: Readonly<Partial<Record<MicTestStopReason, string>>> = {
  DEVICE_LOST: "That microphone disconnected, so the check stopped.",
};

export function MicTestPanel() {
  const waveform = useRef<WaveformHandle>(null);
  const curve = useLevelCurve();
  const settings = useSettings();
  const [listening, setListening] = useState(false);
  const [quiet, setQuiet] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<AppError | null>(null);

  const heardSound = useRef(false);
  const listeningRef = useRef(false);
  listeningRef.current = listening;

  const device = settings.data?.[INPUT_DEVICE_KEY];
  const deviceKey = device?.type === "CHOICE" ? device.value : "default";

  const stop = useCallback(() => {
    setListening(false);
    waveform.current?.reset();
    void unwrapCommand(commands.stopMicTest);
  }, []);

  const start = useCallback(async () => {
    setError(null);
    setNotice(null);
    setQuiet(false);
    heardSound.current = false;
    const result = await unwrapCommand(commands.startMicTest);
    if (result.status === "error") {
      setListening(false);
      setError(result.error);
      return;
    }
    setListening(true);
  }, []);

  useTauriEvent(events.micTestChanged, ({ update }) => {
    if (update.kind === "LEVEL") {
      // A non-finite reading arrives as null; the pill falls back the same way.
      const rms = update.level.rms ?? update.level.peak ?? 0;
      if (rms > AUDIBLE_RMS) heardSound.current = true;
      waveform.current?.push(toDisplayLevel(rms, curve));
      return;
    }
    setListening(false);
    waveform.current?.reset();
    setNotice(MIC_STOP_NOTICE[update.reason] ?? null);
  });

  // The hint is a one-shot per check, so it is a timer rather than a per-frame
  // comparison.
  useEffect(() => {
    if (!listening) return;
    const timer = window.setTimeout(() => setQuiet(!heardSound.current), SILENCE_HINT_MS);
    return () => window.clearTimeout(timer);
  }, [listening]);

  // Point the meter at the device the setting now names.
  useEffect(() => {
    if (listeningRef.current) void start();
  }, [deviceKey, start]);

  // Leaving Settings must not leave the microphone open.
  useEffect(
    () => () => {
      if (listeningRef.current) void unwrapCommand(commands.stopMicTest);
    },
    [],
  );

  return (
    <div className="flex flex-col gap-2 py-3">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0 flex-1">
          <p className="text-body text-text-primary">Check your microphone</p>
          <p className="text-caption text-text-secondary">
            {quiet && listening
              ? "No sound yet. Speak up, or pick a different input device above."
              : (notice ?? "Talk normally — the bars should move. Nothing is recorded.")}
          </p>
        </div>
        <Waveform ref={waveform} className="shrink-0" />
        <Button onClick={listening ? stop : () => void start()} aria-pressed={listening}>
          {listening ? "Stop" : "Test"}
        </Button>
      </div>
      {error ? <ErrorSurface size="compact" error={error} /> : null}
    </div>
  );
}
