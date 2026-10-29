/**
 * SOURCE OF TRUTH KEYWORDS: SETTINGS_PAGES, SettingsPage, SettingsPageId, pageForSection,
 *   SECTION_TITLES, settings-navigation, settings-search, hidden-page
 * WHAT:  The pages of the Settings screen — their order, names, icons and
 *        one-line descriptions — and which registry sections each one shows.
 * WHY:   What settings EXIST is the registry's business (nothing here lists
 *        them). How they are GROUPED into pages is a layout judgement, and the
 *        one thing the registry cannot decide: a sensible reading order, with
 *        the speech model, the writing style, grammar and AI, and the delivery
 *        of the text each on a page of their own so none of them is a wall of
 *        unrelated switches. A page can hold several sections (Grammar & AI
 *        holds the rule-based fixes and the on-device model as two cards), so
 *        the rule-based and the AI features read as two different things.
 *
 *        A section with no page would silently vanish from the screen, so
 *        `pageForSection` is total over SettingSection and the compiler enforces
 *        it. A page can be `hidden`: it stays in the code, with everything it
 *        needs, and only the navigation stops offering it — per-app profiles
 *        are held back from the first release this way rather than deleted.
 * WHERE: SettingsView (navigation + routing) and SettingsPageBody (the cards).
 */

import { AppWindow, BookMarked, Languages, Mic, NotebookPen, PenLine, ShieldCheck, SlidersHorizontal, Sparkles, Type } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { SettingSection } from "@/lib/bindings";

export type SettingsPageId =
  | "recording"
  | "transcription"
  | "writing"
  | "grammar"
  | "output"
  | "scratchpad"
  | "vocabulary"
  | "profiles"
  | "privacy"
  | "general";

export interface SettingsPage {
  id: SettingsPageId;
  label: string;
  /** One line under the page's heading: what lives here. */
  blurb: string;
  icon: LucideIcon;
  /** The registry sections whose settings this page shows, one card each and in
   *  this order; empty for a page that is not made of settings. */
  sections: readonly SettingSection[];
  /** Kept in the code but left out of the navigation. Remove it to bring the
   *  page back — nothing else needs to change. */
  hidden?: boolean;
}

/** The title of a section's card where it differs from its page's name. */
export const SECTION_TITLES: Readonly<Partial<Record<SettingSection, string>>> = {
  LANGUAGE_HOTKEYS: "Language hotkeys",
  GRAMMAR: "Quick fixes",
  SMART_GRAMMAR: "Smart grammar (on-device AI)",
};

const ALL_PAGES: readonly SettingsPage[] = [
  {
    id: "recording",
    label: "Recording",
    blurb: "Your microphone, the shortcut, and how a dictation starts and stops.",
    icon: Mic,
    sections: ["RECORDING", "LANGUAGE_HOTKEYS"],
  },
  {
    id: "transcription",
    label: "Transcription",
    blurb: "The speech model, the language, and what it listens for.",
    icon: Languages,
    sections: ["TRANSCRIPTION"],
  },
  {
    id: "writing",
    label: "Writing",
    blurb: "How your words are tidied and styled: punctuation, filler words and tone.",
    icon: PenLine,
    sections: ["WRITING"],
  },
  {
    id: "grammar",
    label: "Grammar & AI",
    blurb: "Fix slips with simple rules, or with a small AI model that runs on your Mac.",
    icon: Sparkles,
    sections: ["GRAMMAR", "SMART_GRAMMAR"],
  },
  {
    id: "output",
    label: "Output",
    blurb: "Where the finished text goes: pasting, the clipboard and previews.",
    icon: Type,
    sections: ["OUTPUT"],
  },
  {
    id: "scratchpad",
    label: "Scratchpad",
    blurb: "A floating notepad: ramble, let Vocs shape it, then send it to the app you came from.",
    icon: NotebookPen,
    sections: ["SCRATCHPAD"],
  },
  {
    id: "vocabulary",
    label: "Vocabulary",
    blurb: "Names and terms Vocs should always get right.",
    icon: BookMarked,
    sections: ["VOCABULARY"],
  },
  {
    id: "profiles",
    label: "Per-app",
    blurb: "Settings that apply only while a particular app is in front.",
    icon: AppWindow,
    sections: [],
    hidden: true,
  },
  {
    id: "privacy",
    label: "Privacy",
    blurb: "What is kept on this Mac, and for how long.",
    icon: ShieldCheck,
    sections: ["PRIVACY"],
  },
  {
    id: "general",
    label: "General",
    blurb: "Startup, updates and backups.",
    icon: SlidersHorizontal,
    sections: ["GENERAL"],
  },
];

/** What the navigation offers. */
export const SETTINGS_PAGES: readonly SettingsPage[] = ALL_PAGES.filter((page) => !page.hidden);

const BY_SECTION: Readonly<Record<SettingSection, SettingsPageId>> = {
  RECORDING: "recording",
  LANGUAGE_HOTKEYS: "recording",
  TRANSCRIPTION: "transcription",
  WRITING: "writing",
  GRAMMAR: "grammar",
  SMART_GRAMMAR: "grammar",
  OUTPUT: "output",
  SCRATCHPAD: "scratchpad",
  VOCABULARY: "vocabulary",
  PRIVACY: "privacy",
  GENERAL: "general",
};

export function pageForSection(section: SettingSection): SettingsPage {
  const id = BY_SECTION[section];
  return SETTINGS_PAGES.find((page) => page.id === id) ?? SETTINGS_PAGES[0];
}

/** The page a deep link names, else the first. `#/settings/privacy` → Privacy.
 *  A hidden page is not a destination: its link lands on the first page. */
export function resolvePage(section: string | null): SettingsPage {
  return SETTINGS_PAGES.find((page) => page.id === section) ?? SETTINGS_PAGES[0];
}
