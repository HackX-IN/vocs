/**
 * SOURCE OF TRUTH KEYWORDS: loadReleases, latestRelease, release-cache, github-releases-api,
 *   release-summary, download-url, dmg-asset
 * WHAT:  Asks GitHub for the project's published releases once and returns them as plain objects:
 *        version, date, notes, the DMG's download URL, and the release page.
 * WHY:   The version label beside the download buttons and the Releases section are both driven by
 *        this one answer, so the page makes ONE request and the two can never disagree about what
 *        "latest" is. It does NOT drive the download itself: the buttons are plain links to a
 *        permanent file URL (see downloads.js), so a failed lookup here costs a label, never a
 *        download. Looked up in
 *        the visitor's browser, not written into the HTML at deploy time: publishing a GitHub
 *        Release is then the whole act of shipping, with no commit and no workflow run for the site.
 *        The DMG's filename carries the version, so a stable /releases/latest/download/<name> URL
 *        cannot be hard-coded; the API supplies the real one.
 *
 *        Drafts never appear (the API only returns published releases to anonymous callers), which is
 *        what keeps an unreviewed build from being offered. Every failure resolves to an EMPTY list
 *        rather than throwing, so callers have one "nothing to show" case: no network, GitHub's
 *        60-requests-an-hour anonymous limit, no release yet. The answer is cached ten minutes in
 *        sessionStorage so a reload does not spend the limit; storage is wrapped because it throws
 *        in private windows.
 *
 *        Releases are paged, because a project with a hundred of them must not fetch a hundred
 *        release bodies to paint the first screen: page 1 is thirty, and the Releases section asks
 *        for the next page only when the visitor asks for older ones (or searches).
 * WHERE: Imported by downloads.js and releases-view.js. Nothing else talks to the API.
 */

const REPO = "HackX-IN/vocs";
/** Releases per request. GitHub's maximum is 100; 30 keeps the first paint light and the rest one click away. */
export const PAGE_SIZE = 30;
const api = (page) => `https://api.github.com/repos/${REPO}/releases?per_page=${PAGE_SIZE}&page=${page}`;
const cacheKey = (page) => `vocs.releases.v3.p${page}`;
const CACHE_MS = 10 * 60 * 1000;

/** The releases page; the fallback link for a release that reports no page of its own. */
export const RELEASES_PAGE = `https://github.com/${REPO}/releases`;

function readCache(page) {
  try {
    const raw = sessionStorage.getItem(cacheKey(page));
    if (!raw) return null;
    const { at, releases, hasMore } = JSON.parse(raw);
    return Date.now() - at < CACHE_MS && Array.isArray(releases) ? { releases, hasMore: Boolean(hasMore) } : null;
  } catch {
    return null;
  }
}

function writeCache(page, answer) {
  try {
    sessionStorage.setItem(cacheKey(page), JSON.stringify({ at: Date.now(), ...answer }));
  } catch {
    // Private window: the page works the same, it just asks again next time.
  }
}

/** The fields the page uses, or null for a release with no installable DMG. */
function summarise(release) {
  const dmg = (release.assets ?? []).find((asset) => asset.name.toLowerCase().endsWith(".dmg"));
  if (!dmg || release.draft) return null;
  return {
    version: String(release.tag_name ?? "").replace(/^v/, ""),
    date: release.published_at ?? release.created_at ?? null,
    prerelease: Boolean(release.prerelease),
    notes: release.body ?? "",
    dmg: dmg.browser_download_url,
    size: dmg.size ?? null,
    page: release.html_url ?? RELEASES_PAGE,
  };
}

/**
 * One page of releases, newest first: `{ releases, hasMore }`. Always resolves; an empty list
 * means "nothing to offer yet". `hasMore` is read from the raw page size, not the summarised one,
 * because releases without a DMG are dropped and a page of 30 can summarise to fewer.
 */
export async function loadReleases(page = 1) {
  const cached = readCache(page);
  if (cached) return cached;

  try {
    const response = await fetch(api(page), { headers: { Accept: "application/vnd.github+json" } });
    if (!response.ok) return { releases: [], hasMore: false };
    const raw = await response.json();
    const answer = { releases: raw.map(summarise).filter(Boolean), hasMore: raw.length === PAGE_SIZE };
    // An empty answer is NOT cached: right after a release is published the page would otherwise
    // keep showing "no release yet" for the rest of the cache window.
    if (answer.releases.length > 0) writeCache(page, answer);
    return answer;
  } catch {
    return { releases: [], hasMore: false };
  }
}

/** The release the download buttons offer: the newest one that is not a pre-release. */
export function latestRelease(releases) {
  return releases.find((release) => !release.prerelease) ?? null;
}
