/**
 * SOURCE OF TRUTH KEYWORDS: initDownloads, download-button, download-toast, releases-section,
 *   showLatest, renderReleases, data-download, direct-download
 * WHAT:  Wires every Download button to the newest release's DMG so a click downloads the app
 *        from this page, confirms it with a "download started" note, and fills the Releases
 *        section with each release's date and notes.
 * WHY:   The buttons start as plain links to the GitHub releases page so the page works with no
 *        JavaScript; once the release is known they are re-pointed at the DMG itself. GitHub serves
 *        that file as an attachment, so the browser downloads it and the visitor never leaves the
 *        page, which is what lets the page stay put and show the next step (the first-launch
 *        approval) instead of dropping them on a repository.
 *
 *        A click that arrives BEFORE the lookup resolves is held and decided when it does, not
 *        allowed to fall through to the releases page: the visitor asked for the app. If there
 *        turns out to be no published release, that fallback is the honest answer.
 *
 *        Releases are rendered with DOM calls, never innerHTML, because the notes are data from a
 *        release body (see notes.js).
 * WHERE: Called once by main.js. Reads release.js; renders into #release-list inside #releases,
 *        and the #started toast in index.html.
 */

import { loadReleases, latestRelease, RELEASES_PAGE } from "./release.js";
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
  document.querySelectorAll("[data-download]").forEach((link) => {
    link.href = release.dmg;
  });
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
  toast.querySelector("[data-toast-version]").textContent = `v${release.version}`;
  toast.hidden = false;
  clearTimeout(announceStarted.timer);
  announceStarted.timer = setTimeout(() => {
    toast.hidden = true;
  }, TOAST_MS);
}

function startDownload(release) {
  const link = element("a");
  link.href = release.dmg;
  link.download = "";
  document.body.append(link);
  link.click();
  link.remove();
  announceStarted(release);
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

  document.addEventListener("click", async (event) => {
    const link = event.target.closest("a[data-download]");
    if (!link) return;

    // Known: let the link do its job (it already points at the DMG) and say what happens next.
    if (latest) {
      announceStarted(latest);
      return;
    }

    // Not known yet: decide when it is, rather than sending the visitor to a repository.
    event.preventDefault();
    const release = latestRelease(await releases);
    if (release) startDownload(release);
    else window.location.href = RELEASES_PAGE;
  });
}
