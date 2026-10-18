/**
 * SOURCE OF TRUTH KEYWORDS: DataList, DataListProps, DataListSearch, DataListRowArgs,
 *   useVirtualWindow, activeIndex, renderRow, renderRowActions, listbox
 * WHAT:  A virtualized, searchable, keyboard-navigable list. It owns the
 *        window, the query, the active row and the roving focus; the row, the
 *        row actions, the toolbar and both empty states arrive as slots.
 * WHY:   The generic half of a list is the half everyone rewrites badly —
 *        virtualization, ⌘F, arrow keys, page keys, scroll-into-view, aria
 *        wiring — and the specific half is only ever "what does a row look
 *        like". Search and pagination are both optional halves: omit `matches`
 *        and the SERVER owns filtering while the list still owns the field and
 *        ⌘F; supply `onReachEnd` and a paginated source loads as it scrolls.
 *        So this component knows nothing about what it is listing:
 *        no field names, no copy, no data shape, one type parameter. History
 *        is its first consumer, not its definition. Keyboard focus stays on
 *        the container with aria-activedescendant rather than moving DOM focus
 *        per row, because focusing a row that virtualization is about to
 *        unmount throws focus back to <body> mid-scroll.
 *
 *        Two layout slots exist because History grew a notebook look: rows can
 *        be NON-SELECTABLE (`isSelectable` — a day header is a row for the
 *        virtualiser, which needs one uniform height, but arrow keys step over
 *        it and it takes no hover or active state), and the search field can be
 *        DOCKED (`searchPosition="dock"`) — a floating bar over the foot of the
 *        list, with the toolbar inside it — instead of a row above.
 * WHERE: The dashboard history view, and anything else that lists rows. Uses
 *        use-virtual-window.ts; row height and overscan come from tokens.css.
 */

import {
  type CSSProperties,
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";
import { Search } from "lucide-react";
import { cn } from "@/lib/utils";
import { useScrollEdges } from "../scroll-area/use-scroll-edges";
import { readNumberToken, readPxToken } from "@/lib/motion";
import { useVirtualWindow } from "./use-virtual-window";

export interface DataListRowArgs<T> {
  item: T;
  index: number;
  isActive: boolean;
}

export interface DataListSearch<T> {
  /** Client-side filter. Keep it stable (useCallback) — it is a filter
   *  dependency over the whole collection. Omit it when the SERVER owns the
   *  search: the list then still owns the field, the query and ⌘F, and the
   *  caller re-fetches `items` on query change. */
  matches?: (item: T, query: string) => boolean;
  placeholder?: string;
  /** Controlled query. Omit both and the list owns its own. */
  query?: string;
  onQueryChange?: (query: string) => void;
  /** ⌘F focuses the field from anywhere in the window. Default true. */
  hotkey?: boolean;
}

export type DataListSearchPosition = "top" | "dock";

export interface DataListProps<T> {
  items: readonly T[];
  getKey: (item: T, index: number) => string;
  renderRow: (args: DataListRowArgs<T>) => ReactNode;
  /** Right-aligned per-row slot, revealed on hover or when the row is active. */
  renderRowActions?: (args: DataListRowArgs<T>) => ReactNode;
  /** Shown when there is nothing at all. */
  empty: ReactNode;
  /** Shown when a query matched nothing. Falls back to `empty`. */
  noResults?: ReactNode;
  /** A filter other than the text query is active, so an empty list means "nothing
   *  matches", not "there is nothing". */
  narrowed?: boolean;
  search?: DataListSearch<T>;
  /** Sits beside the search field — filters, sort, bulk actions. */
  toolbar?: ReactNode;
  /** ⏎ on the active row — and a click or a double click, per `activateOn`. */
  onActivate?: (item: T, index: number) => void;
  /** What opens a row with the pointer. A table wants a double click (a click
   *  selects); a list of documents wants a single click, like every list of
   *  documents a person has used. */
  activateOn?: "click" | "doubleClick";
  onActiveChange?: (item: T | null, index: number) => void;
  /** Fired when the rendered window comes within an overscan of the last row.
   *  Paginated sources load their next page here; it fires once per arrival at
   *  the end, not once per scroll event. */
  onReachEnd?: () => void;
  /** Rendered below the rows — a loading row, a "that's everything" line. */
  footer?: ReactNode;
  rowHeight?: number;
  overscan?: number;
  dividers?: boolean;
  /** Rows that cannot be selected — group headers. Skipped by the keyboard,
   *  and drawn without hover or active styling. Omit for "every row is". */
  isSelectable?: (item: T) => boolean;
  /** "top" puts search in a row above the list; "dock" floats it over the foot. */
  searchPosition?: DataListSearchPosition;
  /** Replaces the default row chrome (padding, hover shape). Dividers still
   *  follow `dividers`. */
  rowClassName?: string;
  /** Content above the rows that scrolls with them — a summary card. Its height
   *  must be given, because the virtualiser reads the scroll position past it. */
  lead?: ReactNode;
  leadHeight?: number;
  /** Pad the content down past the overlaying page header and fade under it,
   *  exactly as ScrollArea does for every other page. */
  headerInset?: boolean;
  /** Accessible name for the listbox — the caller's words, not ours. */
  label: string;
  className?: string;
}

export function DataList<T>({
  items,
  getKey,
  renderRow,
  renderRowActions,
  empty,
  noResults,
  narrowed = false,
  search,
  toolbar,
  onActivate,
  activateOn = "doubleClick",
  onActiveChange,
  onReachEnd,
  footer,
  rowHeight,
  overscan,
  dividers = true,
  isSelectable,
  searchPosition = "top",
  rowClassName,
  lead,
  leadHeight = 0,
  headerInset = false,
  label,
  className,
}: DataListProps<T>) {
  const listId = useId();
  const scrollRef = useRef<HTMLDivElement>(null);
  // Same scroller the virtualiser measures, so the fade can never disagree
  // with what is actually rendered.
  const edges = useScrollEdges(scrollRef);
  const inputRef = useRef<HTMLInputElement>(null);

  const [tokenRowHeight] = useState(() => readPxToken("--row-height"));
  const [tokenOverscan] = useState(() => readNumberToken("--row-overscan"));
  const [insetPx] = useState(() => (headerInset ? readPxToken("--page-header-height") : 0));
  // Everything above the first row inside the scroller, in pixels.
  const leadPx = insetPx + leadHeight;
  const height = rowHeight ?? tokenRowHeight;
  const overscanRows = overscan ?? tokenOverscan;

  const [internalQuery, setInternalQuery] = useState("");
  const query = search?.query ?? internalQuery;
  const matches = search?.matches;

  const filtered = useMemo(() => {
    if (!matches || query.length === 0) return items;
    return items.filter((item) => matches(item, query));
  }, [items, query, matches]);

  const selectableRef = useRef(isSelectable);
  selectableRef.current = isSelectable;
  const canSelect = useCallback((item: T) => selectableRef.current?.(item) ?? true, []);

  const [activeIndex, setActiveIndex] = useState(0);
  // The nearest selectable row to the requested one — forward first, then back.
  // A list that opens on a day header must still start with a real row active.
  const clampedActive = useMemo(() => {
    if (filtered.length === 0) return -1;
    const wanted = Math.min(activeIndex, filtered.length - 1);
    for (let index = wanted; index < filtered.length; index += 1) if (canSelect(filtered[index])) return index;
    for (let index = wanted - 1; index >= 0; index -= 1) if (canSelect(filtered[index])) return index;
    return -1;
  }, [activeIndex, canSelect, filtered]);

  const window_ = useVirtualWindow({
    count: filtered.length,
    rowHeight: height,
    overscan: overscanRows,
    containerRef: scrollRef,
    leadHeight: leadPx,
  });

  const onReachEndRef = useRef(onReachEnd);
  onReachEndRef.current = onReachEnd;
  const reachedFor = useRef(-1);
  useEffect(() => {
    if (!onReachEndRef.current || filtered.length === 0) return;
    const atEnd = window_.end >= filtered.length - overscanRows;
    // Latched on the length that triggered it, so one arrival at the end asks
    // for one page rather than one per scroll frame.
    if (atEnd && reachedFor.current !== filtered.length) {
      reachedFor.current = filtered.length;
      onReachEndRef.current();
    }
  }, [window_.end, filtered.length, overscanRows]);

  const onActiveChangeRef = useRef(onActiveChange);
  onActiveChangeRef.current = onActiveChange;
  useEffect(() => {
    onActiveChangeRef.current?.(clampedActive < 0 ? null : filtered[clampedActive], clampedActive);
  }, [clampedActive, filtered]);

  /** Keeps the active row on screen by arithmetic — nothing is measured. */
  const revealRow = useCallback(
    (index: number) => {
      const container = scrollRef.current;
      if (!container) return;
      const top = leadPx + index * height;
      // Keep the row clear of the page header's fade and the dock, not merely
      // inside the viewport — a row scrolled to the very edge is still hidden.
      if (top < container.scrollTop + insetPx) container.scrollTop = top - insetPx;
      else if (top + height > container.scrollTop + container.clientHeight) {
        container.scrollTop = top + height - container.clientHeight;
      }
    },
    [height, insetPx, leadPx],
  );

  const moveActive = useCallback(
    (next: number) => {
      if (filtered.length === 0) return;
      const bounded = Math.max(0, Math.min(next, filtered.length - 1));
      const direction = bounded >= clampedActive ? 1 : -1;
      // Step over headers in the direction of travel; if the edge of the list is
      // all headers, stay put rather than land on one.
      let target = bounded;
      while (target >= 0 && target < filtered.length && !canSelect(filtered[target])) target += direction;
      if (target < 0 || target >= filtered.length) return;
      setActiveIndex(target);
      revealRow(target);
    },
    [canSelect, clampedActive, filtered, revealRow],
  );

  const setQuery = useCallback(
    (next: string) => {
      setActiveIndex(0);
      if (scrollRef.current) scrollRef.current.scrollTop = 0;
      if (search?.onQueryChange) search.onQueryChange(next);
      else setInternalQuery(next);
    },
    [search],
  );

  useEffect(() => {
    if (!search || search.hotkey === false) return;
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.metaKey && event.key.toLowerCase() === "f") {
        event.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [search]);

  const onKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLElement>) => {
      const page = Math.max(1, Math.floor(window_.viewportHeight / height) - 1);
      switch (event.key) {
        case "ArrowDown":
          event.preventDefault();
          moveActive(clampedActive + 1);
          break;
        case "ArrowUp":
          event.preventDefault();
          moveActive(clampedActive - 1);
          break;
        case "PageDown":
          event.preventDefault();
          moveActive(clampedActive + page);
          break;
        case "PageUp":
          event.preventDefault();
          moveActive(clampedActive - page);
          break;
        case "Home":
          event.preventDefault();
          moveActive(0);
          break;
        case "End":
          event.preventDefault();
          moveActive(filtered.length - 1);
          break;
        case "Enter":
          if (clampedActive >= 0) {
            event.preventDefault();
            onActivate?.(filtered[clampedActive], clampedActive);
          }
          break;
        case "Escape":
          if (query.length > 0) {
            event.preventDefault();
            setQuery("");
          }
          break;
        default:
          break;
      }
    },
    [clampedActive, filtered, height, moveActive, onActivate, query, setQuery, window_.viewportHeight],
  );

  const rows: ReactNode[] = [];
  for (let index = window_.start; index < window_.end; index += 1) {
    const item = filtered[index];
    const isActive = index === clampedActive;
    const selectable = canSelect(item);
    const args: DataListRowArgs<T> = { item, index, isActive };
    rows.push(
      <div
        key={getKey(item, index)}
        id={`${listId}-row-${index}`}
        role={selectable ? "option" : "presentation"}
        aria-selected={selectable ? isActive : undefined}
        data-active={isActive}
        style={{ height }}
        onMouseDown={selectable ? () => setActiveIndex(index) : undefined}
        onClick={selectable && activateOn === "click" ? () => onActivate?.(item, index) : undefined}
        onDoubleClick={selectable && activateOn === "doubleClick" ? () => onActivate?.(item, index) : undefined}
        className={cn(
          "group flex items-center gap-3 cursor-default transition-colors",
          rowClassName ?? "px-4",
          dividers && selectable && "hairline-b",
          selectable && (isActive ? "bg-sunken-strong" : "hover:bg-sunken"),
        )}
      >
        <div className="min-w-0 flex-1">{renderRow(args)}</div>
        {renderRowActions && selectable ? (
          <div
            className={cn(
              "shrink-0 opacity-0 transition-opacity",
              "group-hover:opacity-100 group-data-[active=true]:opacity-100 focus-within:opacity-100",
            )}
          >
            {renderRowActions(args)}
          </div>
        ) : null}
      </div>,
    );
  }

  // Which empty state to show is decided by whether a QUERY is active, never by
  // comparing counts. With server-owned search `filtered` IS `items`, so a
  // search that matched nothing looks identical to having no data at all — and
  // telling someone with 4,000 transcripts that they have none, because they
  // searched for a typo, is the worst version of an empty state.
  const searching = query.trim().length > 0 || narrowed;
  const showNoResults = filtered.length === 0 && searching;
  const showEmpty = filtered.length === 0 && !searching;

  const docked = searchPosition === "dock";

  const searchField = search ? (
    <input
      ref={inputRef}
      type="search"
      value={query}
      placeholder={search.placeholder}
      aria-label={search.placeholder ?? "Search"}
      onChange={(event) => setQuery(event.target.value)}
      onKeyDown={(event) => {
        if (event.key === "ArrowDown" || event.key === "Enter") {
          event.preventDefault();
          scrollRef.current?.focus();
          if (event.key === "ArrowDown") moveActive(clampedActive + 1);
          else if (clampedActive >= 0) onActivate?.(filtered[clampedActive], clampedActive);
        } else if (event.key === "Escape" && query.length > 0) {
          event.preventDefault();
          setQuery("");
        }
      }}
      className={cn(
        "min-w-0 flex-1 text-body text-text-primary placeholder:text-text-tertiary",
        docked ? "h-full bg-transparent" : "hairline h-8 rounded-input bg-sunken px-3",
      )}
    />
  ) : null;

  return (
    <div className={cn("relative flex h-full min-h-0 flex-col", className)}>
      {!docked && (search || toolbar) ? (
        <div className="flex items-center gap-2 px-4 py-3">
          {searchField}
          {toolbar}
        </div>
      ) : null}

      <div
        ref={scrollRef}
        role="listbox"
        aria-label={label}
        aria-activedescendant={clampedActive >= 0 ? `${listId}-row-${clampedActive}` : undefined}
        tabIndex={0}
        onKeyDown={onKeyDown}
        data-scroll-area=""
        data-fade-top={edges.hasAbove}
        data-fade-bottom={edges.hasBelow}
        style={
          headerInset
            ? ({
                // Content must dissolve completely before it reaches the title.
                "--fade-size-top": "calc(var(--page-header-height) + var(--scroll-fade-size))",
                "--fade-hold-size-top": "var(--page-header-height)",
              } as CSSProperties)
            : undefined
        }
        className="scroll-fade min-h-0 flex-1 overflow-auto outline-offset-[calc(var(--focus-ring-offset)*-1)]"
      >
        {insetPx > 0 ? <div aria-hidden="true" style={{ height: insetPx }} /> : null}
        {showEmpty ? empty : null}
        {showNoResults ? (noResults ?? empty) : null}
        {!showEmpty && !showNoResults ? (
          <>
            {lead ? <div style={{ height: leadHeight }}>{lead}</div> : null}
            <div style={{ height: window_.totalHeight }} className="relative">
              <div style={{ transform: `translateY(${window_.offsetY}px)` }}>{rows}</div>
            </div>
          </>
        ) : null}
        {footer}
        {docked ? <div aria-hidden="true" className="h-[var(--dock-clearance)]" /> : null}
      </div>

      {docked && (search || toolbar) ? (
        <div className="pointer-events-none absolute inset-x-0 bottom-0 flex justify-center px-[var(--page-padding-x)] pb-[var(--dock-offset)]">
          <div className="material-elevated pointer-events-auto flex h-[var(--dock-height)] w-full max-w-[var(--dock-max-width)] items-center gap-3 rounded-pill pr-2 pl-5">
            <Search aria-hidden="true" className="size-4 shrink-0 text-text-tertiary" />
            {searchField}
            {toolbar}
          </div>
        </div>
      ) : null}
    </div>
  );
}
