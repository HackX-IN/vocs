/**
 * SOURCE OF TRUTH KEYWORDS: SidebarWindow, SidebarWindowProps, detached-rail,
 *   rail-width, rail-item-size, nav-selected
 * WHAT:  The contents of the detached sidebar WINDOW: the identity mark, then
 *        one icon per registry nav entry, sorted by NavDef.order.
 * WHY:   THE WINDOW IS THE RAIL, which is the same rule the pill lives under
 *        and it carries the same consequences. The window is transparent with
 *        native vibrancy cut to --radius-rail (a full pill), and this nav fills
 *        it exactly, painting the same curve in CSS. Both have to be the same
 *        radius: a square web surface over a rounded effect view shows square
 *        corners, which is what the rail used to look like. The system shadow is
 *        off (tauri.conf.json) because it was drawn around the SQUARE window, and
 *        a CSS shadow has nowhere to fall — the window ends where the rail ends.
 *
 *        Hovering an item shows its name as a native tooltip (the `title`): a
 *        tooltip drawn in the page would be clipped by this window's edge.
 *
 *        The mark glances left and right on a slow loop (Mark `look`): the one
 *        piece of ambient life in an otherwise still rail. Spacing is tight and
 *        every gap comes from --rail-* tokens, with the rail's width equal to a
 *        tile plus its padding on both sides, so tiles and mark share one
 *        centre line.
 *
 *        IT HAS NO TRAFFIC-LIGHT SPACER, and that is the substantive difference
 *        from the rail this replaces. The embedded version reserved
 *        --titlebar-height at its top because the window's hidden title bar put
 *        the three buttons over that glass. A detached window has no buttons to
 *        clear, so reserving the space would leave a dead 38pt gap above the
 *        mark and make a deliberate floating object look mis-padded. Padding is
 *        --rail-padding, equal top and bottom, because a widget is symmetrical.
 *
 *        IT HAS NO DRAG REGION either. It is attached as a child window of the
 *        dashboard so macOS moves it with the parent; dragging a child moves
 *        only the child, which would tear the rail away from the window it is
 *        supposed to be hanging off. The dashboard's header remains the drag
 *        surface for the app.
 *
 *        Selection is the accent on its soft wash — the one place the rail
 *        speaks. The rail paints its own solid surface (body[data-window=
 *        "sidebar"] in global.css) because the dashboard no longer shows native
 *        vibrancy through a translucent page; a bare rail would be the only
 *        see-through object in the app.
 * WHERE: Rendered by src/entries/sidebar.tsx into the `sidebar` window. Route
 *        choices leave via onSelect, which the entry forwards to the dashboard
 *        as `nav-selected`. Icons resolve through lib/icons.ts.
 */

import { cn } from "@/lib/utils";
import { iconFor } from "@/lib/icons";
import { Mark } from "@/components/global";
import type { NavDef } from "@/lib/bindings";

export interface SidebarWindowProps {
  items: readonly NavDef[];
  activeRoute: string;
  onSelect: (route: string) => void;
}

export function SidebarWindow({ items, activeRoute, onSelect }: SidebarWindowProps) {
  return (
    <nav
      aria-label="Sections"
      className="hairline flex h-full w-full flex-col items-center gap-[var(--rail-item-gap)] rounded-[var(--radius-rail)] bg-elevated p-[var(--rail-padding)]"
    >
      <span className="mb-[var(--rail-mark-gap)] flex h-[var(--rail-mark-height)] shrink-0 items-center text-text-primary">
        <Mark label="Vocs" look />
      </span>

      {items.map((item) => {
        const Icon = iconFor(item.icon);
        const isActive = item.route === activeRoute;
        return (
          <button
            key={item.route}
            type="button"
            onClick={() => onSelect(item.route)}
            title={item.label}
            aria-label={item.label}
            aria-current={isActive ? "page" : undefined}
            className={cn(
              "flex size-[var(--rail-item-size)] shrink-0 items-center justify-center rounded-input transition-colors",
              isActive
                ? "bg-accent-soft text-accent"
                : "text-text-secondary hover:bg-sunken hover:text-text-primary",
            )}
          >
            <Icon className="size-4 shrink-0" aria-hidden="true" />
          </button>
        );
      })}
    </nav>
  );
}
