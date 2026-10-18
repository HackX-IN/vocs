/**
 * SOURCE OF TRUTH KEYWORDS: findMatches, offsetTopOf, textarea_find, find_in_editor, mirror_measure
 * WHAT:  Two pure helpers for finding text in a textarea: every place a query
 *        occurs, and how far down the textarea a given character sits.
 * WHY:   A textarea that grows to its content scrolls with the PAGE, not itself,
 *        so selecting a match does not bring it into view — the browser only
 *        scrolls a textarea's own overflow. The vertical position is therefore
 *        measured by laying the text before the match out in an off-screen copy
 *        with the textarea's own font, width and wrapping, and reading its
 *        height. That is the only way to know where a wrapped line really ends
 *        without counting characters per line, which is wrong for any
 *        proportional font.
 * WHERE: FindBar (matches) and DictationEditor (scrolling the page to the match).
 */

/** Start index of every case-insensitive occurrence of `query` in `text`. */
export function findMatches(text: string, query: string): number[] {
  const needle = query.trim().toLowerCase();
  if (needle.length === 0) return [];
  const haystack = text.toLowerCase();
  const found: number[] = [];
  for (let from = haystack.indexOf(needle); from !== -1; from = haystack.indexOf(needle, from + needle.length)) {
    found.push(from);
  }
  return found;
}

/** Pixels from the top of `textarea` to the line holding character `index`. */
export function offsetTopOf(textarea: HTMLTextAreaElement, index: number): number {
  const style = window.getComputedStyle(textarea);
  const mirror = document.createElement("div");
  for (const property of ["font", "letterSpacing", "lineHeight", "padding", "whiteSpace", "wordBreak", "overflowWrap"] as const) {
    mirror.style[property] = style[property];
  }
  mirror.style.position = "absolute";
  mirror.style.visibility = "hidden";
  mirror.style.width = `${textarea.clientWidth}px`;
  mirror.style.whiteSpace = "pre-wrap";
  mirror.textContent = textarea.value.slice(0, index);
  const marker = document.createElement("span");
  marker.textContent = "​";
  mirror.appendChild(marker);
  document.body.appendChild(mirror);
  const top = marker.offsetTop;
  mirror.remove();
  return top;
}
