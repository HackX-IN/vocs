/**
 * SOURCE OF TRUTH KEYWORDS: UpdateNotice, updateAvailable, checkForUpdate,
 *   installUpdate, UpdateCheck, update-bar, updateProgress, install-and-restart
 * WHAT:  A quiet bar along the bottom of the dashboard offering to install a
 *        newer version. Absent entirely until there is one.
 * WHY:   IT MUST NEVER TAKE THE SCREEN. The person using this app is usually
 *        dictating into something else and the dashboard may not even be open —
 *        an update prompt that interrupts that is worse than one he finds late,
 *        because the whole product is "talk and the words appear" and anything
 *        that steals focus mid-sentence breaks the only promise it makes. So
 *        this is not a modal, not a toast and not an overlay: it is docked
 *        BELOW the scroll area, so it covers nothing, moves nothing, and is
 *        simply there when he next looks.
 *
 *        IT ASKS AS WELL AS LISTENS, and that is what makes the feature visible
 *        at all. The backend checks at launch and every 24h and emits an event,
 *        but the dashboard opens from the menu bar and may not exist when that
 *        event fires — a listener alone would miss nearly every one and the
 *        control would appear to be broken while working perfectly. So it calls
 *        check_for_update ONCE on mount for the answer that already exists, and
 *        listens for the event to catch a check that lands while it is open.
 *        Same shape as the pill: a command for first paint, events after that.
 *
 *        IT SHOWS THE DOWNLOAD AS IT HAPPENS. A button that said "Installing" and
 *        then nothing for as long as a connection takes looks like a hang, so the
 *        bar reports real percentages from `UpdateProgress` and, once the bytes are
 *        in, says Vocs is restarting — which it does by itself: there is no second
 *        click, because the whole point of "Install and restart" is that the person
 *        never has to remember the second half.
 *
 *        THE QUIET LOOK ON MOUNT REUSES THE CACHED ANSWER (force: false). Opening
 *        the dashboard used to cost a request to GitHub every time; the backend now
 *        answers from the last check and only a pressed button goes back out.
 *
 *        THE RESTART IS IN THE BUTTON, not in a sentence beside it. Installing
 *        swaps the running app, and the label is the last thing read before
 *        committing — putting it anywhere else means it can be missed by
 *        exactly the person who most needed to see it.
 *
 *        The button is a bordered secondary, deliberately not the inverted
 *        primary fill: docs/04 reserves that weight for the app's one primary
 *        action, and a bar whose whole point is to stay quiet must not carry
 *        the loudest control in the product.
 * WHERE: Rendered once by Dashboard, below PageShell, so it survives route
 *        changes and shows on every page.
 */

import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { commands, events, type AppError, type UpdateProgress } from "@/lib/bindings";
import { unwrapCommand } from "@/lib/ipc";
import { Button } from "@/components/global";
import { useTauriEvent } from "@/lib/use-event";

export function UpdateNotice() {
  const [version, setVersion] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);

  // The answer that may already exist from the launch check. Not forced: a recent
  // answer is as good as a new one, and it costs no round trip.
  useEffect(() => {
    void unwrapCommand(() => commands.checkForUpdate({ force: false })).then((result) => {
      if (result.status === "ok" && result.data.kind === "AVAILABLE") {
        setVersion(result.data.version);
      }
    });
  }, []);

  // And a check that lands while the dashboard happens to be open.
  useTauriEvent(events.updateAvailable, (payload) => {
    setVersion(payload.version);
    setDismissed(false);
  });

  useTauriEvent(events.updateProgress, (next) => setProgress(next));

  if (!version || dismissed) return null;

  const install = () => {
    setInstalling(true);
    setError(null);
    setProgress(null);
    void unwrapCommand(commands.installUpdate).then((result) => {
      // On success the app is replaced and restarts, so there is no success
      // state to render — only the failure needs somewhere to go.
      if (result.status === "error") {
        setError(result.error);
        setInstalling(false);
      }
    });
  };

  return (
    <div
      role="status"
      className="hairline-t flex shrink-0 items-center justify-between gap-4 px-[var(--page-padding-x)] py-3"
    >
      <p className="min-w-0 truncate text-label text-text-secondary">
        {error ? (
          <span className="text-danger">{error.message}</span>
        ) : installing ? (
          installingLabel(version, progress)
        ) : (
          <>Vocs {version} is ready to install.</>
        )}
      </p>

      <div className="flex shrink-0 items-center gap-1">
        <Button onClick={install} disabled={installing} className="h-[var(--control-height-sm)] text-label">
          {installing ? "Installing…" : error ? "Try again" : "Install and restart"}
        </Button>
        {installing ? null : <button
          type="button"
          onClick={() => setDismissed(true)}
          aria-label="Dismiss until the next check"
          className="flex size-[var(--control-height-sm)] items-center justify-center rounded-input text-text-tertiary transition-colors hover:bg-sunken hover:text-text-secondary"
        >
          <X className="size-3.5" />
        </button>}
      </div>
    </div>
  );
}

/** What the bar says while the update downloads, then installs and restarts. */
function installingLabel(version: string, progress: UpdateProgress | null): string {
  if (progress === null) return `Downloading Vocs ${version}…`;
  const { received_bytes: received, total_bytes: total } = progress;
  if (total === null || total <= 0) return `Downloading Vocs ${version}…`;
  const percent = Math.min(100, Math.floor((received / total) * 100));
  return percent >= 100 ? `Installing Vocs ${version} — it restarts by itself.` : `Downloading Vocs ${version} — ${percent}%`;
}
