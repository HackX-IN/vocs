/**
 * SOURCE OF TRUTH KEYWORDS: WhatsNewDialog, useWhatsNew, whats-new-carousel,
 *   getWhatsNew, markWhatsNewSeen
 * WHAT:  The "What's new" dialog — a carousel of what a release added — and the hook
 *        that opens it once per version.
 * WHY:   Two pieces because there are two ways in. `useWhatsNew` asks Rust whether
 *        the notes for the running version are due (it is the one that knows, from
 *        the last version it recorded) and opens the dialog if so; Settings opens it
 *        on request with `WhatsNewDialog` directly. Closing by ANY route — Done,
 *        Skip, Escape, a click on the scrim — records the version as seen, so it is
 *        never shown twice and a person who skips it is not nagged on the next launch.
 *
 *        A version with no slides is marked seen without ever opening: the hook
 *        must not leave a release permanently "due" because nobody wrote its copy.
 *        The first slide is where focus lands and the last slide's button is the
 *        one primary control, "Done".
 * WHERE: `useWhatsNew` is used by Dashboard; the dialog is also opened by the
 *        Updates panel's "What's new" button.
 */

import { useCallback, useEffect, useState } from "react";
import { commands } from "@/lib/bindings";
import { unwrapCommand } from "@/lib/ipc";
import { Button, Carousel, Dialog } from "@/components/global";
import { slidesFor, type Slide } from "./slides";

export interface WhatsNewDialogProps {
  version: string;
  open: boolean;
  onClose: () => void;
}

export function WhatsNewDialog({ version, open, onClose }: WhatsNewDialogProps) {
  const slides = slidesFor(version);
  const [index, setIndex] = useState(0);

  // Each opening starts from the first slide.
  useEffect(() => {
    if (open) setIndex(0);
  }, [open]);

  if (slides.length === 0) return null;
  const last = index === slides.length - 1;

  const footer = (
    <footer className="flex items-center justify-between gap-2 px-6 pb-5 pt-3">
      {last ? <span /> : (
        <Button variant="ghost" onClick={onClose}>
          Skip
        </Button>
      )}
      <div className="flex items-center gap-2">
        {index > 0 ? <Button onClick={() => setIndex(index - 1)}>Back</Button> : null}
        {last ? (
          <Button variant="primary" data-autofocus onClick={onClose}>
            Done
          </Button>
        ) : (
          <Button data-autofocus onClick={() => setIndex(index + 1)}>
            Next
          </Button>
        )}
      </div>
    </footer>
  );

  return (
    <Dialog open={open} onClose={onClose} label={`What's new in Vocs ${version}`}>
      <Carousel
        label={`What's new in Vocs ${version}`}
        slides={slides}
        index={index}
        onIndexChange={setIndex}
        renderSlide={(slide: Slide, position) => (
          <article>
            {slide.visual}
            <div className="flex flex-col gap-2 px-6 pt-5">
              <p className="flex items-center gap-2 font-mono text-eyebrow uppercase text-text-tertiary">
                <span className="text-accent">{String(position + 1).padStart(2, "0")}</span>· {slide.eyebrow}
              </p>
              <h2 className="text-heading text-text-primary">{slide.title}</h2>
              <p className="min-h-[5.5rem] text-body text-text-secondary">{slide.body}</p>
            </div>
          </article>
        )}
        footer={footer}
      />
    </Dialog>
  );
}

/**
 * WHAT:  The version whose notes are due, if any, and the open/dismiss handlers.
 * WHY:   Asks once on mount. A failure to ask is silent — an announcement is never
 *        worth an error box — and simply means it is offered on a later launch.
 */
export function useWhatsNew() {
  const [version, setVersion] = useState<string | null>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void unwrapCommand(commands.getWhatsNew).then((result) => {
      if (cancelled || result.status !== "ok" || !result.data) return;
      if (slidesFor(result.data.version).length === 0) {
        // Nothing to say for this release: record it so it is not asked about again.
        void unwrapCommand(commands.markWhatsNewSeen);
        return;
      }
      setVersion(result.data.version);
      setOpen(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const dismiss = useCallback(() => {
    setOpen(false);
    void unwrapCommand(commands.markWhatsNewSeen);
  }, []);

  return { version, open, dismiss };
}
