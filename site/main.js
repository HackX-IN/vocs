/**
 * SOURCE OF TRUTH KEYWORDS: site-script, latest-release, download-button, github-releases-api,
 *   release-fallback, typing-demo, nav-stuck
 * WHAT:  The launch page's only script: points every download button at the newest published
 *        release's DMG, runs the typing demo in the product picture, and marks the nav once the
 *        page scrolls.
 * WHY:   The download link is looked up in the browser, when the page loads, instead of written
 *        into the HTML at deploy time. That is what makes "latest version, every time" true with no
 *        commit and no workflow run per release: publishing a GitHub Release is the whole act.
 *        The DMG's filename carries the version, so the stable /releases/latest/download/<name>
 *        URL cannot be hard-coded; the API answers with the real one.
 *
 *        EVERY failure lands on the releases page, which is already the href in the HTML. No JS, a
 *        blocked API, GitHub's 60-requests-an-hour anonymous limit, no release published yet, a
 *        release without a DMG: the button still goes somewhere true. The answer is cached for ten
 *        minutes in sessionStorage so a reload does not spend the limit, and storage is wrapped
 *        because it throws in private windows.
 *
 *        The typing demo is decoration. Reduced motion, or no JS, leaves the first sentence fully
 *        written in the HTML.
 * WHERE: Loaded (deferred) by site/index.html.
 */

const REPO = "HackX-IN/vocs";
const API = `https://api.github.com/repos/${REPO}/releases/latest`;
const CACHE_KEY = "vocs.latest-release";
const CACHE_MS = 10 * 60 * 1000;

function readCache() {
  try {
    const raw = sessionStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const { at, release } = JSON.parse(raw);
    return Date.now() - at < CACHE_MS ? release : null;
  } catch {
    return null;
  }
}

function writeCache(release) {
  try {
    sessionStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), release }));
  } catch {
    // Private window: the page works the same, it just asks again next time.
  }
}

/** The bits of a release the page uses, or null when it has no installable DMG. */
function summarise(release) {
  const dmg = (release.assets ?? []).find((asset) => asset.name.toLowerCase().endsWith(".dmg"));
  if (!dmg) return null;
  return {
    version: String(release.tag_name ?? "").replace(/^v/, ""),
    url: dmg.browser_download_url,
    notes: release.html_url,
  };
}

async function latestRelease() {
  const cached = readCache();
  if (cached) return cached;

  const response = await fetch(API, { headers: { Accept: "application/vnd.github+json" } });
  if (!response.ok) return null;
  const summary = summarise(await response.json());
  if (summary) writeCache(summary);
  return summary;
}

function showRelease(release) {
  document.querySelectorAll("[data-download]").forEach((link) => {
    link.href = release.url;
  });
  document.querySelectorAll("[data-version]").forEach((node) => {
    node.textContent = `v${release.version}`;
  });
  document.querySelectorAll("[data-version-wrap]").forEach((node) => {
    node.hidden = false;
  });
  document.querySelectorAll("[data-notes]").forEach((link) => {
    link.href = release.notes;
  });
}

latestRelease()
  .then((release) => {
    if (release) showRelease(release);
  })
  .catch(() => {
    // Offline or blocked: the HTML's own links to the releases page stand.
  });

// ── Nav: a hairline appears once the page has scrolled ──────────────────
const nav = document.querySelector(".nav");
const syncNav = () => nav?.setAttribute("data-stuck", String(window.scrollY > 8));
window.addEventListener("scroll", syncNav, { passive: true });
syncNav();

// ── Typing demo ─────────────────────────────────────────────────────────
const target = document.querySelector("[data-typing]");
const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

if (target && !reduced) {
  const lines = [
    "Push the launch to Thursday. Design needs one more pass on the onboarding screens.",
    "Can you send me the Q3 numbers before the call? I'll review them on the train.",
    "Thanks for the quick turnaround. I've merged it and the build is green.",
  ];
  const TYPE_MS = 28;
  const HOLD_MS = 2600;
  let index = 0;

  const typeLine = () => {
    const line = lines[index];
    let shown = 0;
    target.textContent = "";
    const tick = () => {
      shown += 1;
      target.textContent = line.slice(0, shown);
      if (shown < line.length) {
        setTimeout(tick, TYPE_MS);
      } else {
        index = (index + 1) % lines.length;
        setTimeout(typeLine, HOLD_MS);
      }
    };
    tick();
  };

  // Let the first sentence be read as written for a moment, then start the loop.
  setTimeout(typeLine, HOLD_MS);
}
