/**
 * SOURCE OF TRUTH KEYWORDS: Caption, CaptionBody, captionChanged, live-captions, preview-pill,
 *   caption-hud, heard-vs-fixed, tailOf
 * WHAT:  The caption HUD: a small dark panel that shows, while you speak, what Vocs
 *        HEARD and what it will WRITE — and, after you stop, the finished text for
 *        a moment before it is pasted, with a draining line and "Esc to keep it
 *        out".
 * WHY:   One window for two features, because they are the same object at two
 *        moments of a dictation (types/caption.rs). It renders exactly the last
 *        caption Rust pushed and holds no domain state: there is no polling and no
 *        second copy of the truth, and the switch over the caption's `kind` is
 *        exhaustive with a `never` assertion so a new variant is a compile error
 *        here rather than a blank window.
 *
 *        IT NEVER BLANKS ITSELF. Rust hides the window when the caption goes away
 *        and the same news reaches this webview over IPC; the two do not land on
 *        the same frame, so a view that cleared on `null` would flash an empty
 *        panel at the end of every caption. It keeps the last thing it was told
 *        and lets the window be taken away — the rule the pill lives under.
 *
 *        LIVE shows the END of the text, not the start: while you are speaking,
 *        the words that matter are the newest, so a long transcript is cut from
 *        the front (`tailOf`) rather than from the back. PREVIEW shows the start,
 *        because that is what you recognise a finished paragraph by. "Heard" is
 *        shown only when it differs from "fixed" — repeating identical text is
 *        noise.
 *
 *        Dark in both colour schemes, like the pill, for the same reason: it
 *        floats over arbitrary content. The window is a fixed size the text is
 *        laid out INTO (docs/04 §15), never one it is fitted to.
 * WHERE: Mounted by src/entries/caption.tsx into the `caption` window. Sizes come
 *        from tokens.css, which src-tauri/src/tray.rs reads.
 */

import { useRef, useState } from "react";
import { events, type Caption as CaptionState } from "@/lib/bindings";
import { useTauriEvent } from "@/lib/use-event";
import { cn } from "@/lib/utils";
import { CountdownLine, Eyebrow } from "@/components/global";

/** How many characters of a live caption are shown — about two lines of the HUD. */
const LIVE_TAIL_CHARS = 150;

/** The last `max` characters of `text`, cut at a word and marked with an ellipsis. */
export function tailOf(text: string, max: number): string {
  const trimmed = text.trim();
  if (trimmed.length <= max) return trimmed;
  const cut = trimmed.slice(trimmed.length - max);
  const firstSpace = cut.indexOf(" ");
  return `…${firstSpace > 0 ? cut.slice(firstSpace + 1) : cut}`;
}

export function Caption() {
  /** The last caption worth drawing. Never cleared — see the module WHY. */
  const [shown, setShown] = useState<CaptionState | null>(null);
  /** Distinguishes one preview from the next, so its line restarts from full. */
  const previews = useRef(0);
  const [previewId, setPreviewId] = useState(0);

  useTauriEvent(events.captionChanged, ({ caption }) => {
    if (!caption) return;
    if (caption.kind === "PREVIEW") {
      previews.current += 1;
      setPreviewId(previews.current);
    }
    setShown(caption);
  });

  if (!shown) return null;

  return (
    <div
      role="status"
      aria-live="polite"
      className={cn(
        "flex h-full w-full flex-col gap-2 overflow-hidden rounded-[var(--radius-caption)] bg-pill-surface px-5 py-4 text-pill-ink",
        "border-[length:var(--pill-ring-width)] border-pill-ring",
      )}
    >
      <CaptionBody caption={shown} previewId={previewId} />
    </div>
  );
}

function CaptionBody({ caption, previewId }: { caption: CaptionState; previewId: number }) {
  switch (caption.kind) {
    case "LIVE": {
      const differs = caption.heard.trim() !== caption.fixed.trim();
      return (
        <>
          <header className="flex items-center gap-2">
            <span aria-hidden="true" className="size-1.5 animate-breathe rounded-full bg-pill-signal" />
            <Eyebrow className="text-pill-ink/60">Live</Eyebrow>
          </header>
          {differs ? (
            <p className="text-label text-pill-ink/55">
              <span className="font-mono text-eyebrow uppercase tracking-[0.1em]">Heard </span>
              {tailOf(caption.heard, LIVE_TAIL_CHARS)}
            </p>
          ) : null}
          <p className="text-body leading-snug text-pill-ink">{tailOf(caption.fixed, LIVE_TAIL_CHARS)}</p>
        </>
      );
    }
    case "PREVIEW":
      return (
        <>
          <header className="flex items-center justify-between gap-3">
            <Eyebrow className="text-pill-ink/60">Pasting in a moment</Eyebrow>
            <span className="text-caption text-pill-ink/60">Esc to keep it out</span>
          </header>
          {/* Keyed so each preview's line is a fresh animation that starts full:
              the line is created ONCE per preview and never restarted by a tick
              (see CountdownLine). The duration is the hold time Rust chose. */}
          <CountdownLine
            key={previewId}
            className="text-pill-signal"
            remainingMs={caption.hold_ms}
            state="draining"
            label="Time left before pasting"
          />
          <p className="line-clamp-3 text-body leading-snug text-pill-ink">{caption.text}</p>
        </>
      );
    default: {
      // A new Caption variant lands here as a compile error naming it.
      const unreachable: never = caption;
      return unreachable;
    }
  }
}
