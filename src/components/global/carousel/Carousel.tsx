/**
 * SOURCE OF TRUTH KEYWORDS: Carousel, CarouselProps, carousel-track, slide-dots,
 *   roledescription-carousel
 * WHAT:  A controlled row of slides: one is showing, the track moves to bring
 *        another in, dots show where you are, and ←/→ step through.
 * WHY:   CONTROLLED, because whoever owns the carousel owns the buttons around it —
 *        "Next" turning into "Done" on the last slide is the parent's decision —
 *        and a carousel that kept its own index would have to be reached into to
 *        know when it is finished. Generic over the slide type and given a render
 *        function, so the slides' content is the caller's and nothing about what
 *        is in them is hardcoded here (CLAUDE.md §7).
 *
 *        The motion is a CSS transform on the track by `--motion-duration-medium`
 *        and `--motion-ease-standard` — content moving inside a surface, which is
 *        what docs/04 §6 says CSS is for — and it cuts under reduced motion. A slide
 *        that is not showing is `inert`, so Tab cannot land on a control that is
 *        off screen and a screen reader does not read three slides at once.
 * WHERE: The What's new dialog. Any other short, ordered sequence of equals.
 */

import { useId, type KeyboardEvent, type ReactNode } from "react";
import { cn } from "@/lib/utils";

export interface CarouselProps<T> {
  /** Names the carousel for assistive tech: "What's new in Vocs 1.1". */
  label: string;
  slides: readonly T[];
  renderSlide: (slide: T, index: number) => ReactNode;
  index: number;
  onIndexChange: (index: number) => void;
  /** Rendered below the dots, INSIDE the carousel, so arrow keys work while a
   *  button in it has focus — which is where focus sits after "Next". */
  footer?: ReactNode;
  className?: string;
}

export function Carousel<T>({ label, slides, renderSlide, index, onIndexChange, footer, className }: CarouselProps<T>) {
  const groupId = useId();
  const last = slides.length - 1;

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const delta = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (delta === 0) return;
    event.preventDefault();
    onIndexChange(Math.min(last, Math.max(0, index + delta)));
  }

  return (
    <div
      role="group"
      aria-roledescription="carousel"
      aria-label={label}
      onKeyDown={onKeyDown}
      className={cn("flex flex-col", className)}
    >
      <div className="overflow-hidden">
        <div
          className="flex transition-transform duration-[var(--motion-duration-medium)] ease-[var(--motion-ease-standard)] motion-reduce:transition-none"
          style={{ transform: `translateX(-${index * 100}%)` }}
        >
          {slides.map((slide, position) => (
            <div
              key={`${groupId}-${position}`}
              role="group"
              aria-roledescription="slide"
              aria-label={`${position + 1} of ${slides.length}`}
              aria-hidden={position !== index}
              inert={position !== index}
              className="w-full shrink-0"
            >
              {renderSlide(slide, position)}
            </div>
          ))}
        </div>
      </div>

      <div role="tablist" aria-label={`${label} slides`} className="flex items-center justify-center gap-1 pt-3">
        {slides.map((_, position) => (
          <button
            key={`${groupId}-dot-${position}`}
            type="button"
            role="tab"
            aria-selected={position === index}
            aria-label={`Slide ${position + 1}`}
            onClick={() => onIndexChange(position)}
            // The hit area is larger than the dot it draws, so it is easy to land on.
            className="group grid size-5 place-items-center rounded-pill"
          >
            <span
              className={cn(
                "size-2 rounded-pill transition-colors duration-[var(--motion-duration-fast)]",
                position === index ? "bg-accent" : "bg-sunken-strong group-hover:bg-text-tertiary",
              )}
            />
          </button>
        ))}
      </div>

      {footer}
    </div>
  );
}
