/**
 * SOURCE OF TRUTH KEYWORDS: initReleaseTimeline, release-timeline, release-pagination, release-search,
 *   releaseRow, year-groups, show-older-releases
 * WHAT:  Renders the Releases section: the newest release as one featured card, then every earlier
 *        release as a collapsed timeline row grouped by year, with "show older" paging and a
 *        find-a-version box.
 * WHY:   Built for a hundred releases, not for three. Every earlier release is ONE line (version,
 *        a one-line gist of what changed, a short date) that opens in place, so a long history is
 *        scannable instead of a wall of cards. Only the first few rows are in the DOM; older ones
 *        arrive ten at a time, and the next page is fetched from GitHub only when the visitor asks
 *        for it, so the first screen costs one small request however long the history is.
 *
 *        Search needs the whole history, so the first keystroke loads the remaining pages (a handful
 *        of requests at the very most) and then filters locally; until then the box would
 *        quietly search only what happened to be loaded, which is a search that lies.
 *
 *        Nothing is assigned as HTML: rows are built from createElement and textContent (dom.js).
 * WHERE: Called once by downloads.js with the first page of releases. Renders into
 *        #release-feature, #release-list and #release-more inside #releases in index.html.
 */

import { loadReleases, latestRelease } from "./release.js";
import { renderNotes, gistOf } from "./notes.js";
import { element, formatDate, formatShortDate, formatSize, releaseYear } from "./dom.js";

/** Rows shown before the visitor asks for more, and how many each click adds. */
const INITIAL_ROWS = 6;
const STEP = 10;
const SEARCH_DEBOUNCE_MS = 150;

const byId = (id) => document.getElementById(id);

function downloadLink(release, className) {
  const link = element("a", className, `Download${release.size ? ` · ${formatSize(release.size)}` : ""}`);
  link.href = release.dmg;
  link.setAttribute("data-release-download", "");
  return link;
}

function badges(release, isLatest) {
  const nodes = [];
  if (isLatest) nodes.push(element("span", "badge", "Latest"));
  if (release.prerelease) nodes.push(element("span", "badge badge-muted", "Pre-release"));
  return nodes;
}

function featureCard(release) {
  const card = element("article", "release-feature-card");
  const head = element("div", "release-head");
  head.append(element("h3", "", `Vocs ${release.version}`), ...badges(release, true));
  const date = formatDate(release.date);
  if (date) head.append(element("time", "release-date", date));
  head.append(downloadLink(release, "btn btn-sm"));

  const notes = element("div", "notes");
  notes.append(renderNotes(release.notes));
  card.append(head, notes);
  return card;
}

function releaseRow(release) {
  const item = element("li", "rel");
  const details = element("details", "rel-row");

  const summary = element("summary", "rel-summary");
  summary.append(element("span", "rel-version", `v${release.version}`), ...badges(release, false));
  summary.append(element("span", "rel-gist", gistOf(release.notes)));
  const date = formatShortDate(release.date);
  if (date) summary.append(element("time", "rel-date", date));

  const body = element("div", "rel-body");
  const notes = element("div", "notes");
  notes.append(renderNotes(release.notes));
  body.append(notes, downloadLink(release, "btn btn-sm"));

  details.append(summary, body);
  item.append(details);
  return item;
}

export function initReleaseTimeline(first) {
  const feature = byId("release-feature");
  const list = byId("release-list");
  const more = byId("release-more");
  const moreButton = byId("release-more-button");
  const count = byId("release-count");
  const search = byId("release-search");
  const searchWrap = byId("release-search-wrap");
  const noMatch = byId("release-no-match");
  const empty = byId("releases-empty");
  if (!list || !feature) return;

  const state = {
    all: [...first.releases],
    hasMore: first.hasMore,
    nextPage: 2,
    visible: INITIAL_ROWS,
    query: "",
    loading: false,
  };

  const featured = latestRelease(state.all) ?? state.all[0] ?? null;
  if (empty) empty.hidden = featured !== null;
  if (featured) feature.replaceChildren(featureCard(featured));

  const earlier = () => {
    const rest = state.all.filter((release) => release !== featured);
    // "v1.2.3" and "1.2.3" are the same search: the tag has a v, the version shown to people reads either way.
    const query = state.query.trim().toLowerCase().replace(/^v(?=\d)/, "");
    if (!query) return rest;
    return rest.filter((release) => `${release.version} ${release.notes}`.toLowerCase().includes(query));
  };

  async function fetchNextPage() {
    if (state.loading || !state.hasMore) return;
    state.loading = true;
    moreButton.disabled = true;
    const page = await loadReleases(state.nextPage);
    state.all.push(...page.releases);
    state.hasMore = page.hasMore;
    state.nextPage += 1;
    state.loading = false;
    moreButton.disabled = false;
  }

  function render() {
    const rows = earlier();
    const shown = rows.slice(0, state.visible);
    const fragment = document.createDocumentFragment();
    let year = null;
    for (const release of shown) {
      const releaseOf = releaseYear(release.date);
      if (releaseOf !== year) {
        year = releaseOf;
        if (year !== null) fragment.append(element("li", "rel-year", String(year)));
      }
      fragment.append(releaseRow(release));
    }
    list.replaceChildren(fragment);

    const remaining = rows.length - shown.length;
    more.hidden = remaining <= 0 && !(state.hasMore && !state.query);
    moreButton.textContent = `Show ${Math.min(STEP, Math.max(remaining, STEP))} older releases`;
    if (noMatch) noMatch.hidden = !(state.query && rows.length === 0);

    const total = rows.length + (state.hasMore && !state.query ? "+" : "");
    count.textContent = shown.length === 0 ? "" : `Showing ${shown.length} of ${total} earlier releases`;
    if (searchWrap) searchWrap.hidden = state.all.length <= INITIAL_ROWS && !state.hasMore;
  }

  moreButton.addEventListener("click", async () => {
    state.visible += STEP;
    if (state.visible > earlier().length) await fetchNextPage();
    render();
  });

  let debounce = 0;
  search?.addEventListener("input", () => {
    clearTimeout(debounce);
    debounce = setTimeout(async () => {
      state.query = search.value;
      state.visible = INITIAL_ROWS;
      // Search covers the whole history, so fetch what is still missing first.
      while (state.query.trim() && state.hasMore) await fetchNextPage();
      render();
    }, SEARCH_DEBOUNCE_MS);
  });

  render();
}
