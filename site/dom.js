/**
 * SOURCE OF TRUTH KEYWORDS: element, formatDate, formatShortDate, formatSize, site-dom-helpers
 * WHAT:  The three tiny helpers every script on the page needs: build an element, format a date,
 *        format a byte count.
 * WHY:   Shared so the version label, the release timeline and the download dialog all print a
 *        date and a size the same way, and so none of them reaches for innerHTML: every node is
 *        built with createElement and textContent, because release notes are data from outside the
 *        page (see notes.js).
 * WHERE: Imported by downloads.js, releases-view.js and modal.js.
 */

export function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text) node.textContent = text;
  return node;
}

function parse(iso) {
  if (!iso) return null;
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? null : date;
}

export function formatDate(iso) {
  return parse(iso)?.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" }) ?? "";
}

/** "Oct 18": the year is already in the heading above a timeline row. */
export function formatShortDate(iso) {
  return parse(iso)?.toLocaleDateString(undefined, { month: "short", day: "numeric" }) ?? "";
}

export function releaseYear(iso) {
  return parse(iso)?.getFullYear() ?? null;
}

export function formatSize(bytes) {
  return bytes ? `${Math.round(bytes / (1024 * 1024))} MB` : "";
}
