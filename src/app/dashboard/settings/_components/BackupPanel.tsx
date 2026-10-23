/**
 * SOURCE OF TRUTH KEYWORDS: BackupPanel, exportBackup, importBackup,
 *   openTextFile, describeImport, vocs-backup
 * WHAT:  Export and import for the dictionary and per-app profiles, as one
 *        JSON file.
 * WHY:   Moving a hundred custom words and a set of app profiles between a work
 *        Mac and a personal one should not need a cloud account, and the whole
 *        point of Vocs is that it has none. The file is chosen and written
 *        through Rust dialogs for the reason save-file.ts spells out: a webview
 *        download fails silently, and a backup the user believes exists but does
 *        not is the worst outcome this feature can have. Import reports what it
 *        added, what it updated and what it had to skip, because "imported"
 *        with nothing said about the three entries that were dropped is a quiet
 *        lie. Cancelling either dialog is a decision, not an error, and shows
 *        nothing.
 * WHERE: Rendered by SettingsView in the GENERAL section; `onImported` makes the
 *        dictionary list and the profile list reload what they show.
 */

import { useState } from "react";
import { commands, type AppError, type ImportSummary } from "@/lib/bindings";
import { unwrapCommand } from "@/lib/ipc";
import { saveTextFile } from "@/lib/save-file";
import { Button, ErrorSurface } from "@/components/global";

type Busy = "export" | "import" | null;

const plural = (count: number, noun: string): string => `${count} ${noun}${count === 1 ? "" : "s"}`;

/** One sentence the user can check against what they expected. */
export function describeImport(summary: ImportSummary): string {
  const added = summary.dictionary_added + summary.profiles_added;
  const updated = summary.dictionary_updated + summary.profiles_updated;
  const skipped = summary.dictionary_skipped + summary.profiles_skipped;

  if (added + updated + skipped === 0) return "That backup was empty, so nothing changed.";

  const parts = [
    `Added ${plural(summary.dictionary_added, "word")} and ${plural(summary.profiles_added, "profile")}`,
    `updated ${plural(summary.dictionary_updated, "word")} and ${plural(summary.profiles_updated, "profile")}`,
  ];
  if (skipped > 0) parts.push(`skipped ${skipped} that could not be used`);
  return `${parts.join(", ")}.`;
}

export function BackupPanel({ onImported }: { onImported: () => void }) {
  const [busy, setBusy] = useState<Busy>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<AppError | null>(null);

  const begin = (kind: Exclude<Busy, null>) => {
    setBusy(kind);
    setMessage(null);
    setError(null);
  };
  const fail = (failure: AppError) => {
    setError(failure);
    setBusy(null);
  };

  const exportBackup = async () => {
    begin("export");
    const result = await unwrapCommand(commands.exportBackup);
    if (result.status === "error") return fail(result.error);

    const stamp = new Date().toISOString().slice(0, 10);
    const outcome = await saveTextFile(`vocs-backup-${stamp}.json`, result.data);
    if (outcome.status === "error") return fail(outcome.error);
    if (outcome.status === "saved") setMessage("Backup saved.");
    setBusy(null);
  };

  const importBackup = async () => {
    begin("import");
    const picked = await unwrapCommand(() => commands.openTextFile({ filter_name: "Vocs backup", extensions: ["json"] }));
    if (picked.status === "error") return fail(picked.error);
    // Cancelled: nothing to say.
    const json = picked.data;
    if (json === null) return setBusy(null);

    const imported = await unwrapCommand(() => commands.importBackup({ json }));
    if (imported.status === "error") return fail(imported.error);

    setMessage(describeImport(imported.data));
    setBusy(null);
    onImported();
  };

  return (
    <div className="flex flex-col gap-2 py-3">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0 flex-1">
          <p className="text-body text-text-primary">Back up and restore</p>
          <p className="text-caption text-text-secondary">
            {message ??
              "Save your dictionary and per-app profiles to a file, or load them from one — to move them to another Mac. Importing adds to what you have; it never deletes."}
          </p>
        </div>
        <Button disabled={busy !== null} onClick={() => void exportBackup()}>
          {busy === "export" ? "Exporting" : "Export"}
        </Button>
        <Button disabled={busy !== null} onClick={() => void importBackup()}>
          {busy === "import" ? "Importing" : "Import"}
        </Button>
      </div>
      {error ? <ErrorSurface size="compact" error={error} /> : null}
    </div>
  );
}
