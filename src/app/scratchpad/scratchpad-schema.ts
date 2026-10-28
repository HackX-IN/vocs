/**
 * SOURCE OF TRUTH KEYWORDS: scratchpadSchema, ScratchpadValues, SCRATCHPAD_MAX_CHARS,
 *   MODE_OPTIONS, readStoredMode, storeMode
 * WHAT:  The scratchpad form's Zod schema, the four organise modes as picker
 *        options, and the remembered choice between them.
 * WHY:   Every input is validated against a declared schema (CLAUDE.md §5). The
 *        limit mirrors MAX_SCRATCHPAD_CHARS in ipc/commands/scratchpad.rs — Rust
 *        enforces it for real, this only lets the form say so before a round trip.
 *        The mode is remembered in localStorage because it is a per-viewer
 *        convenience, not domain state: nothing on the Rust side reads it, and
 *        the storage may be empty or blocked, so every access is guarded.
 * WHERE: Used by Scratchpad.tsx.
 */

import { z } from "zod";
import type { OrganizeMode } from "@/lib/bindings";
import type { SegmentOption } from "@/components/global";

export const SCRATCHPAD_MAX_CHARS = 50_000;

export const scratchpadSchema = z.object({
  text: z.string().max(SCRATCHPAD_MAX_CHARS, "That is too much for the scratchpad to handle at once."),
});

export type ScratchpadValues = z.infer<typeof scratchpadSchema>;

export const MODE_OPTIONS: readonly SegmentOption<OrganizeMode>[] = [
  { value: "AUTO", label: "Auto" },
  { value: "BULLETS", label: "Bullets" },
  { value: "ACTIONS", label: "Actions" },
  { value: "PARAGRAPH", label: "Paragraph" },
];

const MODE_STORAGE_KEY = "vocs.scratchpad.mode";

export function readStoredMode(): OrganizeMode {
  try {
    const stored = window.localStorage.getItem(MODE_STORAGE_KEY);
    return MODE_OPTIONS.find((option) => option.value === stored)?.value ?? "AUTO";
  } catch {
    return "AUTO";
  }
}

export function storeMode(mode: OrganizeMode): void {
  try {
    window.localStorage.setItem(MODE_STORAGE_KEY, mode);
  } catch {
    // Storage can be blocked; the choice then simply is not remembered.
  }
}
