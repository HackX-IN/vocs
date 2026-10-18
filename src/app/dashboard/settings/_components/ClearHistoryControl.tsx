/**
 * SOURCE OF TRUTH KEYWORDS: ClearHistoryControl, clearHistory, delete-everything, two-step-delete
 * WHAT:  The delete-everything control.
 * WHY:   Two-step, and the second step is the destructive one. There is no
 *        tombstone and no undo behind this — clear_history destroys the rows —
 *        so the confirmation is the only safety net, exactly as the cancel
 *        countdown is for a recording. The confirm disarms on blur, so walking
 *        away mid-confirmation leaves nothing armed.
 * WHERE: The Privacy page, next to diagnostics.
 */

import { useState } from "react";
import { commands } from "@/lib/bindings";
import { unwrapCommand } from "@/lib/ipc";
import { Button } from "@/components/global";

export function ClearHistoryControl() {
  const [confirming, setConfirming] = useState(false);
  const [deleted, setDeleted] = useState<number | null>(null);

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
      <Button
        variant={confirming ? "danger" : "default"}
        onClick={() => {
          if (!confirming) {
            setConfirming(true);
            return;
          }
          void unwrapCommand(commands.clearHistory).then((result) => {
            if (result.status === "ok") setDeleted(result.data);
            setConfirming(false);
          });
        }}
        onBlur={() => setConfirming(false)}
      >
        {confirming ? "Delete everything" : "Delete…"}
      </Button>
    </div>
  );
}
