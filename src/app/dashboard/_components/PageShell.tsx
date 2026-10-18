/**
 * SOURCE OF TRUTH KEYWORDS: PageShell, PageHeader, page-header-height,
 *   data-tauri-drag-region, data-scroll-area, traffic-lights, page-title
 * WHAT:  The frame every dashboard page is rendered into: a page header that
 *        overlays a scrolling body, with the title set large beneath the
 *        window's traffic lights.
 * WHY:   TWO ROWS, because the lights and the title are different jobs. macOS
 *        draws the three buttons from the window's top-left at a fixed inset
 *        and nothing in CSS can move them, so the first row is an empty
 *        --traffic-light-band that exists only to be a drag handle beside them;
 *        the title sits on its own row below, on the page's own grid. The old
 *        arrangement squeezed the title into the lights' row, which forced it
 *        to be small and indented — and a large title is the first thing a page
 *        of this design says.
 *
 *        IT OVERLAYS THE BODY RATHER THAN SITTING ABOVE IT, because content is
 *        meant to tuck under the header and fade. That only works if they share
 *        space; the consequence is handled in one place — ScrollArea's
 *        headerInset pads content down past this bar — so no page has to
 *        remember not to hide its first line.
 *
 *        THE WHOLE BAR IS THE DRAG REGION, because it stays put while the page
 *        scrolls. Interactive children opt out with data-tauri-drag-region=
 *        {false}; without that a button inside the bar drags the window instead
 *        of firing. The wheel is forwarded to the sibling marked
 *        data-scroll-area: the header is not inside the scroller (the fade mask
 *        paints on the scroll container's own box and would erase the title
 *        along with the content), so a wheel over the bar would otherwise have
 *        no scrollable ancestor.
 * WHERE: Wrapped around every routed view by Dashboard. The title comes from
 *        the capability registry's NavDef.label, so a new page added to the
 *        registry is titled without touching this file.
 */

import type { ReactNode, WheelEvent } from "react";

export interface PageShellProps {
  /** Null for a view that draws its own header (the dictation editor): the
   *  shell then adds nothing, so there is no second title and no dead band. */
  title: string | null;
  /** Right-hand side of the title row. Must opt out of the drag region. */
  actions?: ReactNode;
  children: ReactNode;
}

export function PageShell({ title, actions, children }: PageShellProps) {
  const forwardWheel = (event: WheelEvent<HTMLElement>) => {
    const scroller = event.currentTarget.parentElement?.querySelector<HTMLElement>("[data-scroll-area]");
    if (scroller) scroller.scrollTop += event.deltaY;
  };

  if (title === null) return <div className="relative flex min-h-0 flex-1 flex-col">{children}</div>;

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <header
        data-tauri-drag-region
        onWheel={forwardWheel}
        className="absolute inset-x-0 top-0 z-10 flex h-[var(--page-header-height)] flex-col justify-end px-[var(--page-padding-x)] pb-1"
      >
        <div data-tauri-drag-region className="flex items-end justify-between gap-4">
          <h1 data-tauri-drag-region className="font-display text-title truncate text-text-primary">
            {title}
          </h1>
          {actions ? (
            <div data-tauri-drag-region={false} className="flex shrink-0 items-center gap-2 pb-2">
              {actions}
            </div>
          ) : null}
        </div>
      </header>
      {children}
    </div>
  );
}
