/**
 * SOURCE OF TRUTH KEYWORDS: HistoryDetail, DiffText, diffHistoryEntry,
 *   setHistoryReverted, use-original, said-vs-fixed, DiffSegment
 * WHAT:  The panel under the history list for the selected session: what was
 *        said against what was delivered, with every change marked, and one
 *        button to take the original instead.
 * WHY:   A correction nobody can see is a correction nobody can trust. The diff
 *        is computed in Rust on request (one session, not a page of them) and
 *        drawn as runs: removed words struck through, added words highlighted,
 *        unchanged words plain — colour is never the only signal, because the
 *        strike-through carries the removal for anyone who cannot tell the two
 *        tints apart. "Use original" records the choice as a flag rather than
 *        rewriting the text, so it is undoable and the delivered text is never
 *        lost; copy and export then honour it. Only shown when something
 *        actually changed — a panel restating a transcript that was not
 *        touched is noise under every row.
 * WHERE: Rendered by HistoryView below the DataList for the active row.
 */

import { useState } from "react";
import { commands, type AppError, type DiffKind, type SessionSummary } from "@/lib/bindings";
import { unwrapCommand, useCommand } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { Button, ErrorSurface } from "@/components/global";

const SEGMENT_CLASS: Readonly<Record<DiffKind, string>> = {
  SAME: "text-text-secondary",
  REMOVED: "rounded-input bg-danger-soft px-0.5 text-danger line-through",
  ADDED: "rounded-input bg-success-soft px-0.5 text-success",
};

export interface HistoryDetailProps {
  session: SessionSummary;
  /** Called with the new flag once the backend has accepted it. */
  onRevertedChange: (id: string, reverted: boolean) => void;
}

export function HistoryDetail({ session, onRevertedChange }: HistoryDetailProps) {
  const diff = useCommand(() => commands.diffHistoryEntry({ id: session.id }), [session.id]);
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);

  const toggle = async () => {
    setBusy(true);
    setError(null);
    const next = !session.reverted;
    const result = await unwrapCommand(() => commands.setHistoryReverted({ id: session.id, reverted: next }));
    setBusy(false);
    if (result.status === "error") setError(result.error);
    else onRevertedChange(session.id, next);
  };

  return (
    <section aria-label="What was said and what was fixed" className="hairline-t flex max-h-[var(--history-detail-max-height)] shrink-0 flex-col gap-2 overflow-y-auto px-[var(--page-padding-x)] py-3">
      <div className="flex items-center justify-between gap-4">
        <p className="text-label text-text-secondary">
          {session.reverted ? "You chose the original" : "Said → fixed"}
        </p>
        <Button disabled={busy} onClick={() => void toggle()}>
          {session.reverted ? "Use the fixed text" : "Use original"}
        </Button>
      </div>

      {diff.error ? (
        <ErrorSurface size="compact" error={diff.error} onRetry={diff.reload} />
      ) : (
        <p className="text-body">
          {(diff.data ?? []).map((segment, index) => (
            <span key={`${index}-${segment.kind}`} className={cn(SEGMENT_CLASS[segment.kind])}>
              {segment.text}{" "}
            </span>
          ))}
        </p>
      )}
      {error ? <ErrorSurface size="compact" error={error} /> : null}
    </section>
  );
}
