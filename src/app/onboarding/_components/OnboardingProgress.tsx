/**
 * SOURCE OF TRUTH KEYWORDS: OnboardingProgress, ONBOARDING_STEPS, onboarding_stepper, step_index
 * WHAT:  The row of four numbered step labels across the top of first run: Tour,
 *        Access, Model, Try — the current one lit in ink with an accent numeral,
 *        finished ones ticked, the rest dim.
 * WHY:   First run is four screens and a person deserves to know how many. The
 *        numerals and mono capitals are the same instrument-panel convention as
 *        every card in the app (docs/04 §1), so setup already looks like the
 *        product. It is display only: the step is DERIVED from what the backend
 *        reports (Onboarding.tsx), so nothing here can be clicked to skip ahead
 *        of a permission or a download.
 * WHERE: Top of the Onboarding window, under the drag strip.
 */

import { Check } from "lucide-react";
import { cn } from "@/lib/utils";

export const ONBOARDING_STEPS = ["Tour", "Access", "Model", "Try"] as const;

export function OnboardingProgress({ current }: { current: number }) {
  return (
    <ol aria-label="Setup progress" className="flex items-center justify-center gap-5 px-8">
      {ONBOARDING_STEPS.map((label, index) => {
        const done = index < current;
        const active = index === current;
        return (
          <li
            key={label}
            aria-current={active ? "step" : undefined}
            className={cn(
              "flex items-center gap-2 font-mono text-eyebrow uppercase transition-colors",
              active ? "text-text-primary" : "text-text-tertiary",
            )}
          >
            {done ? (
              <Check className="size-3.5 text-accent" aria-hidden="true" />
            ) : (
              <span className={cn("tabular-nums", active ? "text-accent" : "text-text-tertiary")}>
                {String(index + 1).padStart(2, "0")}
              </span>
            )}
            <span>{label}</span>
          </li>
        );
      })}
    </ol>
  );
}
