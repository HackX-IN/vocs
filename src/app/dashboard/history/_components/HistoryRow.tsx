/**
 * SOURCE OF TRUTH KEYWORDS: HistoryRow, HistoryHeaderRow, SessionSummary, OutcomeBadge,
 *   sessionTitle, DeliveryKind, SessionOutcome, document-tile
 * WHAT:  One row of the Dictations list: a document tile, the dictation's name
 *        over a line of facts, and the time of day at the right — plus the
 *        day-header row that groups them.
 * WHY:   The name is the user's title if they gave one, else the first line of
 *        the text; the facts are words · language · duration. Failed and
 *        orphaned sessions are shown, not hidden — recovering the one that went
 *        wrong is the entire reason history exists (docs/01 §7) — and a failed
 *        row leads its facts with the REASON, rendered VERBATIM from
 *        `error_message`: it is written once where the failure happened, which
 *        knows more about itself than any later reader does, and this file
 *        deliberately holds no code-to-sentence map that would drift from it.
 *        The reason goes on the facts line rather than a third line because the
 *        row height is fixed by the virtualised list. The tile carries the
 *        outcome too (a warning glyph on a danger wash), so a failure is
 *        findable by shape when scanning, not only by reading.
 *
 *        Cancelled sessions cannot appear because the backend destroys them, so
 *        there is no filter for them here.
 * WHERE: The renderRow slot of the Dictations DataList.
 */

import { AlertTriangle, FileText, PencilLine } from "lucide-react";
import { cn } from "@/lib/utils";
import { formatClock, formatCount, formatLanguage, formatTimeOfDay } from "@/lib/format";
import type { SessionOutcome, SessionSummary } from "@/lib/bindings";
import { Eyebrow } from "@/components/global";
import { sessionTitle } from "../session-text";
import type { HistoryEntry } from "../day-groups";

const OUTCOME_TONE: Readonly<Record<SessionOutcome, string>> = {
  DELIVERED: "",
  FAILED: "text-danger",
  ORPHANED: "text-warning",
};

/** The tile carries the outcome too, so a failure is findable by shape while
 *  scanning. A RECOVERED session is not a failure — it is words that were saved
 *  after an interruption — so it gets the warning wash, never the danger one. */
const TILE_TONE: Readonly<Record<SessionOutcome, string>> = {
  DELIVERED: "bg-sunken-strong text-text-secondary",
  FAILED: "bg-danger-soft text-danger",
  ORPHANED: "bg-warning-soft text-warning",
};

const OUTCOME_LABEL: Readonly<Record<SessionOutcome, string>> = {
  DELIVERED: "",
  FAILED: "Failed",
  ORPHANED: "Recovered",
};

export function HistoryRow({ session }: { session: SessionSummary }) {
  const title = sessionTitle(session);

  // The persisted message when there is one. The fallback is only for FAILED:
  // an ORPHANED session was RECOVERED — recovery finished transcribing it and
  // it may well have text — so telling the user it did not finish would
  // contradict the transcript sitting on the line above.
  const reason = session.error_message ?? (session.outcome === "FAILED" ? "The recording did not finish" : null);
  const problem = session.outcome !== "DELIVERED";

  const facts = [
    session.word_count !== null ? `${formatCount(session.word_count)} words` : null,
    session.language !== null ? formatLanguage(session.language) : null,
    session.duration_ms !== null ? formatClock(session.duration_ms) : null,
    session.delivery === "CLIPBOARD_ONLY" ? "Clipboard only" : null,
    session.reverted ? "Original" : null,
  ].filter((entry): entry is string => entry !== null);

  return (
    <div className="flex min-w-0 items-center gap-4">
      <span
        aria-hidden="true"
        className={cn("flex size-10 shrink-0 items-center justify-center rounded-input", TILE_TONE[session.outcome])}
      >
        {problem ? <AlertTriangle className="size-5" /> : <FileText className="size-5" />}
      </span>

      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="flex min-w-0 items-center gap-2">
          <span className={cn("truncate text-body", title ? "text-text-primary" : "text-text-tertiary")}>
            {title ?? "Nothing was said"}
          </span>
          {session.edited_at_ms !== null ? (
            <PencilLine aria-label="Edited" className="size-3.5 shrink-0 text-text-tertiary" />
          ) : null}
          {problem ? (
            <span className={cn("shrink-0 text-caption", OUTCOME_TONE[session.outcome])}>
              {OUTCOME_LABEL[session.outcome]}
            </span>
          ) : null}
        </span>

        {/* The reason truncates; the facts do not. Duration is what docs/04 §9
            calls "what was lost" — a long failure message must never be the
            thing that pushes it off the row. */}
        <span className="flex min-w-0 items-baseline gap-1 text-caption text-text-tertiary">
          {reason ? <span className={cn("truncate", OUTCOME_TONE[session.outcome])}>{reason}</span> : null}
          {facts.length > 0 ? (
            <span className="shrink-0">
              {reason ? "· " : null}
              {facts.join(" · ")}
            </span>
          ) : null}
        </span>
      </div>

      <time
        dateTime={new Date(session.started_at_ms).toISOString()}
        className="shrink-0 font-mono text-caption tabular-nums text-text-tertiary"
      >
        {formatTimeOfDay(session.started_at_ms)}
      </time>
    </div>
  );
}

/** The day header. It fills a full row slot so the virtualiser stays uniform;
 *  the label sits at the bottom of it, which is what makes it read as spacing. */
export function HistoryHeaderRow({ entry }: { entry: Extract<HistoryEntry, { kind: "header" }> }) {
  return (
    <div className="flex h-full w-full items-end justify-between pb-2">
      <Eyebrow>{entry.label}</Eyebrow>
      <span className="font-mono text-eyebrow tabular-nums text-text-tertiary">{entry.count}</span>
    </div>
  );
}
