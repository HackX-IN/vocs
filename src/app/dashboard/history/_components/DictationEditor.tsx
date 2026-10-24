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
 *        It behaves like a notepad: ⌘F finds in the text (⌘G / ⇧⌘G for the next and
 *        previous match), ⌘S saves now, and undo, redo, cut, copy, paste and select
 *        all come from the application menu's Edit items.
 *
 *        It draws its OWN header (the shell is told not to): back, save status
 *        and actions form a toolbar at the top of the reading column, directly
 *        above the title they act on — not up in the window bar, where they
 *        floated a full screen-edge away from the thing being edited. On this
 *        page the dictation's title is the heading, and a second "Dictations"
 *        above it would be wrong twice.
 * WHERE: Rendered by HistoryView when the route carries a session id. Header
 *        actions call copy_text / delete_history_entry; saves go through
 *        update_history_entry.
 */

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { ArrowLeft, Calendar, Check, Clock, Copy, Globe, TextCursorInput, Trash2 } from "lucide-react";
import { commands, type AppError, type SessionSummary } from "@/lib/bindings";
import { unwrapCommand, useCommand } from "@/lib/ipc";
import { useAutosave, type AutosaveStatus } from "@/lib/use-autosave";
import { formatClock, formatCount, formatDateTime, formatLanguage } from "@/lib/format";
import { readDurationMs } from "@/lib/motion";
import { findMatches, offsetTopOf } from "@/lib/textarea-find";
import { cn } from "@/lib/utils";
import { Button, Chip, ConfirmButton, ErrorSurface, ScrollArea, Skeleton, WindowBar } from "@/components/global";
import { effectiveText, wasCorrected } from "../session-text";
import { CorrectionsDock } from "./CorrectionsDock";
import { FindBar } from "./FindBar";

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
  const [actionError, setActionError] = useState<AppError | null>(null);
  const copyTimer = useRef(0);
  const [finding, setFinding] = useState(false);
  const [findQuery, setFindQuery] = useState("");
  const [findIndex, setFindIndex] = useState(0);

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

  const matches = useMemo(() => findMatches(draft.text, findQuery), [draft.text, findQuery]);

  /** Selects match `index` in the text and scrolls the page to it. */
  const goToMatch = useCallback(
    (index: number) => {
      const textarea = bodyRef.current;
      const start = matches[index];
      if (!textarea || start === undefined) return;
      setFindIndex(index);
      const scroller = textarea.closest<HTMLElement>("[data-scroll-area]");
      const top = textarea.offsetTop + offsetTopOf(textarea, start);
      if (scroller) scroller.scrollTo({ top: Math.max(top - scroller.clientHeight / 3, 0), behavior: "smooth" });
      // Focus is moved to the text so the match shows as a selection.
      textarea.focus({ preventScroll: true });
      textarea.setSelectionRange(start, start + findQuery.trim().length);
    },
    [bodyRef, findQuery, matches],
  );

  const stepMatch = useCallback(
    (delta: 1 | -1) => {
      if (matches.length === 0) return;
      goToMatch((findIndex + delta + matches.length) % matches.length);
    },
    [findIndex, goToMatch, matches.length],
  );

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const command = event.metaKey || event.ctrlKey;
      if (event.key === "Escape") {
        event.preventDefault();
        // Escape peels one layer: the find bar first, then the page.
        if (finding) setFinding(false);
        else void goBack();
      } else if (command && event.key.toLowerCase() === "f") {
        event.preventDefault();
        setFinding(true);
      } else if (command && event.key.toLowerCase() === "g") {
        event.preventDefault();
        stepMatch(event.shiftKey ? -1 : 1);
      } else if (command && event.key.toLowerCase() === "s") {
        // Everything already saves itself; ⌘S just does it now and says so.
        event.preventDefault();
        void autosave.flush();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [autosave, finding, goBack, stepMatch]);

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
    const result = await unwrapCommand(() => commands.deleteHistoryEntry({ id: session.id }));
    if (result.status === "error") {
      setActionError(result.error);
      return;
    }
    onDeleted(session.id);
  };

  const words = draft.text.trim().length === 0 ? 0 : draft.text.trim().split(/\s+/).length;

  return (
    <div className="page-enter relative flex min-h-0 flex-1 flex-col">
      {/* No controls up here: the window bar is only the drag handle and the
          traffic lights. The editor's own toolbar sits in the column, directly
          above the title it acts on. */}
      <WindowBar />

      {finding ? (
        <FindBar
          query={findQuery}
          onQueryChange={(query) => {
            setFindQuery(query);
            setFindIndex(0);
          }}
          current={matches.length === 0 ? null : Math.min(findIndex, matches.length - 1)}
          total={matches.length}
          onNext={() => stepMatch(1)}
          onPrevious={() => stepMatch(-1)}
          onClose={() => setFinding(false)}
        />
      ) : null}

      <ScrollArea contentClassName="px-[var(--page-padding-x)] pb-24">
        <article className="mx-auto flex w-full max-w-[var(--content-max-width)] flex-col gap-5">
          <div className="grid grid-cols-[1fr_auto_1fr] items-center gap-2">
            <Button
              variant="ghost"
              aria-label="Back to dictations"
              onClick={() => void goBack()}
              className="size-8 justify-self-start px-0"
            >
              <ArrowLeft className="size-4" />
            </Button>
            <span
              role="status"
              className={cn("text-caption", autosave.status === "error" ? "text-danger" : "text-text-tertiary")}
            >
              {STATUS_LABEL[autosave.status]}
            </span>
            <div className="flex items-center gap-1 justify-self-end">
              <Button variant="ghost" onClick={() => void copy()} aria-label="Copy text">
                {copied ? <Check className="size-4 text-success" /> : <Copy className="size-4" />}
                {copied ? "Copied" : "Copy"}
              </Button>
              <ConfirmButton idleVariant="ghost" confirmLabel="Delete it" onConfirm={remove} aria-label="Delete dictation">
                <Trash2 className="size-4" />
              </ConfirmButton>
            </div>
          </div>

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
