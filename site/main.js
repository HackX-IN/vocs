/**
 * SOURCE OF TRUTH KEYWORDS: site-script, main-entry, nav-stuck, typing-demo, reduced-motion
 * WHAT:  The page's entry script: starts the download wiring, marks the nav once the page scrolls,
 *        and runs the typing demo in the product picture.
 * WHY:   Kept to the small, decorative things on purpose. Everything that talks to GitHub lives in
 *        release.js and downloads.js so there is one place to look when the download misbehaves.
 *        The typing demo is decoration: with reduced motion, or no JavaScript, the first sentence
 *        stays fully written in the HTML.
 * WHERE: Loaded as a module (deferred) by site/index.html.
 */

import { initDownloads } from "./downloads.js";

initDownloads();

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
