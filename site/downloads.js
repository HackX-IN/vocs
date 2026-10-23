/**
 * SOURCE OF TRUTH KEYWORDS: initDownloads, download-button, download-toast, releases-section,
 *   showLatest, renderReleases, data-download, direct-download
 * WHAT:  Shows the "download started" note when a Download button is clicked, fills in the
 *        version and size beside the buttons, and renders the Releases section with each
 *        release's date and notes.
 * WHY:   The buttons are plain links to ONE permanent URL, releases/latest/download/Vocs-macOS.dmg,
 *        which GitHub redirects to the newest published release (release.yml uploads that copy of
 *        the DMG with every release). They used to be re-pointed here from an API answer, and any
 *        failed, rate-limited or stale lookup sent the visitor to the releases page instead of
 *        downloading. A link to a file has no such failure, and GitHub serves it as an attachment,
 *        so the browser downloads it and the visitor stays on the page. Everything this file does
 *        is decoration around that: if the API is down the download still works and only the
 *        version label and the Releases section are missing.
 *
 *        Releases are rendered with DOM calls, never innerHTML, because the notes are data from a
 *        release body (see notes.js).
 * WHERE: Called once by main.js. Reads release.js; renders into #release-list inside #releases,
 *        and the #started toast in index.html.
 */

import { loadReleases, latestRelease } from "./release.js";
import { renderNotes } from "./notes.js";

const TOAST_MS = 14000;

function formatDate(iso) {
  if (!iso) return "";
  const date = new Date(iso);
  return Number.isNaN(date.getTime())
    ? ""
    : date.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
}

function formatSize(bytes) {
  return bytes ? `${Math.round(bytes / (1024 * 1024))} MB` : "";
}

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text) node.textContent = text;
  return node;
}

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

function releaseHeader(release, isLatest) {
  const header = element("div", "release-head");
  const title = element("h3", "", `Vocs ${release.version}`);
  header.append(title);
  if (isLatest) header.append(element("span", "badge", "Latest"));
  if (release.prerelease) header.append(element("span", "badge badge-muted", "Pre-release"));
  const date = formatDate(release.date);
  if (date) header.append(element("time", "release-date", date));
  return header;
}

function renderReleases(releases, latest) {
  const list = document.getElementById("release-list");
  const empty = document.getElementById("releases-empty");
  if (!list) return;

  list.replaceChildren();
  if (empty) empty.hidden = releases.length > 0;

  releases.forEach((release, index) => {
    const isLatest = release === latest;
    const notes = element("div", "notes");
    notes.append(renderNotes(release.notes));

    const download = element("a", "btn btn-sm", `Download${release.size ? ` · ${formatSize(release.size)}` : ""}`);
    download.href = release.dmg;
    download.setAttribute("data-release-download", "");

    const open = index === 0;
    const card = element(open ? "article" : "details", "release");
    if (open) {
      const head = releaseHeader(release, isLatest);
      head.append(download);
      card.append(head, notes);
    } else {
      const summary = element("summary", "");
      summary.append(releaseHeader(release, false));
      const body = element("div", "release-body");
      body.append(notes, download);
      card.append(summary, body);
    }
    list.append(card);
  });
}

/** Tells the visitor the download began and what to do with it. */
function announceStarted(release) {
  const toast = document.getElementById("started");
  if (!toast) return;
  toast.querySelector("[data-toast-version]").textContent = release ? `v${release.version}` : "";
  toast.hidden = false;
  clearTimeout(announceStarted.timer);
  announceStarted.timer = setTimeout(() => {
    toast.hidden = true;
  }, TOAST_MS);
}

export function initDownloads() {
  const releases = loadReleases();
  let latest = null;

  releases.then((all) => {
    latest = latestRelease(all);
    if (latest) showLatest(latest);
    renderReleases(all, latest);
  });

  document.getElementById("started-close")?.addEventListener("click", () => {
    document.getElementById("started").hidden = true;
  });

  // The link itself downloads the file (see the module WHY); all this adds is the note saying so.
  // It never cancels the click, so a failed lookup cannot get in the way of a download.
  document.addEventListener("click", (event) => {
    if (event.target.closest("a[data-download]")) announceStarted(latest);
  });
}
