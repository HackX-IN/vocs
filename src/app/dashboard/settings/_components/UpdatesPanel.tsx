/**
 * SOURCE OF TRUTH KEYWORDS: UpdatesPanel, checkForUpdate, installUpdate, updateProgress,
 *   updateAvailable, UpdateCheck, check-for-updates-button, release-notes
 * WHAT:  The Settings card for updates: the running version, a Check for updates button, and one
 *        clear answer — offline, up to date, turned off, or a newer version with its release notes
 *        and a download that reports its progress.
 * WHY:   Until this card the only manual check lived in the menu bar and answered in a system
 *        dialog, and the dashboard bar swallowed an unreachable server in silence — so someone
 *        offline pressing the only button there was got no answer at all. Every outcome of a
 *        check is rendered here, including the failures, because "could not reach the server" is
 *        the answer an offline Mac needs to be told.
 *
 *        THE ANSWER LIVES IN RUST. `check_for_update` decides disabled / current / available and
 *        the scheduler pushes `UpdateAvailable`; this component only renders the last answer it
 *        was given and holds the UI-only parts (which phase is on screen, when it asked).
 *        It also asks ONCE on mount, quietly, so opening Settings while the launch check has
 *        already found something shows it instead of an idle button next to a bar saying
 *        otherwise. That quiet check never shows a failure: nobody pressed anything.
 *
 *        Installing replaces the running app and restarts it, so the button names that, and the
 *        backend refuses mid-dictation — that refusal is surfaced as an ordinary error. Progress
 *        is real bytes from `UpdateProgress`, never a spinner, for the reason in docs/04 §9.
 * WHERE: Rendered by SettingsPageBody under the General page, between the settings and Backup.
 *        Counterpart of the dashboard's UpdateNotice bar, which offers the same install.
 */

import { useCallback, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { commands, events, type AppError, type UpdateCheck, type UpdateProgress } from "@/lib/bindings";
import { unwrapCommand } from "@/lib/ipc";
import { formatBytes, formatTimeOfDay } from "@/lib/format";
import { useTauriEvent } from "@/lib/use-event";
import { Button, ErrorSurface, ProgressBar } from "@/components/global";

type Available = Extract<UpdateCheck, { kind: "AVAILABLE" }>;

/** What the card is showing. Only one at a time, so impossible combinations cannot be drawn. */
type Phase =
  | { kind: "IDLE" }
  | { kind: "CHECKING" }
  | { kind: "CURRENT"; at: Date }
  | { kind: "DISABLED" }
  | { kind: "AVAILABLE"; update: Available }
  | { kind: "INSTALLING"; update: Available; progress: UpdateProgress | null }
  | { kind: "FAILED"; error: AppError };

function formatPublished(iso: string | null): string | null {
  if (!iso) return null;
  const date = new Date(iso);
  return Number.isNaN(date.getTime())
    ? null
    : date.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
}

export function UpdatesPanel() {
  const [version, setVersion] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>({ kind: "IDLE" });

  // `quiet` is the check on mount: it may report what it finds but never an error, since the
  // person did not ask and an offline Mac would otherwise open Settings to a red box.
  const check = useCallback(async (quiet: boolean) => {
    if (!quiet) setPhase({ kind: "CHECKING" });
    const result = await unwrapCommand(commands.checkForUpdate);

    if (result.status === "error") {
      setPhase(quiet ? { kind: "IDLE" } : { kind: "FAILED", error: result.error });
      return;
    }
    switch (result.data.kind) {
      case "AVAILABLE":
        setPhase({ kind: "AVAILABLE", update: result.data });
        break;
      case "UP_TO_DATE":
        setPhase({ kind: "CURRENT", at: new Date() });
        break;
      case "DISABLED":
        setPhase({ kind: "DISABLED" });
        break;
    }
  }, []);

  useEffect(() => {
    void getVersion().then(setVersion, () => setVersion(null));
    void check(true);
  }, [check]);

  // A scheduled check that lands while Settings is open.
  useTauriEvent(events.updateAvailable, (payload) => {
    setPhase((current) =>
      current.kind === "INSTALLING" ? current : { kind: "AVAILABLE", update: { kind: "AVAILABLE", ...payload, published_at: null } },
    );
  });

  useTauriEvent(events.updateProgress, (progress) => {
    setPhase((current) => (current.kind === "INSTALLING" ? { ...current, progress } : current));
  });

  const install = (update: Available) => {
    setPhase({ kind: "INSTALLING", update, progress: null });
    void unwrapCommand(commands.installUpdate).then((result) => {
      // On success the app is replaced and restarts, so only a failure has anywhere to go.
      if (result.status === "error") setPhase({ kind: "FAILED", error: result.error });
    });
  };

  const checking = phase.kind === "CHECKING";
  const installing = phase.kind === "INSTALLING";

  return (
    <div className="flex flex-col gap-3 py-3">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0 flex-1">
          <p className="text-body text-text-primary">{version ? `Vocs ${version}` : "Vocs"}</p>
          <p className="text-caption text-text-secondary">{summary(phase)}</p>
        </div>
        {phase.kind === "AVAILABLE" ? (
          <Button onClick={() => install(phase.update)}>Install and restart</Button>
        ) : (
          <Button disabled={checking || installing} onClick={() => void check(false)}>
            {checking ? "Checking" : phase.kind === "FAILED" ? "Try again" : "Check for updates"}
          </Button>
        )}
      </div>

      {phase.kind === "AVAILABLE" || phase.kind === "INSTALLING" ? <ReleaseNotes update={phase.update} /> : null}

      {phase.kind === "INSTALLING" ? <DownloadProgress progress={phase.progress} /> : null}

      {phase.kind === "FAILED" ? <ErrorSurface size="compact" error={phase.error} /> : null}
    </div>
  );
}

function summary(phase: Phase): string {
  switch (phase.kind) {
    case "IDLE":
      return "Check whether a newer version is available.";
    case "CHECKING":
      return "Looking for a new version";
    case "CURRENT":
      return `You are on the newest version. Checked at ${formatTimeOfDay(phase.at.getTime())}.`;
    case "DISABLED":
      return "Update checks are turned off. Turn on Check for updates above to look for a new version.";
    case "AVAILABLE":
      return `Vocs ${phase.update.version} is available${publishedSuffix(phase.update)}.`;
    case "INSTALLING":
      return `Installing Vocs ${phase.update.version}. Vocs restarts when it is done.`;
    case "FAILED":
      return "The check did not finish.";
  }
}

function publishedSuffix(update: Available): string {
  const published = formatPublished(update.published_at);
  return published ? `, released ${published}` : "";
}

function ReleaseNotes({ update }: { update: Available }) {
  const notes = update.notes?.trim();
  if (!notes) return null;
  return (
    <div className="max-h-48 overflow-y-auto rounded-input bg-sunken px-3 py-2">
      <p className="whitespace-pre-wrap text-caption text-text-secondary">{notes}</p>
    </div>
  );
}

function DownloadProgress({ progress }: { progress: UpdateProgress | null }) {
  const total = progress?.total_bytes ?? null;
  const received = progress?.received_bytes ?? 0;
  const fraction = total ? received / total : null;
  const done = fraction !== null && fraction >= 1;

  return (
    <ProgressBar
      fraction={progress === null ? 0 : fraction}
      label="Downloading the update"
      caption={
        done
          ? "Installing and restarting"
          : total
            ? `${formatBytes(received)} of ${formatBytes(total)}`
            : `${formatBytes(received)} downloaded`
      }
    />
  );
}
