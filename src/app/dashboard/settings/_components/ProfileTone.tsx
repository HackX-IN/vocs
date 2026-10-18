/**
 * SOURCE OF TRUTH KEYWORDS: ProfileTone, tone_preset, writing_style, TONE_KEY, per_app_tone,
 *   follow_global
 * WHAT:  The writing-style picker at the top of a per-app profile: Default (follow
 *        the global setting), then Natural, Casual, Formal and Literal.
 * WHY:   A tone preset is not a new object — it is the `enhance.tone` setting
 *        overridden for one app — so this is a thin control over the profile's
 *        sparse overrides, and the options, their names and their descriptions
 *        are read from the registry's own SettingDef rather than being typed out
 *        a second time. "Default" removes the override, so the app goes back to
 *        following the global setting AND keeps following it as that changes; a
 *        control that wrote the current value instead would freeze the app at
 *        today's setting, which is exactly what the sparse design exists to
 *        prevent. It is a segmented control, not a select, because there are only
 *        five choices and they are the whole point of the profile — hiding four
 *        behind a click hides what the feature is.
 * WHERE: At the top of each profile card in AppProfiles.tsx.
 */

import type { AppProfile, SettingDef as RegistrySettingDef } from "@/lib/bindings";
import { SegmentedControl, type SegmentOption } from "@/components/global";

/** The registry key of the writing-style setting — see registry/keys.rs::TONE. */
export const TONE_KEY = "enhance.tone";

/** Sentinel for "no override" — never a stored value. */
const FOLLOW_GLOBAL = "__default__";

export interface ProfileToneProps {
  profile: AppProfile;
  /** Every registry definition; the tone's own options come out of it. */
  defs: readonly RegistrySettingDef[];
  onChange: (profile: AppProfile) => void;
}

export function ProfileTone({ profile, defs, onChange }: ProfileToneProps) {
  const def = defs.find((candidate) => candidate.key === TONE_KEY);
  if (!def || def.kind.kind !== "CHOICE") return null;

  const options: SegmentOption<string>[] = [
    { value: FOLLOW_GLOBAL, label: "Default" },
    ...def.kind.options.map((choice) => ({ value: choice.value, label: choice.label })),
  ];

  const stored = profile.overrides[TONE_KEY];
  const current = stored?.type === "CHOICE" ? stored.value : FOLLOW_GLOBAL;
  const described = def.kind.options.find((choice) => choice.value === current);

  const choose = (value: string) => {
    const { [TONE_KEY]: _dropped, ...rest } = profile.overrides;
    onChange({
      ...profile,
      overrides: value === FOLLOW_GLOBAL ? rest : { ...rest, [TONE_KEY]: { type: "CHOICE", value } },
    });
  };

  return (
    <div className="flex flex-col gap-1.5 pl-6">
      <div className="flex items-center justify-between gap-3">
        <span className="text-label text-text-secondary">Writing style</span>
        <SegmentedControl
          label={`Writing style for ${profile.display_name}`}
          options={options}
          value={current}
          onChange={choose}
        />
      </div>
      <p className="text-caption text-text-tertiary">
        {described?.description ?? "Follows the global Writing style setting."}
      </p>
    </div>
  );
}
