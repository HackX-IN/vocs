/**
 * SOURCE OF TRUTH KEYWORDS: SettingsPageBody, SettingsResults, SettingRows, advanced-disclosure,
 *   toControlSetting, settings-cards, numbered-cards
 * WHAT:  The right-hand side of Settings: one page's cards — its registry
 *        settings as rows, then the panels that belong with them — or, while
 *        searching, the matching settings from every page.
 * WHY:   Nothing here lists a setting. Rows come from the registry's SettingDefs
 *        through toControlSetting, so adding a setting in Rust makes it appear on
 *        its page with no change here (CLAUDE.md §7). What this file decides is
 *        the presentation: a page is one card of rows, plus a second card for
 *        the panel that belongs with it (models under Transcription, the
 *        dictionary under Vocabulary), numbered in reading order by position —
 *        the same instrument-panel convention as Stats.
 *
 *        Advanced settings sit behind a disclosure at the foot of their card:
 *        they exist to unstick a specific machine, not to be browsed. A missing
 *        OS permission is announced ONCE per page, above the cards, not once per
 *        blocked row — two controls behind the same grant are one problem with
 *        one fix.
 *
 *        Search results group matches under their page's name and render the
 *        very same rows, so a setting behaves identically whether it was found
 *        by browsing or by searching.
 * WHERE: Rendered by SettingsView.
 */

import { useState } from "react";
import { ChevronDown } from "lucide-react";
import type {
  EngineCapabilities,
  OsPermission,
  PermissionReport,
  SettingDef as RegistrySettingDef,
  SettingValue,
} from "@/lib/bindings";
import { missingPermissions } from "@/lib/use-permissions";
import { cn } from "@/lib/utils";
import { Card, EmptyState, Eyebrow, SettingControl } from "@/components/global";
import { toControlSetting, type DynamicOptions } from "../to-setting-def";
import { pageForSection, SETTINGS_PAGES, type SettingsPage } from "../settings-pages";
import { AppProfiles } from "./AppProfiles";
import { BackupPanel } from "./BackupPanel";
import { ClearHistoryControl } from "./ClearHistoryControl";
import { DictionaryManager } from "./DictionaryManager";
import { DiagnosticsPanel } from "./DiagnosticsPanel";
import { LearnedCorrections } from "./LearnedCorrections";
import { MicTestPanel } from "./MicTestPanel";
import { ModelManager } from "./ModelManager";
import { PermissionNotice } from "./PermissionNotice";

/** Everything a row needs to render and write — shared by every page. */
export interface SettingsContext {
  values: { [key in string]: SettingValue } | null;
  dynamic: DynamicOptions;
  engine: EngineCapabilities | null;
  permissions: readonly PermissionReport[] | null;
  onWrite: (key: string, value: SettingValue) => void;
}

const ROW_CLASS = "hairline-b last:border-b-0";

function SettingRows({ defs, context }: { defs: readonly RegistrySettingDef[]; context: SettingsContext }) {
  return (
    <>
      {defs.map((def) => (
        <SettingControl
          key={def.key}
          className={ROW_CLASS}
          setting={toControlSetting(
            def,
            context.values?.[def.key],
            context.dynamic,
            context.engine,
            context.permissions,
            (value) => context.onWrite(def.key, value),
          )}
        />
      ))}
    </>
  );
}

function AdvancedRows({ defs, context }: { defs: readonly RegistrySettingDef[]; context: SettingsContext }) {
  const [open, setOpen] = useState(false);
  if (defs.length === 0) return null;

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        className="flex w-fit items-center gap-1 pt-3 text-label text-text-secondary transition-colors hover:text-text-primary"
      >
        <ChevronDown className={cn("size-4 transition-transform", open && "rotate-180")} />
        Advanced
      </button>
      {open ? <SettingRows defs={defs} context={context} /> : null}
    </>
  );
}

export interface SettingsPageBodyProps {
  page: SettingsPage;
  /** The registry settings whose section this page shows. */
  defs: readonly RegistrySettingDef[];
  /** Every definition — per-app profiles override any of them. */
  allDefs: readonly RegistrySettingDef[];
  context: SettingsContext;
  /** Changes when an import has rewritten data some panels display. */
  dataVersion: number;
  onDataImported: () => void;
}

export function SettingsPageBody({ page, defs, allDefs, context, dataVersion, onDataImported }: SettingsPageBodyProps) {
  const plain = defs.filter((def) => !def.advanced);
  const advanced = defs.filter((def) => def.advanced);

  // One notice per distinct missing grant, not one per blocked control.
  const blocking = [
    ...new Set(defs.flatMap((def) => missingPermissions(def.requires_permission, context.permissions))),
  ] as OsPermission[];

  // Cards are numbered by position among those actually shown.
  const cards: { title: string; body: React.ReactNode }[] = [];

  if (page.id === "profiles") {
    cards.push({
      title: "Per-app profiles",
      body: (
        <AppProfiles
          key={dataVersion}
          defs={allDefs}
          globals={context.values}
          dynamic={context.dynamic}
          engine={context.engine}
          permissions={context.permissions}
        />
      ),
    });
  } else {
    if (defs.length > 0 || page.id === "recording") {
      cards.push({
        title: page.label,
        body: (
          <>
            <SettingRows defs={plain} context={context} />
            {page.id === "recording" ? <MicTestPanel /> : null}
            <AdvancedRows defs={advanced} context={context} />
          </>
        ),
      });
    }
    if (page.id === "transcription") cards.push({ title: "Models", body: <ModelManager kind="SPEECH" /> });
    if (page.id === "output") cards.push({ title: "Grammar model", body: <ModelManager kind="GRAMMAR" /> });
    if (page.id === "vocabulary") {
      cards.push({ title: "Dictionary", body: <DictionaryManager key={dataVersion} /> });
      cards.push({ title: "Learned corrections", body: <LearnedCorrections key={dataVersion} /> });
    }
    if (page.id === "privacy") {
      cards.push({
        title: "Your data",
        body: (
          <>
            <ClearHistoryControl />
            <DiagnosticsPanel />
          </>
        ),
      });
    }
    if (page.id === "general") {
      cards.push({ title: "Backup", body: <BackupPanel onImported={onDataImported} /> });
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <p className="text-body text-text-secondary">{page.blurb}</p>
      <PermissionNotice permissions={blocking} />
      {cards.map((card, index) => (
        <Card key={card.title} index={String(index + 1).padStart(2, "0")} title={card.title}>
          {card.body}
        </Card>
      ))}
    </div>
  );
}

export interface SettingsResultsProps {
  query: string;
  defs: readonly RegistrySettingDef[];
  context: SettingsContext;
}

/** Whether a definition answers a query — by what it is called or says. */
export function matchesQuery(def: RegistrySettingDef, query: string): boolean {
  const needle = query.trim().toLowerCase();
  return needle.length > 0 && `${def.label} ${def.description}`.toLowerCase().includes(needle);
}

export function SettingsResults({ query, defs, context }: SettingsResultsProps) {
  const hits = defs.filter((def) => matchesQuery(def, query));

  if (hits.length === 0) {
    return <EmptyState headline="No setting matches that" description="Try a shorter or different word." />;
  }

  // Grouped under the page each belongs to, in the pages' own order.
  const groups = SETTINGS_PAGES.map((page) => ({
    page,
    defs: hits.filter((def) => pageForSection(def.section).id === page.id),
  })).filter((group) => group.defs.length > 0);

  return (
    <div className="flex flex-col gap-4">
      <p className="text-body text-text-secondary">
        {hits.length} {hits.length === 1 ? "setting" : "settings"} matching “{query.trim()}”
      </p>
      {groups.map((group) => (
        <Card key={group.page.id} title={group.page.label}>
          <Eyebrow className="sr-only">{group.page.label}</Eyebrow>
          <SettingRows defs={group.defs} context={context} />
        </Card>
      ))}
    </div>
  );
}
