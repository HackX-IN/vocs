/**
 * SOURCE OF TRUTH KEYWORDS: useSpeakStatus, SpeakStatus, speakStatusChanged,
 *   getSpeakStatus, safe_mode_state
 * WHAT:  The live state of Voice mode — speaking or not, meeting safe mode on or
 *        off and why — as a hook.
 * WHY:   Rust owns this state and pushes every change, including the ones nobody
 *        clicked: safe mode comes on by itself when speech fails. So the hook
 *        fetches once and then only listens; it never polls and never assumes its
 *        own request is what changed things.
 * WHERE: The Voice settings page.
 */

import { useEffect, useState } from "react";
import { commands, events, type SpeakStatus } from "@/lib/bindings";
import { unwrapCommand } from "@/lib/ipc";
import { useTauriEvent } from "@/lib/use-event";

export function useSpeakStatus(): SpeakStatus | null {
  const [status, setStatus] = useState<SpeakStatus | null>(null);

  useEffect(() => {
    let live = true;
    void unwrapCommand(commands.getSpeakStatus).then((result) => {
      if (live && result.status === "ok") setStatus((current) => current ?? result.data);
    });
    return () => {
      live = false;
    };
  }, []);

  useTauriEvent(events.speakStatusChanged, (payload) => setStatus(payload.status));
  return status;
}
