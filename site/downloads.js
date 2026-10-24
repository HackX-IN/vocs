/**
 * SOURCE OF TRUTH KEYWORDS: initDownloads, download-button, releases-section,
 *   showLatest, data-download, direct-download
 * WHAT:  Opens the "how to open it" dialog when a Download button is clicked, fills in the
 *        version and size beside the buttons, and hands the releases to the timeline.
 * WHY:   The buttons are plain links to ONE permanent URL, releases/latest/download/Vocs-macOS.dmg,
 *        which GitHub redirects to the newest published release (release.yml uploads that copy of
 *        the DMG with every release). They used to be re-pointed here from an API answer, and any
 *        failed, rate-limited or stale lookup sent the visitor to the releases page instead of
 *        downloading. A link to a file has no such failure, and GitHub serves it as an attachment,
 *        so the browser downloads it and the visitor stays on the page. Everything this file does
 *        is decoration around that: if the API is down the download still works and only the
 *        version label and the Releases section are missing.
 *
 *        The release timeline lives in releases-view.js and the dialog in modal.js; this file only
 *        decides which release a click was about.
 * WHERE: Called once by main.js. Reads release.js.
 */

import { loadReleases, latestRelease } from "./release.js";
import { initReleaseTimeline } from "./releases-view.js";
import { initDownloadModal } from "./modal.js";
import { formatSize } from "./dom.js";

function showLatest(release) {
  document.querySelectorAll("[data-version]").forEach((node) => {
    node.textContent = `v${release.version}`;
  });
  document.querySelectorAll("[data-version-wrap]").forEach((node) => {
    node.hidden = false;
  });
  document.querySelectorAll("[data-size]").forEach((node) => {
    node.textContent = formatSize(release.size);
  });
  document.querySelectorAll("[data-notes]").forEach((link) => {
    link.href = release.page;
  });
}

export function initDownloads() {
  const openModal = initDownloadModal();
  let all = [];
  let latest = null;

  loadReleases(1).then((first) => {
    all = first.releases;
    latest = latestRelease(all);
    if (latest) showLatest(latest);
    initReleaseTimeline(first);
  });

  // The link itself downloads the file (see the module WHY); all this adds is the dialog saying
  // what to do next. It never cancels the click, so a failed lookup cannot get in the way.
  document.addEventListener("click", (event) => {
    const generic = event.target.closest("a[data-download]");
    const specific = event.target.closest("a[data-release-download]");
    if (!generic && !specific) return;
    const release = specific ? all.find((candidate) => candidate.dmg === specific.href) : latest;
    openModal(release ?? latest);
  });
}
