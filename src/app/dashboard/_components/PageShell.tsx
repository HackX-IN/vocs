/**
 * SOURCE OF TRUTH KEYWORDS: PageShell, page-header-height, data-scroll-area,
 *   traffic-lights, page-title, WindowBar
 * WHAT:  The frame every dashboard page is rendered into: a window bar with the
 *        page title centred on it, overlaying a scrolling body.
 * WHY:   IT OVERLAYS THE BODY RATHER THAN SITTING ABOVE IT, because content is
 *        meant to tuck under the bar and fade. That only works if they share
 *        space; the consequence is handled in one place — ScrollArea's
 *        headerInset pads content down past --page-header-height — so no page has
 *        to remember not to hide its first line.
 *
 *        The wheel is forwarded to the sibling marked data-scroll-area: the bar is
 *        not inside the scroller (the fade mask paints on the scroll container's
 *        own box and would erase the title along with the content), so a wheel
 *        over the bar would otherwise have no scrollable ancestor.
 * WHERE: Wrapped around every routed view by Dashboard. The title comes from
 *        the capability registry's NavDef.label, so a new page added to the
 *        registry is titled without touching this file.
 */

import type { ReactNode, WheelEvent } from "react";
import { WindowBar } from "@/components/global";

export interface PageShellProps {
  /** Null for a view that draws its own header (the dictation editor): the
   *  shell then adds nothing, so there is no second title and no dead band. */
  title: string | null;
  /** Right-hand side of the bar. */
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
      <WindowBar title={title} trailing={actions} onWheel={forwardWheel} />
      {children}
    </div>
  );
}
