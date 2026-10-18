/**
 * SOURCE OF TRUTH KEYWORDS: Card, CardProps, Eyebrow, numbered-card, card-index,
 *   instrument-panel, hero-card, dot-grid
 * WHAT:  The app's one surface for a unit of content: an optional numbered
 *        eyebrow ("01 · TIME SAVED"), an optional action slot, and a body.
 * WHY:   The interface is a panel of instruments, and every instrument is this
 *        card — stats, each settings group, the history summary. Authoring it
 *        once is what keeps the radius, the hairline, the shadow and the eyebrow
 *        typography identical everywhere, and it is why those views can be
 *        rebuilt from the registry without each inventing a container.
 *
 *        The index is the signature detail: a small accent numeral before a
 *        mono capital title. It is supplied by the caller rather than counted
 *        here, because only the caller knows the order its cards are shown in —
 *        a card that numbered itself would renumber when another one hid.
 *
 *        `hero` adds the dot lattice behind the content, fading toward the
 *        bottom-left. It is for the one card per view that holds the figure the
 *        view exists to show; repeating it would stop it meaning anything.
 *        Built on GlassPanel so the surface, hairline and inner highlight stay
 *        one definition (docs/04 §3).
 * WHERE: Stats, Settings, the Dictations summary, Billing. Eyebrow is exported
 *        separately for the places that need the label without the surface.
 */

import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { cn } from "@/lib/utils";
import { GlassPanel } from "../glass-panel";

export interface EyebrowProps {
  /** "01" — the card's place in its view. Omit for an unnumbered label. */
  index?: string;
  children: ReactNode;
  className?: string;
}

export function Eyebrow({ index, children, className }: EyebrowProps) {
  return (
    <p className={cn("flex items-center gap-2 font-mono text-eyebrow uppercase text-text-tertiary", className)}>
      {index ? <span className="text-accent tabular-nums">{index}</span> : null}
      <span>{children}</span>
    </p>
  );
}

export type CardTone = "default" | "hero";

export interface CardProps extends Omit<ComponentPropsWithoutRef<"section">, "title"> {
  index?: string;
  title?: string;
  /** Right-hand side of the header: a segmented control, a button. */
  actions?: ReactNode;
  tone?: CardTone;
  /** Drop the body padding for content that brings its own: a list, a table. */
  flush?: boolean;
}

export function Card({ index, title, actions, tone = "default", flush = false, className, children, ...rest }: CardProps) {
  const hasHeader = title !== undefined || actions !== undefined;

  return (
    <GlassPanel material="elevated" radius="card" noise={false} className={cn("flex flex-col", className)} {...rest}>
      {tone === "hero" ? (
        <span aria-hidden="true" className="dot-grid dot-grid-fade pointer-events-none absolute inset-0 -z-10" />
      ) : null}

      {hasHeader ? (
        <header className="flex min-h-[var(--card-header-height)] items-center justify-between gap-3 px-[var(--card-padding)] pt-[var(--card-padding)]">
          {title !== undefined ? <Eyebrow index={index}>{title}</Eyebrow> : <span />}
          {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
        </header>
      ) : null}

      <div className={cn("min-w-0", flush ? "" : "p-[var(--card-padding)]", hasHeader && !flush && "pt-3")}>{children}</div>
    </GlassPanel>
  );
}
