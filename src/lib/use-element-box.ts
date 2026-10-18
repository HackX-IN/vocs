/**
 * SOURCE OF TRUTH KEYWORDS: useElementBox, ElementBox, ResizeObserver, measured-chart
 * WHAT:  Measures an element's content box in CSS pixels, or null until it has
 *        been measured.
 * WHY:   Null rather than a guessed default. A chart drawn at a made-up width
 *        and corrected one frame later is a visible jump on every mount, and the
 *        charts in this app are the one thing on screen that must not animate
 *        into place (docs/04 §11). A ResizeObserver rather than a window
 *        listener, because a card's width changes when a sibling appears, not
 *        only when the window does.
 * WHERE: The dot charts and the activity chart.
 */

import { useEffect, useRef, useState, type RefObject } from "react";

export interface ElementBox {
  width: number;
  height: number;
}

export function useElementBox(): [RefObject<HTMLDivElement | null>, ElementBox | null] {
  const ref = useRef<HTMLDivElement | null>(null);
  const [box, setBox] = useState<ElementBox | null>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const observer = new ResizeObserver((entries) => {
      const rect = entries[0]?.contentRect;
      setBox(rect && rect.width > 0 && rect.height > 0 ? { width: rect.width, height: rect.height } : null);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return [ref, box];
}
