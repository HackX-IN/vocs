/**
 * SOURCE OF TRUTH KEYWORDS: Button, ButtonProps, buttonClass, button-variants
 * WHAT:  The app's one push button: a hairline-bordered sunken control with a
 *        default and a destructive variant.
 * WHY:   The same class string had been pasted into the history export, the
 *        dictionary manager, the profile editor and the delete-history control,
 *        which is exactly the drift a global component exists to stop — change
 *        a radius in one and the others disagree. Variants are a cva map so a
 *        new one is a line here rather than another fork. `buttonClass` is
 *        exported for the rare element that must be a different tag (a label
 *        wrapping a file input) and still needs to look like this.
 * WHERE: Settings (mic check, import / export), history export, and any new
 *        view that needs a button.
 */

import type { ButtonHTMLAttributes, Ref } from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

export const buttonClass = cva(
  "hairline inline-flex h-8 shrink-0 items-center justify-center gap-2 rounded-input px-3 text-body transition-colors disabled:opacity-50",
  {
    variants: {
      variant: {
        default: "bg-sunken text-text-primary hover:bg-sunken-strong",
        danger: "bg-danger text-opaque-elevated",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof buttonClass> {
  ref?: Ref<HTMLButtonElement>;
}

export function Button({ variant, className, type = "button", ref, ...rest }: ButtonProps) {
  return <button ref={ref} type={type} className={cn(buttonClass({ variant }), className)} {...rest} />;
}
