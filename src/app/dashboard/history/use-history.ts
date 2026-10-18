/**
 * SOURCE OF TRUTH KEYWORDS: useHistory, HistoryFeed, listHistory, searchHistory,
 *   PAGE_SIZE, loadMore, exhausted
 * WHAT:  The history feed: a paginated list_history narrowed by a filter (search
 *        phrase, period, status), with loadMore/refresh and the rows it has so
 *        far.
 * WHY:   History reaches tens of thousands of rows, so it is never fetched
 *        whole — pages accumulate as the virtualized list reaches its end.
 *        Search and filters are applied by the SERVER, in the same query as the
 *        paging, because filtering client-side would only ever cover the
 *        handful of pages in memory and would quietly report "no results" for a
 *        session that exists — and a searched or filtered list scrolls and pages
 *        exactly like the plain one. A generation counter guards the swap: a page
 *        requested before the filter changed must not be appended after it.
 * WHERE: Owned by HistoryView.tsx. Wraps list_history.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type AppError, type HistoryFilter, type SessionSummary } from "@/lib/bindings";
import { unwrapCommand } from "@/lib/ipc";

/** Big enough that a page lasts several screens, small enough to stay instant. */
const PAGE_SIZE = 200;

export interface HistoryFeed {
  items: readonly SessionSummary[];
  error: AppError | null;
  loading: boolean;
  /** True once a page came back short — there is nothing further to ask for. */
  exhausted: boolean;
  /** True once the first response has landed, success or failure. Until then
   *  "no rows" means "not asked yet", which must not be drawn as "none". */
  loaded: boolean;
  loadMore: () => void;
  refresh: () => void;
  /** Drops one row locally after a confirmed delete, without refetching. */
  forget: (id: string) => void;
  /** Applies a confirmed change to one row without refetching the page. */
  patch: (id: string, change: Partial<SessionSummary>) => void;
}

export function useHistory(filter: HistoryFilter): HistoryFeed {
  const [items, setItems] = useState<readonly SessionSummary[]>([]);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(true);
  const [exhausted, setExhausted] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [generation, setGeneration] = useState(0);
  const offset = useRef(0);
  const inFlight = useRef(false);

  // Destructured to primitives, so a new object with the same values does not
  // restart the feed.
  const { query, since_ms, status } = filter;

  const fetchPage = useCallback(
    async (from: number, forGeneration: number) => {
      if (inFlight.current) return;
      inFlight.current = true;
      setLoading(true);

      const result = await unwrapCommand(() =>
        commands.listHistory({ filter: { query, since_ms, status }, limit: PAGE_SIZE, offset: from }),
      );

      inFlight.current = false;
      // The query changed while this was in flight — its rows are stale.
      if (forGeneration !== generation) return;

      if (result.status === "error") {
        setError(result.error);
        setLoading(false);
        setLoaded(true);
        return;
      }

      setError(null);
      setLoading(false);
      setLoaded(true);
      setExhausted(result.data.length < PAGE_SIZE);
      offset.current = from + result.data.length;
      setItems((previous) => (from === 0 ? result.data : [...previous, ...result.data]));
    },
    [generation, query, since_ms, status],
  );

  useEffect(() => {
    offset.current = 0;
    setExhausted(false);
    setLoaded(false);
    void fetchPage(0, generation);
    // fetchPage is recreated whenever its inputs change, which is exactly when
    // the feed must restart.
  }, [fetchPage, generation]);

  const loadMore = useCallback(() => {
    if (exhausted || loading) return;
    void fetchPage(offset.current, generation);
  }, [exhausted, fetchPage, generation, loading]);

  const refresh = useCallback(() => setGeneration((value) => value + 1), []);

  const forget = useCallback((id: string) => {
    setItems((previous) => previous.filter((item) => item.id !== id));
  }, []);

  const patch = useCallback((id: string, change: Partial<SessionSummary>) => {
    setItems((previous) => previous.map((item) => (item.id === id ? { ...item, ...change } : item)));
  }, []);

  return { items, error, loading, exhausted, loaded, loadMore, refresh, forget, patch };
}
