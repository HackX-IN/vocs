/**
 * SOURCE OF TRUTH KEYWORDS: renderNotes, release-notes, markdown-to-dom, changelog-render,
 *   safe-markdown, textContent
 * WHAT:  Turns a release's markdown notes into DOM nodes: headings, bullet lists, paragraphs, and
 *        inline bold, code and https links.
 * WHY:   Built from createElement and textContent, never innerHTML. The notes come from a GitHub
 *        release body, which is data from outside the page; assigning it as HTML would let anyone
 *        who can edit a release run script on this site. Building nodes makes that impossible by
 *        construction, with no sanitiser to get wrong. Only the markdown the CHANGELOG actually
 *        uses is supported (### headings, "- " bullets, **bold**, `code`, [text](https://...)),
 *        because a full renderer would be a dependency for a dozen lines of notes.
 * WHERE: Imported by releases-view.js, which fills the Releases section.
 */

const INLINE = /(\*\*[^*]+\*\*|`[^`]+`|\[[^\]]+\]\(https:\/\/[^)\s]+\))/g;

function appendInline(parent, text) {
  for (const part of text.split(INLINE)) {
    if (!part) continue;
    if (part.startsWith("**")) {
      const node = document.createElement("strong");
      node.textContent = part.slice(2, -2);
      parent.append(node);
    } else if (part.startsWith("`")) {
      const node = document.createElement("code");
      node.textContent = part.slice(1, -1);
      parent.append(node);
    } else if (part.startsWith("[")) {
      const [, label, url] = part.match(/^\[([^\]]+)\]\((.+)\)$/);
      const node = document.createElement("a");
      node.textContent = label;
      node.href = url;
      node.rel = "noopener";
      parent.append(node);
    } else {
      parent.append(part);
    }
  }
}

/** Returns a DocumentFragment for the markdown; an empty one for empty notes. */
export function renderNotes(markdown) {
  const fragment = document.createDocumentFragment();
  let list = null;

  for (const raw of String(markdown).split("\n")) {
    const line = raw.trim();
    if (!line) {
      list = null;
      continue;
    }

    const heading = line.match(/^#{1,6}\s+(.*)$/);
    if (heading) {
      list = null;
      const node = document.createElement("h4");
      node.textContent = heading[1];
      fragment.append(node);
      continue;
    }

    const bullet = line.match(/^[-*]\s+(.*)$/);
    if (bullet) {
      if (!list) {
        list = document.createElement("ul");
        fragment.append(list);
      }
      const item = document.createElement("li");
      appendInline(item, bullet[1]);
      list.append(item);
      continue;
    }

    list = null;
    const paragraph = document.createElement("p");
    appendInline(paragraph, line);
    fragment.append(paragraph);
  }
  return fragment;
}

/**
 * WHAT:  The first bullet or sentence of the notes as plain text, shortened — the one-line gist a
 *        collapsed release row shows beside its version.
 * WHY:   With a hundred releases the list is only scannable if each row says something. Inline
 *        markdown is stripped rather than rendered, since this goes into a single ellipsised line.
 * WHERE: Imported by releases-view.js.
 */
export function gistOf(markdown, max = 110) {
  for (const raw of String(markdown).split("\n")) {
    const line = raw.trim();
    if (!line || /^#{1,6}\s/.test(line)) continue;
    const plain = line
      .replace(/^[-*]\s+/, "")
      .replace(/\*\*([^*]+)\*\*/g, "$1")
      .replace(/`([^`]+)`/g, "$1")
      .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1");
    return plain.length > max ? `${plain.slice(0, max - 1).trimEnd()}…` : plain;
  }
  return "";
}
