/**
 * SOURCE OF TRUTH KEYWORDS: ClearHistoryControl, clearHistory, delete-everything, two-step-delete
 * WHAT:  The delete-everything control.
 * WHY:   Two-step, and the second step is the destructive one. There is no
 *        tombstone and no undo behind this — clear_history destroys the rows —
 *        so the confirmation is the only safety net, exactly as the cancel
 *        countdown is for a recording. The confirm is the shared ConfirmButton:
 *        Delete swaps for Cancel and a red confirm, and Escape backs out.
 * WHERE: The Privacy page, next to diagnostics.
 */

import { useState } from "react";
import { commands } from "@/lib/bindings";
import { unwrapCommand } from "@/lib/ipc";
import { ConfirmButton } from "@/components/global";

export function ClearHistoryControl() {
  const [deleted, setDeleted] = useState<number | null>(null);

  const clear = async () => {
    const result = await unwrapCommand(commands.clearHistory);
    if (result.status === "ok") setDeleted(result.data);
  };

  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="min-w-0 flex-1">
        <p className="text-body text-text-primary">Delete all history</p>
        <p className="text-caption text-text-secondary">
          {deleted === null
            ? "Every transcript, permanently. This cannot be undone."
            : `Deleted ${deleted} transcript${deleted === 1 ? "" : "s"}.`}
        </p>
      </div>
      <ConfirmButton confirmLabel="Delete everything" onConfirm={clear}>
        Delete
      </ConfirmButton>
    </div>
  );
}
