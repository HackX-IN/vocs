/**
 * SOURCE OF TRUTH KEYWORDS: WHATS_NEW_SLIDES, slidesFor, Slide, whats-new-copy,
 *   SlideVisual
 * WHAT:  The "What's new" slides, keyed by the version that introduced them: for
 *        each an eyebrow, a title, a sentence, and a small picture drawn from
 *        tokens (keycaps, a struck-through line, a mini list) — no image files.
 * WHY:   Product copy and its illustrations travel together, so a release's
 *        announcement is one edit in one place: add an entry for the new version.
 *        A version with no entry shows nothing and is simply marked seen, so
 *        forgetting to write one is a missing announcement rather than a blank
 *        dialog. Rust only decides WHETHER the notes are due (whats_new.rs); the
 *        words belong to the release, not to the backend.
 *
 *        Every claim here is one the release makes and a test or the changelog
 *        stands behind; the pictures are pure markup so they follow both colour
 *        schemes and cost nothing to load.
 * WHERE: Read by WhatsNewDialog. Add the next release's entry beside the last.
 */

import { useState, type ReactNode } from "react";
import { Check, Feather, Globe2, ListChecks, ShieldCheck, Sparkles, type LucideIcon } from "lucide-react";
import { Keycap, SegmentedControl } from "@/components/global";

export interface Slide {
  icon: LucideIcon;
  eyebrow: string;
  title: string;
  body: string;
  visual: ReactNode;
}

/** The frame every picture sits in, so they share one size and one ground. */
function Stage({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-[var(--dialog-visual-height)] items-center justify-center bg-sunken px-6">{children}</div>
  );
}

function Said({ children }: { children: ReactNode }) {
  return <span className="font-mono text-caption text-text-tertiary">{children}</span>;
}

const ReliabilityVisual = (
  <Stage>
    <ul className="flex flex-col gap-2 text-label text-text-primary">
      {["You stop in the middle of a pause", "Your Mac is busy or slow", "The grammar model is switched on"].map((line) => (
        <li key={line} className="material-elevated flex items-center gap-3 rounded-input px-3 py-2">
          <Check className="size-4 text-success" aria-hidden />
          {line}
          <span className="ml-auto text-caption text-text-tertiary">pastes</span>
        </li>
      ))}
    </ul>
  </Stage>
);

const ScratchVisual = (
  <Stage>
    <div className="flex flex-col items-center gap-3">
      <Said>you say “scratch that”</Said>
      <p className="material-elevated rounded-input px-4 py-3 text-body text-text-primary">
        <s className="text-text-tertiary decoration-danger">Ship it on Friday.</s> Ship it Monday.
      </p>
    </div>
  </Stage>
);

const LanguagesVisual = (
  <Stage>
    <div className="flex flex-col gap-3">
      {[
        { keys: ["⌃", "⌥", "1"], language: "English" },
        { keys: ["⌃", "⌥", "2"], language: "Español" },
      ].map((row) => (
        <div key={row.language} className="flex items-center gap-4">
          <Keycap keys={row.keys} />
          <span className="text-body text-text-secondary">listens for</span>
          <span className="text-body text-text-primary">{row.language}</span>
        </div>
      ))}
    </div>
  </Stage>
);

const ScratchpadVisual = (
  <Stage>
    <div className="material-elevated flex w-full max-w-sm flex-col gap-2 rounded-input p-4 text-caption">
      <p className="text-text-tertiary">“…email Sam about the budget and also remember to book the flights…”</p>
      <p className="font-mono text-eyebrow uppercase text-text-tertiary">Action items</p>
      <p className="text-label text-text-primary">☐ Email Sam about the budget</p>
      <p className="text-label text-text-primary">☐ Book the flights</p>
      <p className="flex items-center gap-2 pt-1 text-text-tertiary">
        <Keycap keys={["↵"]} size="sm" /> organise <Keycap keys={["⌘", "↵"]} size="sm" /> send
      </p>
    </div>
  </Stage>
);

/** A working picker: it is the thing the slide is about, so it should be touchable. */
function AccuracyDemo() {
  const [mode, setMode] = useState<"fast" | "balanced" | "accurate">("fast");
  return (
    <Stage>
      <div className="flex flex-col items-center gap-3">
        <SegmentedControl
          label="Speed or accuracy"
          value={mode}
          onChange={setMode}
          options={[
            { value: "fast", label: "Fast" },
            { value: "balanced", label: "Balanced" },
            { value: "accurate", label: "Accurate" },
          ]}
        />
        <Said>{mode === "fast" ? "the default" : mode === "balanced" ? "a little more context" : "fewest mistakes, slowest"}</Said>
      </div>
    </Stage>
  );
}

const UpdatesVisual = (
  <Stage>
    <div className="material-elevated flex w-full max-w-sm items-center justify-between gap-3 rounded-input px-4 py-3">
      <span className="text-label text-text-secondary">Vocs 1.1.0 is ready to install.</span>
      <span className="hairline rounded-input bg-sunken px-3 py-1 text-label text-text-primary">Install and restart</span>
    </div>
  </Stage>
);

export const WHATS_NEW_SLIDES: Readonly<Record<string, readonly Slide[]>> = {
  "1.1.0": [
    {
      icon: ShieldCheck,
      eyebrow: "Reliability",
      title: "Your words arrive",
      body: "Stopping in a pause, a slow Mac or a busy on-device grammar model can no longer lose what you said or hold up the paste.",
      visual: ReliabilityVisual,
    },
    {
      icon: Feather,
      eyebrow: "Voice commands",
      title: "Say “scratch that”",
      body: "Say “scratch that” to drop the sentence you just spoke, “undo last sentence” for the one before, or “scratch everything” to start over. They are removed before anything is pasted. English.",
      visual: ScratchVisual,
    },
    {
      icon: Globe2,
      eyebrow: "Languages",
      title: "A hotkey for each language",
      body: "Give each language its own shortcut in Settings → Recording and skip auto-detection altogether. Off until you turn it on.",
      visual: LanguagesVisual,
    },
    {
      icon: ListChecks,
      eyebrow: "Scratchpad",
      title: "Ramble first. Tidy second.",
      body: "Open the scratchpad from the menu bar, talk freely, press Return to turn it into bullets, tasks or a paragraph, and ⌘Return to send it to the app you came from.",
      visual: ScratchpadVisual,
    },
    {
      icon: Sparkles,
      eyebrow: "Accuracy",
      title: "Speed or accuracy, your call",
      body: "Fast stays the default. Balanced and Accurate (Settings → Transcription) trade a little time for fewer mistakes, and live captions no longer flicker through half-heard words.",
      visual: <AccuracyDemo />,
    },
    {
      icon: Check,
      eyebrow: "Updates",
      title: "Updates, without the wait",
      body: "Checking is quicker and retries on its own. When a new version is ready it waits in a bar at the bottom of the window: Install and restart, and Vocs comes back by itself.",
      visual: UpdatesVisual,
    },
  ],
};

/** The slides for `version`, or an empty list when that release has none. */
export function slidesFor(version: string): readonly Slide[] {
  return WHATS_NEW_SLIDES[version] ?? [];
}
