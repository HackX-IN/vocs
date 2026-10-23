/**
 * SOURCE OF TRUTH KEYWORDS: SETTINGS_PAGES, SettingsPage, SettingsPageId, pageForSection,
 *   settings-navigation, settings-search
 * WHAT:  The pages of the Settings screen — their order, names, icons and
 *        one-line descriptions — and which registry section each one shows.
 * WHY:   What settings EXIST is the registry's business (nothing here lists
 *        them). How they are GROUPED into pages is a layout judgement, and the
 *        one thing the registry cannot decide: seven pages in a sensible reading
 *        order, with per-app profiles as a page of their own because a profile
 *        is not a setting and claiming it lived under Recording or Output would
 *        be a layout lie about what the registry declares. A section with no page
 *        would silently vanish from the screen, so `pageForSection` is total over
 *        SettingSection and the compiler enforces it.
 * WHERE: SettingsView (navigation + routing) and SettingsPage (the cards).
 */

import { AppWindow, BookMarked, Languages, Mic, ShieldCheck, SlidersHorizontal, Type } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { SettingSection } from "@/lib/bindings";

export type SettingsPageId =
  | "recording"
  | "transcription"
  | "output"
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
  /** The registry section whose settings this page shows; null for a page that
   *  is not made of settings. */
  section: SettingSection | null;
}

export const SETTINGS_PAGES: readonly SettingsPage[] = [
  {
    id: "recording",
    label: "Recording",
    blurb: "Your microphone, the shortcut, and how a dictation starts and stops.",
    icon: Mic,
    section: "RECORDING",
  },
  {
    id: "transcription",
    label: "Transcription",
    blurb: "The speech model, the language, and what it listens for.",
    icon: Languages,
    section: "TRANSCRIPTION",
  },
  {
    id: "output",
    label: "Output",
    blurb: "How the words are cleaned up and where they go.",
    icon: Type,
    section: "OUTPUT",
  },
  {
    id: "vocabulary",
    label: "Vocabulary",
    blurb: "Names and terms Vocs should always get right.",
    icon: BookMarked,
    section: "VOCABULARY",
  },
  {
    id: "profiles",
    label: "Per-app",
    blurb: "Settings that apply only while a particular app is in front.",
    icon: AppWindow,
    section: null,
  },
  {
    id: "privacy",
    label: "Privacy",
    blurb: "What is kept on this Mac, and for how long.",
    icon: ShieldCheck,
    section: "PRIVACY",
  },
  {
    id: "general",
    label: "General",
    blurb: "Startup, updates and backups.",
    icon: SlidersHorizontal,
    section: "GENERAL",
  },
];

const BY_SECTION: Readonly<Record<SettingSection, SettingsPageId>> = {
  RECORDING: "recording",
  TRANSCRIPTION: "transcription",
  OUTPUT: "output",
  VOCABULARY: "vocabulary",
  PRIVACY: "privacy",
  GENERAL: "general",
};

export function pageForSection(section: SettingSection): SettingsPage {
  const id = BY_SECTION[section];
  return SETTINGS_PAGES.find((page) => page.id === id) ?? SETTINGS_PAGES[0];
}

/** The page a deep link names, else the first. `#/settings/privacy` → Privacy. */
export function resolvePage(section: string | null): SettingsPage {
  return SETTINGS_PAGES.find((page) => page.id === section) ?? SETTINGS_PAGES[0];
}
