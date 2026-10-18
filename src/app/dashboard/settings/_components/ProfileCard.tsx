/**
 * SOURCE OF TRUTH KEYWORDS: ProfileCard, per_app_profile_card, profile_header, excluded_app,
 *   profile_avatar, AppProfile
 * WHAT:  One per-app profile as a card: who it is for (initial, editable name,
 *        bundle id), an on/off switch and delete, then its settings as rows —
 *        turn Vocs off here, writing style, and any other overrides.
 * WHY:   The profile used to be a loose stack of a checkbox, a name field, a
 *        bundle id and a second checkbox, with the real controls hanging off
 *        the left edge. A card gives the app an identity and puts every choice in
 *        the same row layout the rest of Settings uses (SettingControl), so a
 *        per-app setting reads exactly like the global one it shadows.
 *
 *        "Turn Vocs off in this app" is the one setting that makes the rest
 *        moot: no recording, nothing pasted. When it is on, the style and
 *        override rows are hidden rather than left editable, because a control
 *        that can be changed and does nothing is the kind this app does not ship.
 * WHERE: AppProfiles.tsx, once per saved profile.
 */

import { Trash2 } from "lucide-react";
import type {
  AppProfile,
  EngineCapabilities,
  PermissionReport,
  SettingDef as RegistrySettingDef,
  SettingValue,
} from "@/lib/bindings";
import { SettingControl } from "@/components/global";
import { cn } from "@/lib/utils";
import type { DynamicOptions } from "../to-setting-def";
import { ProfileOverrides } from "./ProfileOverrides";
import { ProfileTone, TONE_KEY } from "./ProfileTone";

export interface ProfileCardProps {
  profile: AppProfile;
  defs: readonly RegistrySettingDef[];
  globals: { [key in string]: SettingValue } | null;
  dynamic: DynamicOptions;
  engine: EngineCapabilities | null;
  permissions: readonly PermissionReport[] | null;
  onSave: (profile: AppProfile) => void;
  onDelete: (profile: AppProfile) => void;
}

export function ProfileCard({ profile, defs, globals, dynamic, engine, permissions, onSave, onDelete }: ProfileCardProps) {
  const excluded = profile.excluded ?? false;

  return (
    <li className={cn("hairline flex flex-col rounded-card bg-sunken/40 px-4", !profile.enabled && "opacity-60")}>
      <div className="flex items-center gap-3 py-3">
        <span
          aria-hidden="true"
          className="flex size-9 shrink-0 items-center justify-center rounded-input bg-sunken-strong text-body font-medium text-text-primary"
        >
          {profile.display_name.charAt(0).toUpperCase() || "?"}
        </span>
        <div className="flex min-w-0 flex-1 flex-col">
          <input
            defaultValue={profile.display_name}
            aria-label={`Name for ${profile.bundle_id}`}
            onBlur={(event) => {
              const name = event.target.value.trim();
              if (name.length > 0 && name !== profile.display_name) onSave({ ...profile, display_name: name });
            }}
            className="w-full rounded-input bg-transparent text-body font-medium text-text-primary outline-none hover:bg-sunken focus:bg-sunken"
          />
          <span className="truncate text-caption text-text-tertiary">{profile.bundle_id}</span>
        </div>
        <SettingControl
          className="!p-0"
          setting={{
            kind: "toggle",
            id: `profile-enabled-${profile.bundle_id}`,
            label: "",
            value: profile.enabled,
            onChange: (enabled) => onSave({ ...profile, enabled }),
          }}
        />
        <button
          type="button"
          aria-label={`Delete profile for ${profile.display_name}`}
          onClick={() => onDelete(profile)}
          className="shrink-0 rounded-input p-1.5 text-text-secondary transition-colors hover:bg-sunken hover:text-danger"
        >
          <Trash2 className="size-4" />
        </button>
      </div>

      <div className="hairline-t flex flex-col">
        <SettingControl
          className="hairline-b"
          setting={{
            kind: "toggle",
            id: `profile-excluded-${profile.bundle_id}`,
            label: `Turn Vocs off in ${profile.display_name}`,
            description: "No recording and nothing pasted while this app is in front. For password managers and the like.",
            value: excluded,
            onChange: (value) => onSave({ ...profile, excluded: value }),
          }}
        />

        {excluded ? (
          <p className="py-3 text-caption text-text-tertiary">
            Vocs is off in {profile.display_name}, so there is nothing else to set here.
          </p>
        ) : (
          <>
            <ProfileTone profile={profile} defs={defs} onChange={onSave} />
            <ProfileOverrides
              profile={profile}
              // Writing style has its own picker above; listing it here too would
              // be two controls for one override.
              defs={defs.filter((def) => def.key !== TONE_KEY)}
              globals={globals}
              dynamic={dynamic}
              engine={engine}
              permissions={permissions}
              onChange={onSave}
            />
          </>
        )}
      </div>
    </li>
  );
}
