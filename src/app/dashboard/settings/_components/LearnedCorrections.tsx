/**
 * SOURCE OF TRUTH KEYWORDS: LearnedCorrections, LearnedCorrection, listLearned,
 *   setLearnedAuto, deleteLearned, clearLearned, personal_mistake_dictionary
 * WHAT:  The fixes Vocs has learned from you: each one's phrase and its
 *        correction, how many times you kept or undid it, whether it now applies
 *        itself, and the controls to switch it on or off, delete it, or forget
 *        everything.
 * WHY:   A feature that rewrites your words by itself is only trustworthy if you
 *        can see exactly what it has learned. A row says what it will do, in the
 *        words you spoke, and why ("kept 3 times, never undone"). The checkbox is
 *        "applies itself": on for a fix that earned it, but also yours to turn on
 *        early or off for good — a choice made here is never overridden.
 *        Fixes still being watched (not yet kept three times) are shown too, with
 *        their tally, so the list is honest about what is being learned rather
 *        than only what has been decided.
 * WHERE: The second card on the Vocabulary page of Settings.
 */

import { useCallback } from "react";
import { Trash2 } from "lucide-react";
import { commands, type LearnedCorrection } from "@/lib/bindings";
import { unwrapCommand, useCommand } from "@/lib/ipc";
import { Button, EmptyState, ErrorSurface } from "@/components/global";

/** What the tally means, in words. */
function standing(correction: LearnedCorrection): string {
  const { accepted, rejected } = correction;
  if (rejected > 0) return `kept ${accepted}×, undone ${rejected}×`;
  if (accepted === 0) return "just seen";
  return `kept ${accepted}× and never undone`;
}

export function LearnedCorrections() {
  const learned = useCommand(commands.listLearned, []);

  const setAuto = useCallback(
    (correction: LearnedCorrection, autoApply: boolean) => {
      void unwrapCommand(() => commands.setLearnedAuto({ id: correction.id, auto_apply: autoApply })).then(
        learned.reload,
      );
    },
    [learned],
  );

  const remove = useCallback(
    (correction: LearnedCorrection) => {
      void unwrapCommand(() => commands.deleteLearned({ id: correction.id })).then(learned.reload);
    },
    [learned],
  );

  const forgetAll = useCallback(() => {
    void unwrapCommand(() => commands.clearLearned()).then(learned.reload);
  }, [learned]);

  if (learned.error) return <ErrorSurface error={learned.error} onRetry={learned.reload} size="compact" />;

  const rows = learned.data ?? [];
  if (learned.data && rows.length === 0) {
    return (
      <EmptyState
        size="compact"
        headline="Nothing learned yet"
        description="Turn on Smart grammar and Learn from my corrections (Settings → Output). When it fixes something and you leave it, it shows up here."
      />
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <ul className="flex flex-col">
        {rows.map((correction) => (
          <li key={correction.id} className="hairline-b flex items-center gap-3 py-2 last:border-b-0">
            <input
              type="checkbox"
              checked={correction.auto_apply}
              aria-label={`Apply “${correction.pattern}” automatically`}
              onChange={(event) => setAuto(correction, event.target.checked)}
              className="accent-[var(--accent)]"
            />
            <div className="min-w-0 flex-1">
              <p className="text-body text-text-primary">
                <span className="text-text-secondary">{correction.pattern}</span>
                <span className="px-2 text-text-tertiary">→</span>
                {correction.replacement}
              </p>
              <p className="text-caption text-text-tertiary">
                {standing(correction)}
                {correction.auto_apply ? " · applies itself" : ""}
              </p>
            </div>
            <button
              type="button"
              aria-label={`Delete “${correction.pattern}”`}
              onClick={() => remove(correction)}
              className="shrink-0 rounded-input p-1 text-text-secondary transition-colors hover:bg-sunken hover:text-danger"
            >
              <Trash2 className="size-4" />
            </button>
          </li>
        ))}
      </ul>
      <div className="flex justify-end">
        <Button variant="ghost" onClick={forgetAll}>
          Forget everything
        </Button>
      </div>
    </div>
  );
}
