/**
 * SOURCE OF TRUTH KEYWORDS: VoicePanel, SafeModeSwitch, testVoice, setSafeMode,
 *   requestPersonalVoice, PersonalVoiceStatus, virtual_device_hint
 * WHAT:  Everything on the Voice page that is not a plain setting: the meeting
 *        safe mode switch with its live state, a test of the chosen voice and
 *        device, the Personal Voice permission, and what to do when no virtual
 *        audio device is installed.
 * WHY:   These are the parts a person needs while SETTING UP Voice mode and
 *        again in an emergency. Safe mode is a big, plain switch with the reason
 *        beside it, because when it has come on by itself the person must see it
 *        at a glance, mid-meeting. The test reports how long the first sound
 *        took — that number is the dead air a meeting hears, and a person
 *        choosing between two devices deserves to see it.
 *
 *        Personal Voice is explained per state rather than as one disabled
 *        control: Unsupported says it needs macOS 14, Denied says where to change
 *        it (an app cannot re-ask after a refusal), NotDetermined offers the
 *        button.
 * WHERE: The second card of the Voice page in Settings.
 */

import { useCallback, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands, type AppError, type PersonalVoiceStatus } from "@/lib/bindings";
import { unwrapCommand, useCommand } from "@/lib/ipc";
import { useSpeakStatus } from "@/lib/use-speak-status";
import { Button, ErrorSurface } from "@/components/global";

const BLACKHOLE_URL = "https://existential.audio/blackhole/";

const PERSONAL_VOICE_COPY: Readonly<Record<PersonalVoiceStatus, string>> = {
  UNSUPPORTED: "Personal Voice needs macOS 14 or later. Vocs will use a system voice.",
  NOT_DETERMINED: "Allow Vocs to speak in your own Personal Voice. You make one in System Settings → Accessibility → Personal Voice.",
  DENIED:
    "Personal Voice is turned off for Vocs. Allow it in System Settings → Privacy & Security → Personal Voice.",
  AUTHORIZED: "Allowed. Your Personal Voice appears first in the voice list above.",
};

export function VoicePanel() {
  const status = useSpeakStatus();
  const personal = useCommand(commands.getPersonalVoiceStatus, []);
  const devices = useCommand(commands.listOutputDevices, []);
  const [testing, setTesting] = useState(false);
  const [tested, setTested] = useState<string | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [asking, setAsking] = useState(false);

  const test = useCallback(async () => {
    setTesting(true);
    setError(null);
    setTested(null);
    const result = await unwrapCommand(commands.testVoice);
    setTesting(false);
    if (result.status === "error") setError(result.error);
    else setTested(`The first sound came out in ${result.data} ms.`);
  }, []);

  const toggleSafeMode = useCallback(async () => {
    setError(null);
    const result = await unwrapCommand(() => commands.setSafeMode({ on: !(status?.safe_mode ?? false) }));
    if (result.status === "error") setError(result.error);
  }, [status?.safe_mode]);

  const allow = useCallback(async () => {
    setAsking(true);
    await unwrapCommand(commands.requestPersonalVoice);
    setAsking(false);
    personal.reload();
  }, [personal]);

  const safeMode = status?.safe_mode ?? false;
  const hasVirtualDevice = (devices.data ?? []).some((device) => device.is_virtual);

  return (
    <div className="flex flex-col">
      <div className="hairline-b flex items-center justify-between gap-4 py-3">
        <div className="min-w-0 flex-1">
          <p className="text-body text-text-primary">
            Meeting safe mode {safeMode ? <span className="text-accent">— on</span> : null}
          </p>
          <p className="text-caption text-text-secondary">
            {safeMode
              ? (status?.safe_mode_reason ?? "Your real microphone is going straight to the meeting.")
              : "One press passes your real microphone to the meeting, if speech ever fails or lags."}
          </p>
        </div>
        <Button variant={safeMode ? "primary" : "default"} onClick={() => void toggleSafeMode()} aria-pressed={safeMode}>
          {safeMode ? "Turn off" : "Turn on"}
        </Button>
      </div>

      <div className="hairline-b flex items-center justify-between gap-4 py-3">
        <div className="min-w-0 flex-1">
          <p className="text-body text-text-primary">Test the voice</p>
          <p className="text-caption text-text-secondary">
            {tested ?? "Speaks a short sentence with the voice and device above."}
          </p>
        </div>
        <Button onClick={() => void test()} disabled={testing || safeMode}>
          {testing ? "Speaking…" : "Test"}
        </Button>
      </div>

      <div className="hairline-b flex items-center justify-between gap-4 py-3">
        <div className="min-w-0 flex-1">
          <p className="text-body text-text-primary">Personal Voice</p>
          <p className="text-caption text-text-secondary">
            {personal.data ? PERSONAL_VOICE_COPY[personal.data] : "Checking…"}
          </p>
        </div>
        {personal.data === "NOT_DETERMINED" ? (
          <Button onClick={() => void allow()} disabled={asking}>
            Allow
          </Button>
        ) : null}
      </div>

      <div className="flex items-center justify-between gap-4 py-3 last:border-b-0">
        <div className="min-w-0 flex-1">
          <p className="text-body text-text-primary">Getting it into the meeting</p>
          <p className="text-caption text-text-secondary">
            {hasVirtualDevice
              ? "A virtual audio device is installed. In Zoom, Meet or Teams, choose it as your microphone and Vocs speaks into it."
              : "No virtual audio device was found. Install a free one such as BlackHole, restart Vocs, then choose it as your microphone in the meeting app."}
          </p>
        </div>
        {hasVirtualDevice ? null : <Button onClick={() => void openUrl(BLACKHOLE_URL)}>Get BlackHole</Button>}
      </div>

      {error ? <ErrorSurface size="compact" error={error} /> : null}
    </div>
  );
}
