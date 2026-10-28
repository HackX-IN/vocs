/**
 * SOURCE OF TRUTH KEYWORDS: Dialog, DialogProps, native-dialog, showModal, scrim,
 *   dialog-width
 * WHAT:  A modal dialog: a centred elevated panel over a dimmed page, controlled by
 *        `open`, closed by Escape, a click on the scrim, or whatever calls `onClose`.
 * WHY:   Built on the platform's `<dialog>` and `showModal()` rather than a div and
 *        a focus-trap library. The browser traps focus, makes everything behind it
 *        inert, restores focus on close and handles Escape — the parts of a modal
 *        that are easy to get subtly wrong — and WKWebView (macOS 13+) supports it.
 *        The only thing left to write is the sync between React's `open` and the
 *        element's own state, including the case where the browser closes it first
 *        (Escape), which is reported through `onClose` so the parent's state follows.
 *
 *        Everything visual is a token: the surface is `material-elevated`, the
 *        corners `--radius-panel`, the width `--dialog-width`, the dim `--scrim`.
 *        Initial focus: mark the control that should take it with `data-autofocus`.
 * WHERE: The What's new dialog. Any future once-in-a-while sheet that is chosen or
 *        announced rather than an error (an error is an ErrorSurface).
 */

import { useEffect, useRef, type ReactNode } from "react";
import { cn } from "@/lib/utils";

export interface DialogProps {
  open: boolean;
  /** Called when the person dismisses it by any route, so the parent can clear `open`. */
  onClose: () => void;
  /** Names the dialog for assistive tech. */
  label: string;
  children: ReactNode;
  className?: string;
}

export function Dialog({ open, onClose, label, children, className }: DialogProps) {
  const element = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = element.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      dialog.showModal();
      // showModal() focuses the first focusable thing in DOM order, which is rarely
      // the right one (in a carousel it is a pager dot). The caller names the control
      // that should take focus with `data-autofocus`, as the platform's `autofocus`
      // would — React only honours that attribute at mount, before the dialog is open.
      dialog.querySelector<HTMLElement>("[data-autofocus]")?.focus();
    }
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={element}
      aria-label={label}
      // Fires for Escape and for `close()` alike; the parent decides what closing means.
      onClose={onClose}
      onClick={(event) => {
        // A click on the backdrop lands on the dialog element itself, not on its content.
        if (event.target === element.current) onClose();
      }}
      className={cn(
        "material-elevated m-auto w-[var(--dialog-width)] max-w-[calc(100vw-2*var(--space-5))] overflow-hidden rounded-panel p-0 text-text-primary",
        "backdrop:bg-scrim backdrop:backdrop-blur-sm",
        className,
      )}
    >
      {open ? children : null}
    </dialog>
  );
}
