/**
 * SOURCE OF TRUTH KEYWORDS: caption-entry, CaptionEntry, createRoot, caption.html
 * WHAT:  The caption window's entry point. Mounts <Caption /> and nothing else.
 * WHY:   Its own Vite entry for the same reason the pill has one: it must paint
 *        quickly while the app is busy listening. Not wrapped in StrictMode —
 *        the double-invoked effects would subscribe to the caption event twice in
 *        development.
 * WHERE: Loaded by caption.html, which the Rust side opens as a floating window.
 */

import { createRoot } from "react-dom/client";
import { Caption } from "@/app/caption/Caption";
import "@/styles/global.css";

const container = document.getElementById("root");
if (!container) throw new Error("caption.html is missing its #root element.");

createRoot(container).render(<Caption />);
