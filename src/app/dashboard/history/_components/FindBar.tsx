/**
 * SOURCE OF TRUTH KEYWORDS: FindBar, find_in_dictation, find_next, command_f, command_g
 * WHAT:  The find field over the dictation editor: type to count matches, Enter
 *        (or ⌘G) for the next one, ⇧Enter (or ⇧⌘G) for the previous, Escape to
 *        close.
 * WHY:   A document that cannot be searched is not a notepad. WKWebView has no
 *        find-in-page of its own, so this is the editor's own: it reports which
 *        match is current, and the editor does the selecting and scrolling,
 *        because only the editor knows its textarea. Escape closes the bar and
 *        stops there — without that it would also leave the dictation, which is
 *        what Escape does everywhere else on this page.
 * WHERE: Opened by ⌘F in DictationEditor.
 */

import { useEffect, useRef } from "react";
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { Button } from "@/components/global";

export interface FindBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  /** Zero-based current match, or null when there are none. */
  current: number | null;
  total: number;
  onNext: () => void;
  onPrevious: () => void;
  onClose: () => void;
}

export function FindBar({ query, onQueryChange, current, total, onNext, onPrevious, onClose }: FindBarProps) {
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);

  const status = query.trim().length === 0 ? "" : total === 0 ? "No matches" : `${(current ?? 0) + 1} of ${total}`;

  return (
    <div
      role="search"
      className="material-elevated pointer-events-auto absolute top-[calc(var(--titlebar-height)+var(--space-2))] right-[var(--page-padding-x)] z-20 flex h-10 items-center gap-1 rounded-pill pr-1 pl-4"
    >
      <input
        ref={input}
        value={query}
        placeholder="Find in this dictation"
        aria-label="Find in this dictation"
        onChange={(event) => onQueryChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            onClose();
          } else if (event.key === "Enter") {
            event.preventDefault();
            if (event.shiftKey) onPrevious();
            else onNext();
          }
        }}
        className="w-48 bg-transparent text-body text-text-primary outline-none placeholder:text-text-tertiary"
      />
      <span className="w-16 text-right text-caption tabular-nums text-text-tertiary">{status}</span>
      <Button variant="ghost" aria-label="Previous match" disabled={total === 0} onClick={onPrevious} className="size-7 px-0">
        <ChevronUp className="size-4" />
      </Button>
      <Button variant="ghost" aria-label="Next match" disabled={total === 0} onClick={onNext} className="size-7 px-0">
        <ChevronDown className="size-4" />
      </Button>
      <Button variant="ghost" aria-label="Close find" onClick={onClose} className="size-7 px-0">
        <X className="size-4" />
      </Button>
    </div>
  );
}
