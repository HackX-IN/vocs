/**
 * SOURCE OF TRUTH KEYWORDS: SettingsView, getRegistry, getSettings, setSetting,
 *   SettingSection, settings-pages, settings-search, toControlSetting, SettingsNav
 * WHAT:  The Settings screen: a section list with search on the left, and on the
 *        right the cards of the chosen page — or the search results.
 * WHY:   Nothing here lists the settings. The rows come from the registry and
 *        each page shows the registry section it is mapped to (settings-pages.ts),
 *        so adding a setting in Rust makes it appear here with no frontend change
 *        (CLAUDE.md §7). This file owns what the pages share: the data they read
 *        (settings, devices, models, languages, engine, permissions), the dynamic
 *        option lists, and the write path.
 *
 *        Writes go through set_setting and nothing is patched locally — the
 *        settings-changed event brings the new map back, so the display is
 *        correct whoever changed it and whether or not the write was accepted. A
 *        rejected write therefore shows the old value AND the reason, instead of
 *        leaving the control sitting on a value the backend refused.
 *
 *        The page is the ROUTE (`#/settings/<page>`), so a deep link from an
 *        error action opens the right page and the back/forward history of the
 *        window is the history of the screen.
 * WHERE: Rendered by Dashboard.tsx for the registry's "settings" route.
 */

import { useCallback, useMemo, useState } from "react";
import { commands, type AppError, type SettingDef as RegistrySettingDef, type SettingSection, type SettingValue, type RegistrySnapshot } from "@/lib/bindings";
import { unwrapCommand, useCommand } from "@/lib/ipc";
import { usePermissions } from "@/lib/use-permissions";
import { formatBytes } from "@/lib/format";
import { ErrorSurface, ScrollArea, Skeleton } from "@/components/global";
import type { SettingOption } from "@/components/global";
import { navigateTo } from "../use-hash-route";
import { useSettings } from "../use-settings";
import { SettingsNav } from "./_components/SettingsNav";
import { SettingsPageBody, SettingsResults, type SettingsContext } from "./_components/SettingsPageBody";
import { pageForSection, resolvePage, type SettingsPageId } from "./settings-pages";
import type { DynamicOptions } from "./to-setting-def";

export interface SettingsViewProps {
  registry: RegistrySnapshot;
  /** Deep-link target — a page id, e.g. from an error action. */
  section: string | null;
}

export function SettingsView({ registry, section }: SettingsViewProps) {
  const settings = useSettings();
  const devices = useCommand(commands.listInputDevices, []);
  const models = useCommand(commands.listModels, []);
  const languages = useCommand(commands.listLanguages, []);
  const engine = useCommand(commands.getEngineCapabilities, []);
  // Re-checked on window focus: the user grants this in System Settings, in
  // another app, and comes back expecting these controls to have noticed.
  const permissions = usePermissions();
  const [writeError, setWriteError] = useState<AppError | null>(null);
  const [query, setQuery] = useState("");
  // Bumped by a backup import: the dictionary and profile lists fetch once on
  // mount, so a new key is how they are told what they show has changed.
  const [dataVersion, setDataVersion] = useState(0);
  const onDataImported = useCallback(() => setDataVersion((version) => version + 1), []);

  const dynamic = useMemo<DynamicOptions>(() => {
    const deviceOptions: SettingOption[] = (devices.data ?? []).map((device) => ({
      value: device.id,
      label: device.is_default ? `${device.name} (system default)` : device.name,
    }));
    const modelOptions: SettingOption[] = (models.data ?? []).map((model) => ({
      value: model.descriptor.id,
      label: model.descriptor.display_name,
      description: `${formatBytes(model.descriptor.size_bytes)} · ${model.descriptor.approx_ram_mb} MB memory`,
      disabled: model.state.kind !== "READY",
    }));
    // Unsupported languages are shown and disabled, never dropped: a language
    // that silently vanished from the list is one the user cannot work out how
    // to get back. The reason goes in the label because a native <option>
    // cannot render anything else.
    // Named only once known. "the selected engine" is true whether or not the
    // capabilities call has landed, so the reason is never briefly wrong.
    const engineName = engine.data?.display_name ?? "the selected engine";
    const languageOptions: SettingOption[] = (languages.data ?? []).map((language) => ({
      value: language.code,
      label: language.supported ? language.label : `${language.label} — not supported by ${engineName}`,
      description: language.supported ? undefined : `${engineName} cannot transcribe ${language.label}.`,
      disabled: !language.supported,
    }));
    return { INPUT_DEVICES: deviceOptions, MODELS: modelOptions, LANGUAGES: languageOptions };
  }, [devices.data, engine.data, languages.data, models.data]);

  const write = useCallback((key: string, value: SettingValue) => {
    // No reload here: settings-changed brings the new map back on its own.
    void unwrapCommand(() => commands.setSetting({ key, value })).then((result) => {
      setWriteError(result.status === "error" ? result.error : null);
    });
  }, []);

  const allDefs = useMemo(() => registry.capabilities.flatMap((capability) => capability.settings), [registry]);

  const grouped = useMemo(() => {
    const map = new Map<SettingsPageId, RegistrySettingDef[]>();
    for (const def of allDefs) {
      const id = pageForSection(def.section as SettingSection).id;
      const bucket = map.get(id);
      if (bucket) bucket.push(def);
      else map.set(id, [def]);
    }
    return map;
  }, [allDefs]);

  const page = resolvePage(section);
  const searching = query.trim().length > 0;

  if (settings.error) return <ErrorSurface error={settings.error} onRetry={settings.reload} />;

  // Until the stored values land, every control would render from its registry
  // default — a toggle showing off and then flipping on reads as the app having
  // forgotten the user's preference, which is worse than showing nothing.
  if (!settings.data) {
    return (
      <ScrollArea contentClassName="px-[var(--page-padding-x)]">
        <Skeleton rows={8} />
      </ScrollArea>
    );
  }

  const context: SettingsContext = {
    values: settings.data,
    dynamic,
    engine: engine.data,
    permissions: permissions.data,
    onWrite: write,
  };

  return (
    <div className="flex min-h-0 flex-1">
      {/* The list is fixed while the cards scroll, so the page you are on is
          always in view. It sits below the page header band like the content. */}
      <div className="shrink-0 pt-[var(--page-header-height)] pl-[var(--page-padding-x)]">
        <SettingsNav
          activeId={page.id}
          onSelect={(id) => {
            setQuery("");
            navigateTo("settings", id);
          }}
          query={query}
          onQueryChange={setQuery}
        />
      </div>

      <ScrollArea className="flex-1" contentClassName="flex flex-col gap-4 pr-[var(--page-padding-x)] pl-6 pb-8">
        {writeError ? <ErrorSurface size="compact" error={writeError} /> : null}
        {searching ? (
          <SettingsResults query={query} defs={allDefs} context={context} />
        ) : (
          <SettingsPageBody
            key={page.id}
            page={page}
            defs={grouped.get(page.id) ?? []}
            allDefs={allDefs}
            context={context}
            dataVersion={dataVersion}
            onDataImported={onDataImported}
          />
        )}
      </ScrollArea>
    </div>
  );
}
