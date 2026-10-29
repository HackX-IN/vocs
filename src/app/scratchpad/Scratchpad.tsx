/**
 * SOURCE OF TRUTH KEYWORDS: Scratchpad, dictation-canvas, organise-on-enter,
 *   scratchpad-insert, brain-dump, scratchpad-dictation, addDictation
 * WHAT:  The floating scratchpad: a page to talk or type a ramble into, which
 *        Return turns into bullets, a task list or a paragraph, and ⌘Return sends
 *        to the app you came from.
 * WHY:   Dictating straight into an app is right for a sentence and wrong for a
 *        brain-dump, where the useful thing happens AFTER the talking. Dictation
 *        needs nothing special to work here — the normal hotkey pastes into the
 *        focused text field, and in this window that is the pad.
 *
 *        But a synthetic paste into our own window only lands while the text
 *        field has keyboard focus, which a click on a button or the window being
 *        shown again takes away. So when the pad is open Rust sends the finished
 *        text as a ScratchpadDictation event instead, and addDictation puts it
 *        at the caret (or the end) whatever has focus.
 *
 *        Return organises and Shift+Return is a newline, because the point of the
 *        window is the organising; ⌘Return is the commit. Organising is
 *        reversible (Undo, in the header so the footer never outgrows the window)
 *        since it rewrites what the person said, and the text is
 *        only cleared after a successful send — a failed paste leaves their words
 *        exactly where they were.
 *
 *        Rust owns the domain: the structuring is organize_text and the send is
 *        scratchpad_insert, so this holds only the draft and which mode is picked.
 * WHERE: Mounted by src/entries/scratchpad.tsx into the `scratchpad` window.
 */

import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Undo2, X } from "lucide-react";
import { commands, events, type AppError, type OrganizeMode } from "@/lib/bindings";
import { unwrapCommand } from "@/lib/ipc";
import { useTauriEvent } from "@/lib/use-event";
import { Button, Keycap, SegmentedControl, ErrorSurface } from "@/components/global";
import { MODE_OPTIONS, readStoredMode, scratchpadSchema, storeMode, type ScratchpadValues } from "./scratchpad-schema";

type Busy = "organise" | "insert" | null;

export function Scratchpad() {
  const { register, setValue, getValues, watch, reset } = useForm<ScratchpadValues>({
    resolver: zodResolver(scratchpadSchema),
    defaultValues: { text: "" },
  });
  const { ref: registerRef, ...textField } = register("text");
  const textarea = useRef<HTMLTextAreaElement | null>(null);

  const [mode, setMode] = useState<OrganizeMode>(readStoredMode);
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<AppError | null>(null);
  /** The text before the last organise, so it can be taken back. */
  const [before, setBefore] = useState<string | null>(null);

  const text = watch("text");
  const hasText = text.trim().length > 0;

  const focusEnd = useCallback(() => {
    const field = textarea.current;
    if (!field) return;
    field.focus();
    field.setSelectionRange(field.value.length, field.value.length);
  }, []);

  // The window is hidden and shown rather than recreated, so it is told it has
  // been brought back by the webview's own focus event.
  useEffect(() => {
    focusEnd();
    window.addEventListener("focus", focusEnd);
    return () => window.removeEventListener("focus", focusEnd);
  }, [focusEnd]);

  const close = useCallback(() => void unwrapCommand(() => commands.hideScratchpad()), []);

  const organise = useCallback(async () => {
    const draft = getValues("text");
    if (draft.trim().length === 0 || busy) return;
    setBusy("organise");
    setError(null);
    const result = await unwrapCommand(() => commands.organizeText({ text: draft, mode }));
    if (result.status === "ok") {
      setBefore(draft);
      setValue("text", result.data, { shouldDirty: true });
    } else {
      setError(result.error);
    }
    setBusy(null);
    focusEnd();
  }, [busy, focusEnd, getValues, mode, setValue]);

  const insert = useCallback(async () => {
    const draft = getValues("text");
    if (draft.trim().length === 0 || busy) return;
    setBusy("insert");
    setError(null);
    const result = await unwrapCommand(() => commands.scratchpadInsert({ text: draft }));
    if (result.status === "ok") {
      // Sent: the pad starts fresh for the next ramble.
      reset({ text: "" });
      setBefore(null);
    } else {
      setError(result.error);
    }
    setBusy(null);
  }, [busy, getValues, reset]);

  // Dictation arrives as an event rather than a paste, so it lands even when
  // the text field has lost keyboard focus. See PadRoutedInjector.
  const addDictation = useCallback(
    (spoken: string) => {
      const field = textarea.current;
      const current = getValues("text");
      const start = field?.selectionStart ?? current.length;
      const end = field?.selectionEnd ?? current.length;
      const head = current.slice(0, start);
      // Two dictations in a row would otherwise run together.
      const gap = head.length > 0 && !/\s$/.test(head) ? " " : "";
      setValue("text", head + gap + spoken + current.slice(end), { shouldDirty: true });
      // Dictating after an organise makes the snapshot stale; Undo would drop it.
      setBefore(null);
      const caret = head.length + gap.length + spoken.length;
      requestAnimationFrame(() => {
        field?.focus();
        field?.setSelectionRange(caret, caret);
      });
    },
    [getValues, setValue],
  );
  useTauriEvent(events.scratchpadDictation, ({ text: spoken }) => addDictation(spoken));

  const undo = useCallback(() => {
    if (before === null) return;
    setValue("text", before, { shouldDirty: true });
    setBefore(null);
    focusEnd();
  }, [before, focusEnd, setValue]);

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    // Never act mid-composition: Return there confirms an input-method candidate.
    if (event.nativeEvent.isComposing) return;
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    } else if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      void insert();
    } else if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void organise();
    }
  }

  return (
    <div className="flex h-full w-full flex-col gap-3 p-4">
      <header data-tauri-drag-region className="flex items-center justify-between">
        <p data-tauri-drag-region className="font-mono text-eyebrow uppercase text-text-tertiary">
          Scratchpad
        </p>
        <div className="flex items-center gap-1">
          {before !== null ? (
            <Button variant="ghost" onClick={undo} aria-label="Undo organise">
              <Undo2 className="size-4" />
              Undo
            </Button>
          ) : null}
          <Button variant="ghost" aria-label="Close scratchpad" onClick={close} className="size-8 px-0">
            <X className="size-4" />
          </Button>
        </div>
      </header>

      <textarea
        {...textField}
        ref={(node) => {
          registerRef(node);
          textarea.current = node;
        }}
        onKeyDown={onKeyDown}
        spellCheck
        aria-label="Scratchpad text"
        placeholder="Talk or type freely. Press Return to organise it, ⌘Return to send it on."
        className="min-h-0 flex-1 resize-none bg-transparent text-[length:var(--text-reading-size)] leading-[var(--text-reading-line)] text-text-primary outline-none placeholder:text-text-tertiary focus-visible:outline-none"
      />

      {error ? <ErrorSurface size="compact" error={error} /> : null}

      <footer className="flex items-center justify-between gap-3">
        <SegmentedControl
          label="Organise as"
          options={MODE_OPTIONS}
          value={mode}
          onChange={(next) => {
            setMode(next);
            storeMode(next);
          }}
        />
        <div className="flex items-center gap-2">
          <Button onClick={() => void organise()} disabled={!hasText || busy !== null}>
            {busy === "organise" ? "Organising…" : "Organise"}
            <Keycap keys={["↵"]} size="sm" />
          </Button>
          <Button variant="primary" onClick={() => void insert()} disabled={!hasText || busy !== null}>
            Send
            <Keycap keys={["⌘", "↵"]} size="sm" />
          </Button>
        </div>
      </footer>
    </div>
  );
}
