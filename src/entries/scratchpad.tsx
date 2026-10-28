/**
 * SOURCE OF TRUTH KEYWORDS: scratchpad-entry, ScratchpadEntry, createRoot, scratchpad.html
 * WHAT:  The scratchpad window's entry point. Mounts <Scratchpad /> and nothing else.
 * WHY:   Its own Vite entry so the window carries only what it draws. Not wrapped
 *        in StrictMode — the double-invoked effects would register the window's
 *        focus and key listeners twice in development.
 * WHERE: Loaded by scratchpad.html, which the Rust side opens as a floating window.
 */

import { createRoot } from "react-dom/client";
import { Scratchpad } from "@/app/scratchpad/Scratchpad";
import "@/styles/global.css";

const container = document.getElementById("root");
if (!container) throw new Error("scratchpad.html is missing its #root element.");

createRoot(container).render(<Scratchpad />);
