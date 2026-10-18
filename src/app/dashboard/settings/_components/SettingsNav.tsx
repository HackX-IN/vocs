/**
 * SOURCE OF TRUTH KEYWORDS: SettingsNav, settings-search, settings-sections, navigateTo
 * WHAT:  The left column of Settings: a search field over every setting, and the
 *        list of pages. Selecting one changes the route, so every page has an
 *        address.
 * WHY:   Settings used to be one long scroll of seven sections, which is a wall:
 *        finding "Fix grammar slips" meant scrolling past the microphone check.
 *        A section list turns it into seven short pages, and the search field
 *        covers the other way people look — by what the thing is called. The
 *        selection is the route (`#/settings/<page>`), not component state,
 *        because error actions and the menu bar deep-link here, and a page that
 *        can be linked to must be a page the app itself navigates by.
 *
 *        Search is a plain controlled input: the caller filters registry
 *        definitions and swaps the right-hand side for the results. Escape
 *        clears it, because a field you cannot leave with the keyboard is a trap.
 * WHERE: Rendered by SettingsView. Pages come from settings-pages.ts.
 */

import { Search, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { SETTINGS_PAGES, type SettingsPageId } from "../settings-pages";

export interface SettingsNavProps {
  activeId: SettingsPageId;
  onSelect: (id: SettingsPageId) => void;
  query: string;
  onQueryChange: (query: string) => void;
}

export function SettingsNav({ activeId, onSelect, query, onQueryChange }: SettingsNavProps) {
  const searching = query.trim().length > 0;

  return (
    <nav aria-label="Settings sections" className="flex w-[var(--settings-nav-width)] shrink-0 flex-col gap-4">
      <label className="hairline flex h-9 items-center gap-2 rounded-input bg-sunken px-3 focus-within:outline-2 focus-within:outline-accent">
        <Search aria-hidden="true" className="size-4 shrink-0 text-text-tertiary" />
        <input
          type="search"
          value={query}
          placeholder="Search settings"
          aria-label="Search settings"
          onChange={(event) => onQueryChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape" && query.length > 0) {
              event.preventDefault();
              onQueryChange("");
            }
          }}
          className="min-w-0 flex-1 bg-transparent text-body text-text-primary placeholder:text-text-tertiary"
        />
        {query.length > 0 ? (
          <button
            type="button"
            aria-label="Clear search"
            onClick={() => onQueryChange("")}
            className="text-text-tertiary transition-colors hover:text-text-primary"
          >
            <X className="size-4" />
          </button>
        ) : null}
      </label>

      <ul className="flex flex-col gap-1">
        {SETTINGS_PAGES.map((page) => {
          const Icon = page.icon;
          // While searching, no page is "current" — the results are.
          const isActive = !searching && page.id === activeId;
          return (
            <li key={page.id}>
              <button
                type="button"
                onClick={() => onSelect(page.id)}
                aria-current={isActive ? "page" : undefined}
                className={cn(
                  "flex h-9 w-full items-center gap-3 rounded-input px-3 text-body transition-colors",
                  isActive
                    ? "bg-accent-soft text-text-primary"
                    : "text-text-secondary hover:bg-sunken hover:text-text-primary",
                )}
              >
                <Icon aria-hidden="true" className={cn("size-4 shrink-0", isActive && "text-accent")} />
                {page.label}
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
