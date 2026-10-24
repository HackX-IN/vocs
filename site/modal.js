/**
 * SOURCE OF TRUTH KEYWORDS: initDownloadModal, download-dialog, first-launch-steps, copy-command,
 *   native-dialog, showModal
 * WHAT:  Opens a dialog when a Download button is clicked that shows, step by step, how to open
 *        a macOS app that Apple has not notarised, with the two Terminal commands copyable.
 * WHY:   The steps are CLONED from the Install section rather than written a second time, so the
 *        page has one copy of the instructions and the dialog can never say something the page
 *        doesn't. It is a native <dialog>: showModal gives focus trapping, Escape to close and an
 *        inert page behind it for free, which a hand-built overlay would have to get right.
 *
 *        It replaces a toast that the visitor had fourteen seconds to read, and that linked down
 *        the page to the same steps. The first launch is where people get stuck, so the steps
 *        belong in front of them at the moment the download begins.
 *
 *        It never cancels the click: the link is a plain link to a file and keeps working with
 *        JavaScript off or failing. The dialog is only the explanation.
 * WHERE: Called once by downloads.js. Reads #started in index.html and the steps in #install.
 */

import { element } from "./dom.js";

function copyButton(code) {
  const button = element("button", "cmd-copy", "Copy");
  button.type = "button";
  button.setAttribute("aria-label", `Copy command: ${code.textContent}`);
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(code.textContent ?? "");
      button.textContent = "Copied";
    } catch {
      // Clipboard blocked: select the text so ⌘C still works.
      const range = document.createRange();
      range.selectNodeContents(code);
      getSelection()?.removeAllRanges();
      getSelection()?.addRange(range);
      button.textContent = "Press ⌘C";
    }
    setTimeout(() => {
      button.textContent = "Copy";
    }, 1800);
  });
  return button;
}

/** Adds a Copy button to every command block under `root` that has none. */
export function enhanceCommands(root) {
  root.querySelectorAll(".cmd").forEach((block) => {
    const code = block.querySelector("code");
    if (code && !block.querySelector(".cmd-copy")) block.append(copyButton(code));
  });
}

/** Opens a native dialog, falling back to the plain `open` attribute where showModal is missing. */
function openDialog(dialog) {
  if (dialog.open) return;
  if (typeof dialog.showModal === "function") dialog.showModal();
  else dialog.setAttribute("open", "");
}

export function initDownloadModal() {
  const dialog = document.getElementById("started");
  const source = document.getElementById("install-guide");
  if (!dialog || !source) return () => {};

  const slot = dialog.querySelector("[data-modal-steps]");
  let filled = false;

  const fill = () => {
    if (filled) return;
    filled = true;
    slot.replaceChildren(source.cloneNode(true));
    // The clone must not repeat the id, or the page would have two #install-guide.
    slot.firstElementChild?.removeAttribute("id");
    enhanceCommands(slot);
  };

  const close = () => (typeof dialog.close === "function" ? dialog.close() : dialog.removeAttribute("open"));
  dialog.querySelectorAll("[data-modal-close]").forEach((node) => node.addEventListener("click", close));
  // A click on the backdrop lands on the dialog element itself, never on its card.
  dialog.addEventListener("click", (event) => {
    if (event.target === dialog) close();
  });

  enhanceCommands(source);

  /** Shows the dialog for a release (or, with no release known, without a version). */
  return (release) => {
    fill();
    dialog.querySelectorAll("[data-modal-version]").forEach((node) => {
      node.textContent = release ? `v${release.version}` : "";
    });
    const retry = dialog.querySelector("[data-modal-retry]");
    if (retry && release) retry.href = release.dmg;
    openDialog(dialog);
  };
}
