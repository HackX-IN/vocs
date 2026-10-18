/**
 * SOURCE OF TRUTH KEYWORDS: HistoryView, useHistory, DataList, copyText, groupByDay,
 *   search-debounce, HistoryRow, DictationEditor, dictations-list, search-dock
 * WHAT:  The Dictations page: a virtualised, searchable list of past sessions
 *        grouped by day, and — when the route carries a session id — the editor
 *        for that one dictation.
 * WHY:   The list is the global DataList doing the generic half — window, ⌘F,
 *        arrow keys, ⏎ — while this file supplies the rows, the day headers
 *        (which the list steps over), the summary strip and the empty state.
 *        Search is handed to the SERVER (no `matches` prop): searching the pages
 *        that happen to be loaded would report "no results" for a session that
 *        exists, which is worse than no search. The query is debounced by the
 *        token rather than fired per keystroke, because each keystroke is an FTS
 *        query across every row. Copying goes through the copy_text command, not
 *        navigator.clipboard: the web API needs transient user activation and a
 *        permissive CSP, so a copy from a keyboard-navigated row is exactly the
 *        case it refuses; Rust owns the guarantee, on the path the paste
 *        pipeline already uses.
 *
 *        THE LIST STAYS MOUNTED while the editor is open (it is hidden, not
 *        removed). Coming back from a dictation to the exact scroll position,
 *        query and loaded pages is what makes the pair feel like one notebook
 *        rather than two screens, and rebuilding a 200-row page to restore it
 *        would be slower than never having left. The editor reports its saves
 *        back, so the row behind it already shows the new name when it returns.
 * WHERE: Rendered by Dashboard.tsx for the registry's "history" route. `section`
 *        is the session id from the hash (#/history/<id>).
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Check, Copy } from "lucide-react";
import { commands, type AppError, type HotkeyBinding, type SessionSummary } from "@/lib/bindings";
import { unwrapCommand, useCommand } from "@/lib/ipc";
import { readDurationMs, readPxToken } from "@/lib/motion";
import { cn } from "@/lib/utils";
import { Button, DataList, EmptyState, ErrorSurface, Skeleton } from "@/components/global";
import type { DictationMode } from "@/lib/dictation-mode";
import { NoTranscriptionsYet } from "../_components/NoTranscriptionsYet";
import { navigateTo } from "../use-hash-route";
import { DictationEditor } from "./_components/DictationEditor";
import { ExportAction } from "./_components/ExportAction";
import { HistoryHeaderRow, HistoryRow } from "./_components/HistoryRow";
import { HistorySummary } from "./_components/HistorySummary";
import { groupByDay, type HistoryEntry } from "./day-groups";
import { effectiveText } from "./session-text";
import { useHistory } from "./use-history";

export interface HistoryViewProps {
  /** The dictation hotkey, for the empty state. Null while it is unknown. */
  hotkey: HotkeyBinding | null;
  mode: DictationMode;
  /** The open dictation's id, or null for the list. */
  section: string | null;
}

const isSession = (entry: HistoryEntry): boolean => entry.kind === "session";

export function HistoryView({ hotkey, mode, section }: HistoryViewProps) {
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [exportError, setExportError] = useState<AppError | null>(null);
  const feed = useHistory(query);
  const stats = useCommand(commands.getStats, [feed.items.length]);
  const copyTimer = useRef(0);
  const [summaryHeight] = useState(() => readPxToken("--history-summary-height"));

  const entries = useMemo(() => groupByDay(feed.items), [feed.items]);
  const showSummary = query.length === 0 && stats.data !== null && stats.data.total_sessions > 0;
  const editing = section !== null;

  useEffect(() => {
    const handle = window.setTimeout(() => setQuery(input), readDurationMs("--search-debounce"));
    return () => window.clearTimeout(handle);
  }, [input]);

  useEffect(() => () => window.clearTimeout(copyTimer.current), []);

  const copy = useCallback((session: SessionSummary) => {
    const text = effectiveText(session) ?? "";
    if (text.length === 0) return;
    void unwrapCommand(() => commands.copyText({ text })).then((result) => {
      if (result.status !== "ok") return;
      setCopiedId(session.id);
      window.clearTimeout(copyTimer.current);
      copyTimer.current = window.setTimeout(() => setCopiedId(null), readDurationMs("--feedback-hold"));
    });
  }, []);

  if (feed.error && feed.items.length === 0) {
    return (
      <section className="flex h-full flex-col pt-[var(--page-header-height)]">
        <ErrorSurface error={feed.error} onRetry={feed.refresh} />
      </section>
    );
  }

  return (
    <>
      <section className={cn("flex min-h-0 flex-1 flex-col", editing && "hidden")}>
        {exportError ? (
          <div className="px-[var(--page-padding-x)] pt-[var(--page-header-height)]">
            <ErrorSurface size="compact" error={exportError} />
          </div>
        ) : null}
        <DataList
          label="Dictations"
          items={entries}
          getKey={(entry) => entry.key}
          isSelectable={isSession}
          activateOn="click"
          dividers={false}
          headerInset
          searchPosition="dock"
          rowClassName="mx-auto w-full max-w-[var(--content-max-width)] rounded-input px-3"
          lead={showSummary && stats.data ? <HistorySummary data={stats.data} /> : undefined}
          leadHeight={showSummary ? summaryHeight : 0}
          onActivate={(entry) => {
            if (entry.kind === "session") navigateTo("history", entry.session.id);
          }}
          onReachEnd={feed.loadMore}
          renderRow={({ item }) =>
            item.kind === "header" ? <HistoryHeaderRow entry={item} /> : <HistoryRow session={item.session} />
          }
          renderRowActions={({ item }) =>
            item.kind === "session" ? (
              <Button
                variant="ghost"
                aria-label="Copy transcript"
                // The row opens on click; copying must not also open it.
                onClick={(event) => {
                  event.stopPropagation();
                  copy(item.session);
                }}
                className="size-7 px-0"
              >
                {copiedId === item.session.id ? <Check className="size-4 text-success" /> : <Copy className="size-4" />}
              </Button>
            ) : null
          }
          search={{ query: input, onQueryChange: setInput, placeholder: "Search dictations", hotkey: !editing }}
          toolbar={<ExportAction onError={setExportError} />}
          empty={
            !feed.loaded ? (
              <Skeleton rows={6} className="h-[var(--row-height)] rounded-none" />
            ) : (
              <NoTranscriptionsYet hotkey={hotkey} mode={mode} />
            )
          }
          noResults={<EmptyState headline="Nothing matches that" description="Try a shorter search." />}
          footer={
            feed.loading && feed.items.length > 0 ? <Skeleton className="h-[var(--row-height)] rounded-none" /> : null
          }
        />
      </section>

      {editing ? (
        <DictationEditor
          id={section}
          onBack={() => navigateTo("history")}
          onChanged={(session) => feed.patch(session.id, session)}
          onDeleted={(id) => {
            feed.forget(id);
            navigateTo("history");
          }}
        />
      ) : null}
    </>
  );
}
