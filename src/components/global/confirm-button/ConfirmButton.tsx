/**
 * SOURCE OF TRUTH KEYWORDS: ConfirmButton, ConfirmButtonProps, two-step-confirm, destructive-confirm,
 *   cancel-confirm
 * WHAT:  A button for an action that cannot be undone: the first press swaps it
 *        for a Cancel and a red confirm button side by side, and only the
 *        confirm runs `onConfirm`.
 * WHY:   Two screens had grown their own version of "press again to confirm",
 *        and both had the same flaw: the only way out was to click somewhere
 *        else and hope a blur handler fired, so an armed delete just sat there
 *        looking like the button had changed its mind. Cancel is now an
 *        explicit control, Escape cancels as well, and focus lands on Cancel so
 *        pressing Enter by reflex backs out rather than deleting.
 *
 *        There is deliberately no disarm-on-blur. WebKit does not give a
 *        clicked button focus, so a blur handler can fire between the mouse
 *        going down and the click landing and swallow the very press that was
 *        meant to confirm.
 *
 *        `onConfirm` may be async; the pair stays up, disabled, until it
 *        settles, so a slow delete cannot be confirmed twice.
 * WHERE: The delete-all-history control in Settings, and the delete button in
 *        the dictation editor.
 */

import { useEffect, useRef, useState, type ReactNode } from "react";
import { Button, type ButtonProps } from "../button";

export interface ConfirmButtonProps {
  /** The idle button's face. */
  children: ReactNode;
  /** The red button's label once armed — says what will happen. */
  confirmLabel: string;
  onConfirm: () => void | Promise<void>;
  cancelLabel?: string;
  /** How the idle button looks; the armed confirm is always `danger`. */
  idleVariant?: ButtonProps["variant"];
  disabled?: boolean;
  className?: string;
  "aria-label"?: string;
}

export function ConfirmButton({
  children,
  confirmLabel,
  onConfirm,
  cancelLabel = "Cancel",
  idleVariant = "default",
  disabled,
  className,
  "aria-label": ariaLabel,
}: ConfirmButtonProps) {
  const [armed, setArmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const cancelRef = useRef<HTMLButtonElement>(null);

  // Escape backs out, from anywhere, while a confirmation is showing.
  useEffect(() => {
    if (!armed) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) setArmed(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [armed, busy]);

  // Focus the safe choice, so a reflexive Enter cancels.
  useEffect(() => {
    if (armed) cancelRef.current?.focus();
  }, [armed]);

  if (!armed) {
    return (
      <Button variant={idleVariant} className={className} disabled={disabled} aria-label={ariaLabel} onClick={() => setArmed(true)}>
        {children}
      </Button>
    );
  }

  const confirm = async () => {
    setBusy(true);
    try {
      await onConfirm();
    } finally {
      setBusy(false);
      setArmed(false);
    }
  };

  return (
    <div role="group" aria-label="Confirm" className="flex items-center gap-2">
      <Button ref={cancelRef} variant="ghost" disabled={busy} onClick={() => setArmed(false)}>
        {cancelLabel}
      </Button>
      <Button variant="danger" disabled={busy} onClick={() => void confirm()}>
        {confirmLabel}
      </Button>
    </div>
  );
}
