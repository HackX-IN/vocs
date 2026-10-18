/**
 * SOURCE OF TRUTH KEYWORDS: useHotkeyCapture, pauseHotkeys, hotkey_capture, shortcut_recorder
 * WHAT:  While `active`, releases every global shortcut so the keys being
 *        recorded reach the page instead of running the binding they replace.
 * WHY:   Recording a new dictation shortcut started a dictation: the key the
 *        user presses is often the one that is bound right now. The release is
 *        undone by the effect cleanup, so leaving the view, disarming, or
 *        switching apps all restore the shortcuts — a recorder that could leave
 *        the app with no hotkey is worse than the bug it fixes.
 * WHERE: HotkeyControl in components/global/setting-control.
 */

import { useEffect } from "react";
import { commands } from "@/lib/bindings";
import { unwrapCommand } from "@/lib/ipc";

export function useHotkeyCapture(active: boolean): void {
  useEffect(() => {
    if (!active) return;
    void unwrapCommand(() => commands.pauseHotkeys({ paused: true }));
    return () => {
      void unwrapCommand(() => commands.pauseHotkeys({ paused: false }));
    };
  }, [active]);
}
