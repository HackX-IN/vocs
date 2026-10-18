/**
 * SOURCE OF TRUTH KEYWORDS: Chip, ChipProps, meta-chip, fact-chip, pill-label
 * WHAT:  A small rounded label for a fact about the thing on the page — a date,
 *        a duration, a language — with an optional leading icon.
 * WHY:   Facts about a document sit in a row of chips under its title rather than
 *        in a table or a sentence: they are scanned, not read, and each one
 *        stands alone. A chip is display only — it carries no handler — so a row
 *        of them can never be mistaken for a row of buttons; anything clickable
 *        is a Button. One component so the radius, hairline and type size are
 *        decided once.
 * WHERE: The dictation editor's meta row.
 */

import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

export interface ChipProps {
  icon?: ReactNode;
  children: ReactNode;
  className?: string;
}

export function Chip({ icon, children, className }: ChipProps) {
  return (
    <span
      className={cn(
        "hairline inline-flex h-7 items-center gap-1.5 rounded-pill bg-sunken px-3 text-label text-text-secondary",
        className,
      )}
    >
      {icon ? <span aria-hidden="true" className="flex size-3.5 items-center justify-center">{icon}</span> : null}
      {children}
    </span>
  );
}
