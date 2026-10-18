/**
 * SOURCE OF TRUTH KEYWORDS: Pill, PillBody, SessionState, sessionStateChanged,
 *   VisibleState, PillTone, pill-width, pill-width-failed, window-is-the-pill,
 *   capsule-pill, pill-ring
 * WHAT:  The overlay. Renders the current SessionState and nothing else — a dark
 *        horizontal capsule holding the mark beside three audio bars, a draining
 *        line while a cancel is armed, or (wider) the mark beside a failure
 *        sentence.
 * WHY:   THE WINDOW IS THE PILL. The NSPanel is sized to the pill's tokens and
 *        its vibrancy carries --radius-pill, so every pixel of the window is the
 *        capsule and there is no slack to hide a mistake in. Three rules follow,
 *        and breaking any one of them puts the grey rectangle back on screen:
 *
 *        1. THE PILL FILLS THE WINDOW. It never animates its own size and never
 *           transforms itself. A CSS morph would race a native window resize
 *           that is not frame-locked to it, and every frame where the two
 *           disagree is bare window beside the pill. Motion that moves the pill
 *           AS AN OBJECT — arriving, resizing, leaving — belongs to the window,
 *           in Rust. Motion INSIDE the pill belongs here.
 *        2. IDLE NEVER BLANKS THE VIEW. Rust hides the window on IDLE, and the
 *           same transition also reaches this webview over IPC. Those two do not
 *           land on the same frame, so a view that cleared on IDLE would leave
 *           an empty capsule on screen until the native hide caught up — a
 *           blank blob flashing at the end of every dictation. The view renders
 *           the last state that had something to say and lets the window take it
 *           away. `live` is tracked separately so the logo's breathing STOPS on
 *           IDLE; only the pixels persist.
 *        3. THE SWITCH IS EXHAUSTIVE, with a `never` assertion rather than a
 *           default. A new variant in the Rust enum has to become a compile
 *           error here, not a silently blank pill.
 *
 *        It is dark in both colour schemes: it floats over arbitrary content, and
 *        a near-black capsule with a lighter ring is legible over all of it. It
 *        has no timer — the capsule is the shape of "I am listening", and a
 *        number in it would make it a gauge. It lies on its side so the live
 *        caption (a separate window) can rise straight out of its top edge.
 *
 *        It holds no domain state: get_session_state is called ONCE for first
 *        paint and every frame after that comes from session-state-changed, so
 *        there is no second copy of the truth and nothing to poll (CLAUDE.md §7).
 * WHERE: Mounted by src/entries/pill.tsx into the NSPanel window. Sizes come from
 *        tokens.css, which src-tauri/src/tray.rs reads and asserts against its
 *        own constants. Draws with the global Mark and CountdownLine.
 */

import { useEffect, useRef, useState } from "react";
import { commands, events, type SessionState } from "@/lib/bindings";
import { isTransientFailure } from "@/lib/errors";
import { useTauriEvent } from "@/lib/use-event";
import { unwrapCommand } from "@/lib/ipc";
import { readDurationMs } from "@/lib/motion";
import { cn } from "@/lib/utils";
import { CountdownLine, Mark } from "@/components/global";
import { PillBars } from "./_components/PillBars";

/** Every state that puts something on screen. IDLE is the window's business. */
type VisibleState = Exclude<SessionState, { kind: "IDLE" }>;

/** A failure the user can simply retry in a moment is not drawn as a fault:
 *  "still starting up" on a first-ever keypress would say the app is broken
 *  when it is seven seconds old (docs/04 §2). */
function isFault(state: VisibleState): boolean {
  return state.kind === "FAILED" && !isTransientFailure(state.code);
}

export function Pill() {
  /** The last state worth drawing. Never cleared — see rule 2 above. */
  const [shown, setShown] = useState<VisibleState | null>(null);
  /** Whether a session is actually running, which is a different question. */
  const [live, setLive] = useState(false);
  const [refilling, setRefilling] = useState(false);
  const previousKind = useRef<VisibleState["kind"] | null>(null);

  // First paint only. Every subsequent state arrives on the event.
  useEffect(() => {
    void unwrapCommand(commands.getSessionState).then((result) => {
      if (result.status !== "ok") return;
      const state = result.data;
      setLive(state.kind !== "IDLE");
      if (state.kind !== "IDLE") setShown(state);
    });
  }, []);

  useTauriEvent(events.sessionStateChanged, ({ state }) => {
    setLive(state.kind !== "IDLE");
    if (state.kind !== "IDLE") setShown(state);
  });

  // A second Escape: hold the line on screen while it springs back to full,
  // or the gesture that saved the recording has no feedback at all.
  const kind = shown?.kind ?? null;
  useEffect(() => {
    const previous = previousKind.current;
    previousKind.current = kind;
    if (previous !== "CANCEL_PENDING" || kind !== "RECORDING") return;
    setRefilling(true);
    const handle = window.setTimeout(() => setRefilling(false), readDurationMs("--motion-duration-medium"));
    return () => window.clearTimeout(handle);
  }, [kind]);

  if (!shown) return null;

  const failed = shown.kind === "FAILED";
  const showLine = shown.kind === "CANCEL_PENDING" || refilling;

  return (
    <div
      role="status"
      aria-live="polite"
      className={cn(
        "relative flex h-full w-full items-center overflow-hidden rounded-pill bg-pill-surface text-pill-ink",
        "border-[length:var(--pill-ring-width)] border-pill-ring",
        failed ? "gap-3 px-4" : "flex-row justify-center gap-[var(--pill-content-gap)] px-4",
      )}
    >
      {/* The wash is always rendered and transparent at rest, so a failure
          morphs in rather than a tinted layer popping into existence. */}
      <span
        aria-hidden="true"
        className={cn("absolute inset-0 transition-colors", isFault(shown) ? "bg-pill-danger-wash" : "bg-transparent")}
      />

      <Mark
        size="md"
        className={cn("relative", live && shown.kind === "RECORDING" && "animate-breathe")}
      />

      <PillBody state={shown} showLine={showLine} refilling={refilling} />
    </div>
  );
}

/**
 * WHAT:  The slot beside the mark — bars, the draining line, or the failure
 *        sentence.
 * WHY:   Split out so the switch stays exhaustive over VisibleState["kind"]: a
 *        new variant in the Rust enum becomes a TypeScript error here rather
 *        than a silently blank pill.
 *
 *        ARMING deliberately draws EXACTLY what RECORDING draws. It is normally
 *        under 100ms, and a distinct "Listening…" state for a tenth of a second
 *        is not information, it is a flicker. The capsule arrives already in its
 *        working shape, with the bars at rest, and comes alive as sound reaches
 *        it. If arming genuinely stalls, the device error surfaces as FAILED.
 */
function PillBody({
  state,
  showLine,
  refilling,
}: {
  state: VisibleState;
  showLine: boolean;
  refilling: boolean;
}) {
  if (showLine) {
    // No label beside it: a countdown the user armed himself with a keypress
    // does not need a caption telling him he armed it. It is the bars changing
    // mode — a line standing where they stood.
    return (
      <CountdownLine
        className="relative w-[var(--countdown-line-length)] text-pill-ink"
        remainingMs={state.kind === "CANCEL_PENDING" ? state.remaining_ms : 0}
        state={refilling ? "refilling" : "draining"}
        label="Cancelling — press Escape again to keep the recording"
      />
    );
  }

  switch (state.kind) {
    case "ARMING":
    case "RECORDING":
      return <PillBars className="relative text-pill-signal" />;
    case "CANCEL_PENDING":
      // Unreachable while showLine covers it; kept so the switch is total.
      return null;
    case "FAILED":
      // Two lines, not one and an ellipsis: the whole point of this state is
      // that he finds out why nothing arrived, and a truncated sentence is a
      // failure that failed twice. Tone follows the code, so "still starting
      // up" does not read as a fault.
      return (
        <span
          className={cn(
            "relative min-w-0 flex-1 text-label text-pretty",
            isTransientFailure(state.code) ? "text-pill-ink" : "text-pill-danger",
          )}
        >
          {state.message}
        </span>
      );
    default: {
      // A new SessionState variant lands here as a compile error naming it.
      const unreachable: never = state;
      return unreachable;
    }
  }
}
