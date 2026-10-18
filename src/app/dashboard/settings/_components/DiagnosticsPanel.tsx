/**
 * SOURCE OF TRUTH KEYWORDS: DiagnosticsPanel, exportDiagnostics, support-file,
 *   opt-in-diagnostics
 * WHAT:  A button that saves a plain-text support report to a file the user
 *        chooses, and says what is and is not in it.
 * WHY:   Vocs has no telemetry and will not get any, which leaves a user with a
 *        bug and a developer with no information. This is the middle path: the
 *        user makes the file, can open it before sending, and sends it by
 *        whatever route they like — nothing leaves the Mac unless they send it.
 *        The caption states the exclusions because that is the part a cautious
 *        person needs before pressing the button.
 * WHERE: Rendered by SettingsView in the PRIVACY section, beside Delete all
 *        history.
 */

import { useState } from "react";
import { commands, type AppError } from "@/lib/bindings";
import { unwrapCommand } from "@/lib/ipc";
import { saveTextFile } from "@/lib/save-file";
import { Button, ErrorSurface } from "@/components/global";

export function DiagnosticsPanel() {
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  const exportReport = async () => {
    setBusy(true);
    setSaved(false);
    setError(null);

    const report = await unwrapCommand(commands.exportDiagnostics);
    if (report.status === "error") {
      setError(report.error);
      setBusy(false);
      return;
    }

    const stamp = new Date().toISOString().slice(0, 10);
    const outcome = await saveTextFile(`vocs-diagnostics-${stamp}.txt`, report.data);
    if (outcome.status === "error") setError(outcome.error);
    else setSaved(outcome.status === "saved");
    setBusy(false);
  };

  return (
    <div className="flex flex-col gap-2 py-3">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0 flex-1">
          <p className="text-body text-text-primary">Export diagnostics</p>
          <p className="text-caption text-text-secondary">
            {saved
              ? "Saved. Open it first if you like — it is plain text."
              : "A text file for support: versions, permissions, timings, settings and recent errors. It never includes what you said, your dictionary or your audio, and it is not sent anywhere."}
          </p>
        </div>
        <Button disabled={busy} onClick={() => void exportReport()}>
          {busy ? "Exporting…" : "Export…"}
        </Button>
      </div>
      {error ? <ErrorSurface size="compact" error={error} /> : null}
    </div>
  );
}
