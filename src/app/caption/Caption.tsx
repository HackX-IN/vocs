/**
 * SOURCE OF TRUTH KEYWORDS: Caption, CaptionBody, captionChanged, live-captions, preview-pill,
 *   caption-hud, heard-vs-fixed, tailOf
 * WHAT:  The caption HUD: gradient text that rises out of the pill while you speak,
 *        showing what Vocs HEARD and what it will WRITE — and, after you stop, the
 *        finished text for a moment before it is pasted, with a draining line and
 *        "Esc to keep it out".
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
 *        IT HAS NO BOX. The window stands directly above the pill and the text is
 *        painted as a gradient — the accent where it leaves the pill, ink above,
 *        dissolving into nothing at the top — so it reads as coming OUT of the
 *        pill rather than as a second object beside it. A dark vignette behind
 *        the words keeps them legible over any wallpaper or document, in both
 *        colour schemes, because there is no panel to guarantee a background. The
 *        window is a fixed size the text is laid out INTO (docs/04 §15), never one
 *        it is fitted to.
 * WHERE: Mounted by src/entries/caption.tsx into the `caption` window. Sizes come
 *        from tokens.css, which src-tauri/src/tray.rs reads.
 */

import { useRef, useState } from "react";
import { events, type Caption as CaptionState } from "@/lib/bindings";
import { useTauriEvent } from "@/lib/use-event";
import { cn } from "@/lib/utils";
import { CountdownLine } from "@/components/global";

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
      className="relative flex h-full w-full flex-col items-center justify-end px-8 pb-1 text-center"
    >
      {/* Legibility without a panel: a dark vignette rooted at the pill, and a
          warm accent bloom on top of it where the words leave the pill. */}
      <span
        aria-hidden="true"
        className="pointer-events-none absolute inset-0"
        style={{
          background:
            "radial-gradient(ellipse 70% 90% at 50% 100%, color-mix(in srgb, var(--accent) 22%, transparent), transparent 70%), radial-gradient(ellipse 90% 110% at 50% 100%, color-mix(in srgb, var(--pill-surface) 85%, transparent), transparent 75%)",
        }}
      />
      <div className="caption-fade relative flex w-full flex-col items-center justify-end gap-2 pt-[var(--caption-fade-size)]">
        <CaptionBody caption={shown} previewId={previewId} />
      </div>
    </div>
  );
}

const TEXT_CLASS = "caption-gradient text-[length:var(--caption-text-size)] leading-[var(--caption-text-line)] font-medium text-balance";

function CaptionBody({ caption, previewId }: { caption: CaptionState; previewId: number }) {
  switch (caption.kind) {
    case "LIVE": {
      const differs = caption.heard.trim() !== caption.fixed.trim();
      return (
        <>
          {differs ? (
            <p className="text-label text-pill-ink/55">
              <span className="font-mono text-eyebrow uppercase tracking-[0.1em]">Heard </span>
              {tailOf(caption.heard, LIVE_TAIL_CHARS)}
            </p>
          ) : null}
          <p className={TEXT_CLASS}>{tailOf(caption.fixed, LIVE_TAIL_CHARS)}</p>
        </>
      );
    }
    case "PREVIEW":
      return (
        <>
          <p className={cn(TEXT_CLASS, "line-clamp-3")}>{caption.text}</p>
          <div className="flex items-center gap-3 text-caption text-pill-ink/60">
            {/* Keyed so each preview's line is a fresh animation that starts full:
                the line is created ONCE per preview and never restarted by a tick
                (see CountdownLine). The duration is the hold time Rust chose. */}
            <CountdownLine
              key={previewId}
              className="w-24 text-pill-signal"
              remainingMs={caption.hold_ms}
              state="draining"
              label="Time left before pasting"
            />
            <span>Pasting in a moment · Esc to keep it out</span>
          </div>
        </>
      );
    default: {
      // A new Caption variant lands here as a compile error naming it.
      const unreachable: never = caption;
      return unreachable;
    }
  }
}
