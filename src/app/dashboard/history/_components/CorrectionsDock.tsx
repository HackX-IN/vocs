/**
 * SOURCE OF TRUTH KEYWORDS: CorrectionsDock, DiffText, diffHistoryEntry,
 *   setHistoryReverted, use-original, said-vs-fixed, DiffSegment, floating-dock
 * WHAT:  The "said vs fixed" overlay floating over the foot of the dictation
 *        editor: a one-line pill saying how many words were changed, which opens
 *        into what was said against what was delivered, with every change marked
 *        and one button to take the original instead.
 * WHY:   It is an overlay, not a card in the page, because the page is a
 *        document and the document should end where the words end. Collapsed it
 *        is a pill that never covers the text; opened it is a panel over the
 *        text that the user dismisses, so reading it never moves what they are
 *        editing. The diff is computed in Rust on request (one session, not a
 *        page of them) and drawn as runs: removed words struck through, added
 *        words highlighted, unchanged words plain — colour is never the only
 *        signal, because the strike-through carries the removal for anyone who
 *        cannot tell the two tints apart. "Use original" records the choice as a
 *        flag rather than rewriting the text, so it is undoable and the
 *        delivered text is never lost; copy and export then honour it. Only
 *        shown when something actually changed — a pill restating a transcript
 *        that was not touched is noise under every dictation.
 *
 *        The caller owns the moment of toggling: the editor has to save any
 *        words it is holding BEFORE the flag flips (a flip would otherwise swap
 *        the text out from under the cursor), so this overlay asks through
 *        `beforeToggle` and waits.
 * WHERE: Rendered by DictationEditor over the text, for a corrected session.
 */

import { useState } from "react";
import { ChevronDown, Sparkles } from "lucide-react";
import { commands, type AppError, type DiffKind, type SessionSummary } from "@/lib/bindings";
import { unwrapCommand, useCommand } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { Button, ErrorSurface } from "@/components/global";

const SEGMENT_CLASS: Readonly<Record<DiffKind, string>> = {
  SAME: "text-text-secondary",
  REMOVED: "rounded-input bg-danger-soft px-0.5 text-danger line-through",
  ADDED: "rounded-input bg-success-soft px-0.5 text-success",
};

export interface CorrectionsDockProps {
  session: SessionSummary;
  /** Awaited before the flag changes — flush unsaved edits here. */
  beforeToggle: () => Promise<void>;
  /** Called with the new flag once the backend has accepted it. */
  onRevertedChange: (id: string, reverted: boolean) => void;
}

export function CorrectionsDock({ session, beforeToggle, onRevertedChange }: CorrectionsDockProps) {
  const diff = useCommand(() => commands.diffHistoryEntry({ id: session.id }), [session.id, session.edited_at_ms]);
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);

  const toggle = async () => {
    setBusy(true);
    setError(null);
    await beforeToggle();
    const next = !session.reverted;
    const result = await unwrapCommand(() => commands.setHistoryReverted({ id: session.id, reverted: next }));
    setBusy(false);
    if (result.status === "error") setError(result.error);
    else onRevertedChange(session.id, next);
  };

  const [open, setOpen] = useState(false);
  const changes = (diff.data ?? []).filter((segment) => segment.kind === "ADDED").length;
  const title = session.reverted ? "You chose the original" : "Said → fixed";

  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-0 flex justify-center px-[var(--page-padding-x)] pb-[var(--dock-offset)]">
      <section
        aria-label="What was said and what was fixed"
        className="material-elevated pointer-events-auto flex w-full max-w-[var(--dock-max-width)] flex-col gap-3 rounded-card px-4 py-3"
      >
        <div className="flex items-center gap-3">
          <button
            type="button"
            aria-expanded={open}
            onClick={() => setOpen((value) => !value)}
            className="flex min-w-0 flex-1 items-center gap-2 text-left text-label text-text-secondary"
          >
            <Sparkles className="size-4 shrink-0 text-accent" aria-hidden="true" />
            <span className="truncate">
              {title}
              {changes > 0 ? ` · ${changes} ${changes === 1 ? "change" : "changes"}` : ""}
            </span>
            <ChevronDown className={cn("size-4 shrink-0 transition-transform", open && "rotate-180")} aria-hidden="true" />
          </button>
          <Button disabled={busy} onClick={() => void toggle()}>
            {session.reverted ? "Use the fixed text" : "Use original"}
          </Button>
        </div>
        {open ? (
          diff.error ? (
            <ErrorSurface size="compact" error={diff.error} onRetry={diff.reload} />
          ) : (
            <p data-selectable="true" className="max-h-[40vh] overflow-y-auto text-body leading-relaxed">
              {(diff.data ?? []).map((segment, index) => (
                <span key={`${index}-${segment.kind}`} className={cn(SEGMENT_CLASS[segment.kind])}>
                  {segment.text}{" "}
                </span>
              ))}
            </p>
          )
        ) : null}
        {error ? <ErrorSurface size="compact" error={error} /> : null}
      </section>
    </div>
  );
}
