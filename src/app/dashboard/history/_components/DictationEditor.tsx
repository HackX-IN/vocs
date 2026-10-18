/**
 * SOURCE OF TRUTH KEYWORDS: DictationEditor, DictationBody, edit_dictation, updateHistoryEntry,
 *   getHistoryEntry, notepad-view, autosave-editor, said-vs-fixed
 * WHAT:  A dictation opened as a page you can work on: a title, a row of fact
 *        chips, and the text as an editable document that saves itself.
 * WHY:   Dictating produces a first draft, and a first draft is for editing. So
 *        a dictation opens like a note — large title, facts beneath, the words
 *        as a reading column — not as a row with a detail panel. Everything the
 *        user types is saved (useAutosave) with an honest status in the header,
 *        and leaving flushes it; there is no Save button to forget.
 *
 *        THE EDIT GOES TO `final_text`, never `raw_text`: what was said stays a
 *        record, which is what keeps the said-vs-fixed card meaningful after the
 *        user has rewritten half the sentence. Saving also clears "use
 *        original", server-side, so a stale flag can never hide the edit from
 *        copy and export.
 *
 *        Split in two: the outer component fetches, the inner one owns the
 *        draft and is keyed on the session AND a revision, so a "use original"
 *        toggle remounts it with the new effective text instead of fighting the
 *        user's caret with an effect.
 *
 *        It draws its OWN header (the shell is told not to): back, save status
 *        and actions sit in the traffic-light row, because on this page the
 *        dictation's title is the heading and a second "Dictations" above it
 *        would be wrong twice.
 * WHERE: Rendered by HistoryView when the route carries a session id. Header
 *        actions call copy_text / delete_history_entry; saves go through
 *        update_history_entry.
 */

import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { ArrowLeft, Calendar, Check, Clock, Copy, Globe, TextCursorInput, Trash2 } from "lucide-react";
import { commands, type AppError, type SessionSummary } from "@/lib/bindings";
import { unwrapCommand, useCommand } from "@/lib/ipc";
import { useAutosave, type AutosaveStatus } from "@/lib/use-autosave";
import { formatClock, formatCount, formatDateTime, formatLanguage } from "@/lib/format";
import { readDurationMs } from "@/lib/motion";
import { cn } from "@/lib/utils";
import { Button, Chip, ErrorSurface, ScrollArea, Skeleton, WindowBar } from "@/components/global";
import { effectiveText, wasCorrected } from "../session-text";
import { CorrectionsDock } from "./CorrectionsDock";

const STATUS_LABEL: Readonly<Record<AutosaveStatus, string>> = {
  saved: "Saved",
  dirty: "Editing…",
  saving: "Saving…",
  error: "Not saved",
};

/** Grows a textarea to its content, so the page — not the field — scrolls. */
function useAutoGrow(value: string) {
  const ref = useRef<HTMLTextAreaElement>(null);
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    element.style.height = "auto";
    element.style.height = `${element.scrollHeight}px`;
  }, [value]);
  return ref;
}

export interface DictationEditorProps {
  id: string;
  onBack: () => void;
  /** The saved row, so the list behind the editor shows the new name and text. */
  onChanged: (session: SessionSummary) => void;
  onDeleted: (id: string) => void;
}

export function DictationEditor({ id, onBack, onChanged, onDeleted }: DictationEditorProps) {
  const session = useCommand(() => commands.getHistoryEntry({ id }), [id]);
  // Bumped when "use original" flips: the draft is re-seeded from the new text.
  const [revision, setRevision] = useState(0);

  if (session.error) {
    return (
      <div className="flex flex-1 flex-col gap-4 px-[var(--page-padding-x)] pt-[var(--page-header-height)]">
        <Button onClick={onBack} className="w-fit">
          <ArrowLeft className="size-4" /> Back
        </Button>
        <ErrorSurface error={session.error} onRetry={session.reload} />
      </div>
    );
  }

  if (!session.data) {
    return (
      <ScrollArea contentClassName="px-[var(--page-padding-x)]">
        <Skeleton rows={4} />
      </ScrollArea>
    );
  }

  return (
    <DictationBody
      key={`${session.data.id}-${revision}`}
      session={session.data}
      onBack={onBack}
      onChanged={onChanged}
      onDeleted={onDeleted}
      onRevertedChange={(reverted) => {
        onChanged({ ...session.data!, reverted });
        session.reload();
        setRevision((value) => value + 1);
      }}
    />
  );
}

interface Draft {
  title: string;
  text: string;
}

function DictationBody({
  session,
  onBack,
  onChanged,
  onDeleted,
  onRevertedChange,
}: {
  session: SessionSummary;
  onBack: () => void;
  onChanged: (session: SessionSummary) => void;
  onDeleted: (id: string) => void;
  onRevertedChange: (reverted: boolean) => void;
}) {
  const initial: Draft = { title: session.title ?? "", text: effectiveText(session) ?? "" };
  const [draft, setDraft] = useState<Draft>(initial);
  const [copied, setCopied] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [actionError, setActionError] = useState<AppError | null>(null);
  const copyTimer = useRef(0);

  const autosave = useAutosave<Draft>({
    value: draft,
    initial,
    equals: (a, b) => a.title === b.title && a.text === b.text,
    save: async (value) => {
      const result = await unwrapCommand(() =>
        commands.updateHistoryEntry({ id: session.id, title: value.title, text: value.text }),
      );
      if (result.status === "error") return result.error;
      onChanged(result.data);
      return null;
    },
  });

  const titleRef = useAutoGrow(draft.title);
  const bodyRef = useAutoGrow(draft.text);

  useEffect(() => () => window.clearTimeout(copyTimer.current), []);

  const goBack = useCallback(async () => {
    await autosave.flush();
    onBack();
  }, [autosave, onBack]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        void goBack();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [goBack]);

  const copy = async () => {
    const result = await unwrapCommand(() => commands.copyText({ text: draft.text }));
    if (result.status === "error") {
      setActionError(result.error);
      return;
    }
    setCopied(true);
    window.clearTimeout(copyTimer.current);
    copyTimer.current = window.setTimeout(() => setCopied(false), readDurationMs("--feedback-hold"));
  };

  const remove = async () => {
    if (!confirmingDelete) {
      setConfirmingDelete(true);
      return;
    }
    const result = await unwrapCommand(() => commands.deleteHistoryEntry({ id: session.id }));
    if (result.status === "error") {
      setActionError(result.error);
      setConfirmingDelete(false);
      return;
    }
    onDeleted(session.id);
  };

  const words = draft.text.trim().length === 0 ? 0 : draft.text.trim().split(/\s+/).length;

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <WindowBar
        leading={
          <Button variant="ghost" aria-label="Back to dictations" onClick={() => void goBack()} className="size-8 px-0">
            <ArrowLeft className="size-4" />
          </Button>
        }
        title={
          <span
            role="status"
            className={cn("text-caption", autosave.status === "error" ? "text-danger" : "text-text-tertiary")}
          >
            {STATUS_LABEL[autosave.status]}
          </span>
        }
        trailing={
          <>
            <Button variant="ghost" onClick={() => void copy()} aria-label="Copy text">
              {copied ? <Check className="size-4 text-success" /> : <Copy className="size-4" />}
              {copied ? "Copied" : "Copy"}
            </Button>
            <Button
              variant={confirmingDelete ? "danger" : "ghost"}
              onClick={() => void remove()}
              onBlur={() => setConfirmingDelete(false)}
              aria-label="Delete dictation"
            >
              <Trash2 className="size-4" />
              {confirmingDelete ? "Delete it" : null}
            </Button>
          </>
        }
      />

      <ScrollArea contentClassName="px-[var(--page-padding-x)] pb-24">
        <article className="mx-auto flex w-full max-w-[var(--content-max-width)] flex-col gap-5 pt-4">
          <textarea
            ref={titleRef}
            rows={1}
            value={draft.title}
            maxLength={200}
            placeholder={firstWords(draft.text) ?? "Untitled dictation"}
            aria-label="Title"
            onChange={(event) => setDraft((value) => ({ ...value, title: event.target.value.replace(/\n/g, " ") }))}
            onKeyDown={(event) => {
              // Enter in a title moves to the body — it never inserts a newline.
              if (event.key === "Enter") {
                event.preventDefault();
                bodyRef.current?.focus();
              }
            }}
            className="font-display text-title w-full resize-none bg-transparent text-text-primary outline-none placeholder:text-text-tertiary focus-visible:outline-none"
          />

          <div className="flex flex-wrap items-center gap-2">
            <Chip icon={<Calendar className="size-3.5" />}>{formatDateTime(session.started_at_ms)}</Chip>
            {session.duration_ms !== null ? (
              <Chip icon={<Clock className="size-3.5" />}>{formatClock(session.duration_ms)}</Chip>
            ) : null}
            {session.language !== null ? (
              <Chip icon={<Globe className="size-3.5" />}>{formatLanguage(session.language)}</Chip>
            ) : null}
            <Chip icon={<TextCursorInput className="size-3.5" />}>{formatCount(words)} words</Chip>
          </div>

          {session.error_message ? <ErrorSurface size="compact" error={failureOf(session)} /> : null}

          <textarea
            ref={bodyRef}
            value={draft.text}
            placeholder="Nothing was transcribed. Type here to add words of your own."
            aria-label="Dictation text"
            spellCheck
            onChange={(event) => setDraft((value) => ({ ...value, text: event.target.value }))}
            className="min-h-48 w-full resize-none bg-transparent text-[length:var(--text-reading-size)] leading-[var(--text-reading-line)] text-text-primary outline-none placeholder:text-text-tertiary focus-visible:outline-none"
          />

          {autosave.error ? <ErrorSurface size="compact" error={autosave.error} onRetry={() => void autosave.flush()} /> : null}
          {actionError ? <ErrorSurface size="compact" error={actionError} /> : null}

        </article>
      </ScrollArea>

      {wasCorrected(session) ? (
        <CorrectionsDock
          session={session}
          beforeToggle={autosave.flush}
          onRevertedChange={(_, reverted) => onRevertedChange(reverted)}
        />
      ) : null}
    </div>
  );
}

/** The placeholder title: the opening words, so an unnamed dictation still has a name. */
function firstWords(text: string): string | null {
  const words = text.trim().split(/\s+/).slice(0, 6).join(" ");
  return words.length > 0 ? words : null;
}

/** A failed session's stored reason, in the one shape ErrorSurface renders. */
function failureOf(session: SessionSummary): AppError {
  return {
    code: "INTERNAL",
    message: session.error_message ?? "This dictation did not finish.",
    recoverable: false,
    action: null,
    detail: null,
  };
}
