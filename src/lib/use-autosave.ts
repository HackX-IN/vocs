/**
 * SOURCE OF TRUTH KEYWORDS: useAutosave, AutosaveStatus, debounced-save, flush-on-leave,
 *   autosave-delay, last-write-wins
 * WHAT:  Saves a value shortly after it stops changing, one write at a time, and
 *        reports whether what is on screen is what is stored.
 * WHY:   An editor that asks to be saved loses work, and one that saves per
 *        keystroke races itself — so: debounced, SERIALISED, and flushed when
 *        the user leaves. Serialised means a second write waits for the first
 *        rather than overtaking it, and the loop re-checks after every write
 *        because the user kept typing while it was in flight; the last value
 *        always wins and none is skipped. The status is honest about all four
 *        states a writer can be in — nothing to save, waiting to save, saving,
 *        failed — because "Saved" shown while a write is in flight is the lie
 *        that costs someone a paragraph. A failed save keeps the value dirty and
 *        retries on the next change; it never discards the draft.
 *
 *        The save function is held in a ref so passing an inline arrow does not
 *        restart the debounce every render.
 * WHERE: The dictation editor. Delay is --autosave-delay.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { AppError } from "@/lib/bindings";
import { readDurationMs } from "@/lib/motion";

export type AutosaveStatus = "saved" | "dirty" | "saving" | "error";

export interface UseAutosaveOptions<T> {
  value: T;
  /** What is already stored, so opening a document does not "save" it. */
  initial: T;
  /** Resolves to an AppError, or null on success. Must not throw. */
  save: (value: T) => Promise<AppError | null>;
  equals?: (a: T, b: T) => boolean;
}

export interface Autosave {
  status: AutosaveStatus;
  error: AppError | null;
  /** Writes now if anything is unsaved, and resolves when nothing is in flight. */
  flush: () => Promise<void>;
}

export function useAutosave<T>({ value, initial, save, equals = Object.is }: UseAutosaveOptions<T>): Autosave {
  const [status, setStatus] = useState<AutosaveStatus>("saved");
  const [error, setError] = useState<AppError | null>(null);

  const latest = useRef(value);
  const stored = useRef(initial);
  const writing = useRef<Promise<void> | null>(null);
  const timer = useRef(0);
  const saveRef = useRef(save);
  const equalsRef = useRef(equals);
  saveRef.current = save;
  equalsRef.current = equals;
  latest.current = value;

  const run = useCallback((): Promise<void> => {
    if (writing.current) return writing.current;

    const loop = async (): Promise<void> => {
      while (!equalsRef.current(latest.current, stored.current)) {
        const attempt = latest.current;
        setStatus("saving");
        const failure = await saveRef.current(attempt);
        if (failure) {
          setError(failure);
          setStatus("error");
          return;
        }
        stored.current = attempt;
        setError(null);
      }
      setStatus("saved");
    };

    writing.current = loop().finally(() => {
      writing.current = null;
    });
    return writing.current;
  }, []);

  useEffect(() => {
    window.clearTimeout(timer.current);
    if (equalsRef.current(value, stored.current)) {
      if (!writing.current) setStatus("saved");
      return;
    }
    setStatus((current) => (current === "saving" ? current : "dirty"));
    timer.current = window.setTimeout(() => void run(), readDurationMs("--autosave-delay"));
    return () => window.clearTimeout(timer.current);
  }, [value, run]);

  // Leaving with unsaved words writes them; it does not ask.
  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
      void run();
    },
    [run],
  );

  const flush = useCallback(async () => {
    window.clearTimeout(timer.current);
    await run();
  }, [run]);

  return { status, error, flush };
}
