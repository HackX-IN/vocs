/**
 * SOURCE OF TRUTH KEYWORDS: loadReleases, latestRelease, release-cache, github-releases-api,
 *   release-summary, download-url, dmg-asset
 * WHAT:  Asks GitHub for the project's published releases once and returns them as plain objects:
 *        version, date, notes, the DMG's download URL, and the release page.
 * WHY:   The download button and the Releases section are both driven by this one answer, so the
 *        page makes ONE request and the two can never disagree about what "latest" is. Looked up in
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
 * WHERE: Imported by main.js and downloads.js. Nothing else talks to the API.
 */

const REPO = "HackX-IN/vocs";
const API = `https://api.github.com/repos/${REPO}/releases?per_page=10`;
const CACHE_KEY = "vocs.releases.v1";
const CACHE_MS = 10 * 60 * 1000;

/** The releases page, which is where every failure lands. */
export const RELEASES_PAGE = `https://github.com/${REPO}/releases`;

function readCache() {
  try {
    const raw = sessionStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const { at, releases } = JSON.parse(raw);
    return Date.now() - at < CACHE_MS && Array.isArray(releases) ? releases : null;
  } catch {
    return null;
  }
}

function writeCache(releases) {
  try {
    sessionStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), releases }));
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

/** Newest first. Always resolves; an empty array means "nothing to offer yet". */
export async function loadReleases() {
  const cached = readCache();
  if (cached) return cached;

  try {
    const response = await fetch(API, { headers: { Accept: "application/vnd.github+json" } });
    if (!response.ok) return [];
    const releases = (await response.json()).map(summarise).filter(Boolean);
    writeCache(releases);
    return releases;
  } catch {
    return [];
  }
}

/** The release the download buttons offer: the newest one that is not a pre-release. */
export function latestRelease(releases) {
  return releases.find((release) => !release.prerelease) ?? null;
}
