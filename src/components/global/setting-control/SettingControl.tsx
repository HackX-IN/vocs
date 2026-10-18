/**
 * SOURCE OF TRUTH KEYWORDS: SettingControl, SettingControlProps, SettingDef,
 *   renderControl, settings-row
 * WHAT:  Renders one settings row from a SettingDef — label and description on
 *        the left, the control for its `kind` right-aligned.
 * WHY:   It branches on `kind` and nothing else. A branch on a feature name
 *        here would put a second source of truth next to the Rust registry, and
 *        the registry is the only place that is allowed to know what the app
 *        has (CLAUDE.md §3). That is also why the definition arrives whole as
 *        one prop rather than as spread fields: the generated registry mirror
 *        hands over an object, and destructuring it here would mean editing
 *        this file every time the registry grows a field.
 * WHERE: The settings view maps the registry's SettingDefs straight onto this.
 *        Controls live in ./controls.tsx, types in ./types.ts.
 */

import { cn } from "@/lib/utils";
import { HotkeyControl, isLongText, NumberControl, SelectControl, TextControl, ToggleControl } from "./controls";
import type { SettingDef } from "./types";

export interface SettingControlProps {
  setting: SettingDef;
  className?: string;
}

function renderControl(setting: SettingDef) {
  switch (setting.kind) {
    case "toggle":
      return <ToggleControl setting={setting} />;
    case "select":
      return <SelectControl setting={setting} />;
    case "number":
      return <NumberControl setting={setting} />;
    case "text":
      return <TextControl setting={setting} />;
    case "hotkey":
      return <HotkeyControl setting={setting} />;
    case "custom":
      return setting.control;
  }
}

/**
 * SOURCE OF TRUTH KEYWORDS: setting_option_detail, stacked_setting
 * WHAT:  What the row says about the CURRENT value of a select — the chosen
 *        option's own description.
 * WHY:   A native <select> can only show a label, so what each option MEANS
 *        was reachable only as a hover tooltip, and a setting whose options need
 *        explaining ("Casual", "Literal") was a guess. Showing the chosen
 *        option's sentence under the row costs one line and removes the guess.
 */
function selectedDetail(setting: SettingDef): string | undefined {
  if (setting.kind !== "select") return undefined;
  return setting.options.find((option) => option.value === setting.value)?.description;
}

export function SettingControl({ setting, className }: SettingControlProps) {
  const detail = selectedDetail(setting);
  // A long free-text field gets the whole row under its label: squeezed beside
  // the label it is a three-line postage stamp.
  const stacked = setting.kind === "text" && isLongText(setting);

  return (
    <div
      className={cn(
        "flex gap-x-4 gap-y-2 py-3",
        stacked ? "flex-col" : "items-center",
        setting.disabled && "opacity-50",
        className,
      )}
    >
      <div className="min-w-0 flex-1">
        <label htmlFor={setting.id} className="text-body text-text-primary">
          {setting.label}
        </label>
        {setting.description ? <p className="text-caption text-text-secondary">{setting.description}</p> : null}
        {detail ? <p className="mt-1 text-caption text-text-tertiary">{detail}</p> : null}
      </div>
      <div className={cn("flex items-center", stacked ? "w-full" : "shrink-0 justify-end")}>{renderControl(setting)}</div>
    </div>
  );
}
