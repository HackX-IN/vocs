# Vocs — Design System

Exact values, so the look is decided once here rather than improvised per component. If a component needs a value that is not in this file, add it here first.

## 1. Principles

1. **An instrument panel, not a document window.** The interface is a set of numbered cards, each one an instrument that shows one thing: a figure in a dot-matrix face, a small dot chart, a quiet label. Three things make that read — mono capital eyebrows with an accent index ("01 · TIME SAVED"), the dot-matrix display face, and dot-lattice charts. If a screen looks like a form or a table, it has lost the idea.
2. **Paper in the light, ember in the dark.** Light is warm off-white paper with white cards and ink text; dark is warm near-black graphite with a faint ember bloom from the top-left corner. Both are solid: surfaces are 94–96% opaque so the look is calm and legible, and the native vibrancy behind them only has to carry the window's rounded shape.
3. **One accent, one signal.** `--accent` (signal orange) marks the emphasised thing: the selected nav item, the index numeral on a card, the newest dot in a chart, the primary action, the focus ring. `--signal` (live green) means a session is running or text was delivered, and it appears on the pill's bars and nowhere decorative. `--danger` is failure and destruction only, and its hue (~350°) cannot be mistaken for the orange (~20°). Anything else coloured is a bug.
4. **Depth is layering.** A page, cards floating on it with a hairline and a soft shadow, and one pill on top of everything. Cards are the unit; a card inside a card is two cards too many — group inside a card with spacing and a hairline.
5. **The display face is earned.** The dot-matrix face is for the page title and the one big figure per card. It is never used below `--text-title-size`, and never for anything that has to be read as prose.
6. **Motion is physics, not duration.** Springs, not eases. Nothing in the app moves linearly except a countdown, which must, or it lies about the time left.
7. **The pill has a hard budget: 60fps, always.** It renders while a realtime audio thread and an inference worker are both running. Anything expensive in the pill is wrong by definition.

---

## 2. Color tokens

Defined as CSS custom properties on `:root`, redefined under `@media (prefers-color-scheme: dark)`. **Never write a hex value in a component.** The app follows the system appearance; there is no theme switch.

### Light — paper

```css
--surface-glass:         rgba(244, 243, 238, 0.94);  /* the page */
--surface-elevated:      rgba(255, 255, 255, 0.96);  /* a card */
--surface-sunken:        rgba(27, 26, 23, 0.045);    /* hover */
--surface-sunken-strong: rgba(27, 26, 23, 0.08);     /* press, selected, a tile */
--border-hairline:       rgba(27, 26, 23, 0.09);
--border-highlight:      rgba(255, 255, 255, 0.9);   /* top inner edge */
--text-primary:          rgba(27, 26, 23, 0.92);
--text-secondary:        rgba(27, 26, 23, 0.58);
--text-tertiary:         rgba(27, 26, 23, 0.38);
--accent:                #E8501A;   --accent-soft: rgba(232, 80, 26, 0.12);   --accent-ink: #FFFFFF;
--signal:                #4FA823;   --signal-soft: rgba(79, 168, 35, 0.14);
--success:               #3F9B2C;   --warning:     #B9770E;
--danger:                #D6334A;   --danger-soft: rgba(214, 51, 74, 0.10);
--page-glow:             radial-gradient(900px 420px at 6% -10%, rgba(255, 150, 90, 0.12), transparent 70%);
--dot-grid-ink:          rgba(27, 26, 23, 0.10);
--shadow-card:           0 1px 2px rgba(28,24,20,.04), 0 8px 24px rgba(28,24,20,.06);
--shadow-panel:          0 24px 64px rgba(28, 24, 20, 0.14);
```

### Dark — ember

```css
--surface-glass:         rgba(19, 17, 15, 0.94);
--surface-elevated:      rgba(30, 28, 26, 0.96);
--surface-sunken:        rgba(255, 244, 230, 0.05);
--surface-sunken-strong: rgba(255, 244, 230, 0.09);
--border-hairline:       rgba(255, 244, 230, 0.09);
--border-highlight:      rgba(255, 244, 230, 0.07);
--text-primary:          rgba(246, 242, 235, 0.94);
--text-secondary:        rgba(246, 242, 235, 0.58);
--text-tertiary:         rgba(246, 242, 235, 0.36);
--accent:                #FF6A2B;   --accent-soft: rgba(255, 106, 43, 0.16);   --accent-ink: #1A0D06;
--signal:                #7BC83B;   --signal-soft: rgba(123, 200, 59, 0.16);
--success:               #7BC83B;   --warning:     #F0A93A;
--danger:                #FF5D73;   --danger-soft: rgba(255, 93, 115, 0.14);
--page-glow:             radial-gradient(900px 440px at 6% -12%, rgba(255, 122, 48, 0.20), transparent 70%);
--dot-grid-ink:          rgba(255, 244, 230, 0.09);
--shadow-card:           0 1px 2px rgba(0,0,0,.30), 0 8px 24px rgba(0,0,0,.28);
```

**Two sunken steps are the app's interaction feedback.** `--surface-sunken` is hover; `--surface-sunken-strong` is press, an icon tile, and a selected row. Hover is never a hue.

### Where colour is allowed, exhaustively

| Allowed | Where |
|---|---|
| `--accent` | The selected nav item and its icon; a card's index numeral; the newest column and the lit dots of a dot chart; the `primary` button fill; the focus ring; a link. |
| `--signal` | The pill's bars. A "delivered" glyph. A live-captions "listening" dot. |
| `--warning` | A recovered session's tile and label; a caution notice. |
| `--danger` | A failed session's tile and reason; the pill's failure; a destructive control. Used as a MARK (text, icon, dot, a soft wash) — never as a large fill. |

State is still carried by FORM first and hue second: a moving waveform means recording; a draining line means cancelling; a sentence means failed. Hue reinforces a distinction the shape already makes, so a screen is readable in greyscale.

**A failure the user can simply retry in a moment is not drawn as a failure.** `ENGINE_NOT_READY` — the engine still warming in the seconds after launch — is the most likely failure a new user will ever see, on their very first keypress, and it is temporary by definition. Painting the pill red there says the app is broken; it is seven seconds old. Transient failures keep the pill's ink colour and read "not yet". The distinction is made on `ErrorCode`, never on message text.

### The pill is dark in both schemes

```css
--pill-surface: #1D1D1D;  --pill-ring: #3A3A3A;  --pill-ink: #E6E3DC;
--pill-signal:  #7BC83B;  --pill-danger: #FF7A8A; --pill-danger-wash: rgba(255, 93, 115, 0.16);
```

It floats over arbitrary content; a near-black capsule with a lighter ring is legible over all of it, which a scheme-following pill is not.

---

## 3. Materials

Three utilities, one treatment each, kept from the glass era because the **window** is still a native `NSVisualEffectView` cut to `--radius-window` — it supplies the rounded shape and the shadow, and the web layer paints the surface over it.

| Token | Native material | Used for |
|---|---|---|
| `material-pill` | `HudWindow` | The pill window. Its surface is `--pill-surface`, fully opaque. |
| `material-panel` | `Sidebar` | The dashboard window. Painted by `body[data-window]` as `--page-glow` over `--surface-glass`. |
| `material-elevated` | `Popover` | Cards, the search dock, keycaps. Surface `--surface-elevated`, shadow `--shadow-card`. |

Every surface is the same three-part treatment: surface colour, a 0.5px hairline, an inner top highlight, and a shadow. `GlassPanel` is the one primitive that applies it; `Card` is built on it. Cards pass `noise={false}`: at 96% opacity the 3% grain is invisible and costs a layer.

**Reduced transparency.** Surfaces are already near-opaque, so `prefers-reduced-transparency` only has to remove the noise and the backdrop blur; the opaque colours below are the fallback when a surface token is overridden to solid.

```
Light:  --surface-opaque: #F4F3EE    --surface-opaque-elevated: #FFFFFF
Dark:   --surface-opaque: #13110F    --surface-opaque-elevated: #1F1D1A
```

---

## 4. Geometry

```
--radius-pill:   26px      /* HALF THE PILL'S WIDTH: the capsule is fully round */
--radius-window: 26px      /* the native window corner, read by src-tauri */
--radius-panel:  28px
--radius-card:   20px
--radius-input:  12px

--space-1: 4px   --space-2: 8px   --space-3: 12px   --space-4: 16px
--space-5: 24px  --space-6: 32px  --space-8: 48px

--rail-width:        56px     /* the icon-only rail window, §8 */
--control-height:    32px     /* buttons, selects, segmented controls */
--control-height-sm: 28px     /* a control inside a row, §11 */
--card-padding:      20px
--content-max-width: 720px    /* the reading column: Dictations list and editor */
```

Radii are large on purpose: a 20px card corner reads as an object, a 6px one as a table cell. A card inside a window never fights the window's own corner because the page padding (`--page-padding-x`, 32px) is larger than `--radius-window`.

Borders are **0.5px**, not 1px. On a Retina display 0.5px is a true hairline; 1px reads as heavy.

---

## 5. Typography

Two voices and one label style.

| Role | Face | Size / line | Weight | Used for |
|---|---|---|---|---|
| Display | **Doto** (dot matrix) | 52 / 56 | 800 | The one big figure on a hero card |
| Title | **Doto** | 34 / 40 | 800 | The page title; a dictation's title |
| Heading | SF | 16 / 22 | 600 | A section heading inside a card |
| Reading | SF | 15 / 26 | 400 | A dictation's text while editing |
| Body | SF | 13 / 18 | 400 | Everything you read in chrome |
| Label | SF | 12 / 16 | 500 | A control's label, a row's secondary text |
| Caption | SF | 11 / 14 | 400 | Meta, units, dates |
| **Eyebrow** | SF Mono, **capitals**, +0.10em | 10.5 / 14 | 500 | A card's title: "01 · TIME SAVED" |
| Mono | SF Mono | 12 / 18 | 400 | Times, hotkeys |

System fonts (`-apple-system` → SF Pro, `SF Mono`) read everything a person has to read. **Doto** is a bundled font (`@fontsource-variable/doto`, SIL OFL-1.1) — it works offline and costs one 25 KB file, which is why it is allowed where "never ship a webfont" used to be the rule. It is the interface's only decoration and it is used sparingly: **never below `--text-title-size`**, where its dots stop reading as dots and start reading as noise, and never for a sentence.

Line heights are absolute, not multipliers, so a row of UI sits on one baseline grid whatever the size. Numbers in stats use `font-variant-numeric: tabular-nums` so a figure does not jitter as it updates.

---

## 6. Motion

Springs only. Reference values (Framer Motion syntax; a CSS spring equivalent is fine):

| Motion | Spring | Note |
|---|---|---|
| Pill appear | none — **instant** | A fade-in delays the only feedback that the keypress registered |
| Pill exit | not a spring — see below | Owned by the WINDOW, not the document |
| State change | `stiffness: 300, damping: 25` | Colour morph. **Not width** — see §7 |
| Panel / sheet | `stiffness: 260, damping: 26` | |
| Waveform bars | no spring | Driven directly by audio RMS at 60fps |
| Countdown line | linear | The one exception — a decreasing timer must be linear or it lies about remaining time |

**The pill's own motion is not in this table, and must never be put back into it.** The pill's glass is a native `NSVisualEffectView` sized to its window, so anything CSS moves — a scale, a translate, a width morph — slides out from under the glass and reveals it. The pill appears instantly and leaves by animating the WINDOW: `--pill-exit-duration-ms` (160) and `--pill-exit-travel` (8), read out of `tokens.css` by `src-tauri/src/tray.rs` so the motion and the design stay one fact. The travel direction carries the outcome — **up for committed, down for cancelled** — which is the whole reason the exit is animated at all, since nothing else on screen distinguishes the two once the pill is gone. Failed rises too: it is not a discarded recording, and dropping it would read as the words being thrown away.

A spring in this table can only drive motion **inside** a surface. Moving a surface is the window's job.

**Respect `prefers-reduced-motion`:** replace every spring with a 120ms opacity fade. Do not simply disable animation — things appearing instantly reads as a glitch.

### 6.1 CSS-side motion

Springs above are for mount/dismiss and morphs, and they run in JS. Hover, press and focus feedback must never take a JS frame — the pill's budget does not allow it — so those are plain CSS transitions with these values:

```
--motion-duration-fast:   120ms   /* hover, press, focus ring; also the reduced-motion fade */
--motion-duration-medium: 240ms   /* enter/exit of non-critical chrome */
--motion-ease-standard:   cubic-bezier(0.32, 0.72, 0, 1)   /* spring-shaped, no overshoot */
--motion-ease-linear:     linear                            /* countdown ring only */
```

`--motion-duration-fast` and the reduced-motion fade are deliberately the same number: under reduced motion every spring collapses to exactly the duration the app already uses for its smallest transition, so nothing feels out of place.

### 6.2 Springs as tokens

Framer needs numbers, and a component may not carry one. Every spring in the table above is published as a pair of custom properties — `--spring-<name>-stiffness` / `--spring-<name>-damping` — and read out of the stylesheet by `src/lib/motion.ts`. A component asks for a spring by name, never for `{ stiffness: 400 }`. `--spring-pill-appear-*` survives for an opacity-only fade of pill CONTENT; it must never be wired to a transform, for the reason in the table above.

## 7. The pill — exact specification

```
      ╭────────╮
      │   ◖"◗   │   the mark, 26px wide, pale ink; breathes while recording
      │        │
      │        │
      │  ▏▎▍   │   three bars: short · tall · medium, signal green
      ╰────────╯
        52 × 104 pt      radius 26 (half the width), 2px ring
```

- **A vertical capsule.** The operator's reference is a near-black pill with the app's mark above three green bars. It is **52 × 104 pt** (`--pill-width` / `--pill-height`, the reference's 1 : 2) with `--radius-pill` = half the width, so it is fully round; the surface is `--pill-surface` and it is outlined by a `--pill-ring-width` (2px) `--pill-ring` ring. It is dark in both colour schemes (§2).
- **The window is the pill.** The NSPanel is sized to `--pill-width`/`--pill-height` exactly and its vibrancy carries `--radius-pill`, so every pixel of the window is the capsule. Two consequences bind everything else here: the pill fills the window and **never animates its own size or transform** (object motion — arriving, resizing, leaving — belongs to the window, in Rust; only motion *inside* the pill belongs to CSS), and the pill has **no CSS drop shadow**, because a shadow has nowhere to fall and is clipped by the window bounds. Its shadow is the native window shadow. `src-tauri/src/tray.rs` `include_str!`s `tokens.css` and parses these values at startup, so the window has no second copy of them to disagree with. **Change a token and the window follows — there is no Rust number to edit.** Delete a token the pill needs and it is a build-time panic naming it.
- **Position:** bottom-centre, `--pill-offset-bottom` (96pt) above the bottom edge of the screen containing the cursor. Multi-monitor: follow the active screen. Position is decided **once per appearance** — enforced by an `is_visible()` gate in `set_pill_visible`, not by intent. State is emitted several times a second, so positioning unconditionally makes the pill chase the pointer between displays mid-sentence.
- **The mark** (`Mark size="md"`, §12) sits above the bars in `--pill-ink`. While recording it breathes — a 2s opacity pulse (`--pill-pulse-*`), CSS only, so it costs the audio thread nothing. It is the one thing that tells you the app is alive while you speak.
- **Three bars**, `--pill-bar-width` (5px) wide with `--pill-bar-gap` (6px) between, up to `--pill-bar-max` (26px) tall, in `--pill-signal`, growing from their centre. They are an **equaliser, not a scrolling history**: the centre bar follows the current level, the flanks trail it by one and two frames with their own weights, and each is floored at its resting height (short · tall · medium = 0.38 · 0.72 · 0.52 of full) so silence shows the signature shape rather than three dots. Driven by windowed RMS straight into `transform: scaleY` through a ref — no React render per audio frame.
- **Waveform input scaling.** The RMS arriving from the audio thread is linear 0..1, and real speech sits between roughly 0.05 and 0.3 — so a bar drawn at `height = rms` is a flat line for the entire recording. Two values map the audio range onto the visual one:

  ```
  --waveform-input-ceiling: 0.35   /* the RMS treated as a full-height bar */
  --waveform-input-gamma:   0.6    /* < 1 lifts quiet speech into view */
  ```

  `level = clamp((rms / ceiling) ^ gamma)`. The ceiling sits just above normal speech so the loud end has somewhere to go before it pins; the gamma is there because loudness is perceived logarithmically. Together they put 0.05 at roughly a third height and 0.3 near full. The same curve draws the Settings microphone check.
- **No timer.** The capsule is the shape of "I am listening"; a number in it would make it a gauge, and the vertical shape has no room for one without crowding the bars.
- **Arming** draws *exactly* what recording draws — the mark and the bars at rest. It is normally under 100ms, and a distinct state for a tenth of a second is not information, it is a flicker. The capsule arrives already in its working shape and comes alive as sound reaches it. A genuine stall surfaces as Failed.
- **Cancel armed:** the bars are replaced by a **vertical countdown line** in the same slot — `--countdown-line-height` (3px) thick, `--countdown-line-length` (26px) tall, starting full and draining toward the bottom. **There is no label.** A countdown the user armed with a keypress does not need a caption telling him he armed it. A second Escape refills the line with a spring and returns the bars.
- **The line is driven by `remaining_ms`, never by a local timer, and the animation is created ONCE.** The old ring failed because its effect depended on `durationMs` — which *was* `remaining_ms`, a value Rust re-emits several times a second — so every tick cancelled the running animation and restarted a fresh full-to-empty drain. It never progressed and it flickered: two clocks disagreeing. Later ticks may only SEEK the running animation, and only when they disagree with it by more than 120ms. **Never read the countdown's duration as a constant** — it is configurable, and a skip-countdown setting exists.
- **Failed:** the one WIDE state — **360 × 56 pt**, a horizontal capsule with the mark at the left and the reason beside it on **up to two lines, never truncated**. A `--pill-danger-wash` tints the surface and the sentence is `--pill-danger`. Failure is a sentence, not a status: the corpus it must seat runs to 100 characters (*"Vocs heard nothing at all. Check that the right input device is selected and that it is not muted."*), which at `--text-label` wraps to at most two lines at 340pt. The size is fixed rather than fitted to the line count, because only the webview can measure text and only Rust can size the window — a fitted height would need a round trip and would show a frame at the wrong size. It keeps `--radius-pill`, so it is visibly the same object.
- **Not ready yet:** a failure with a transient code — currently `ENGINE_NOT_READY` — keeps `--pill-ink` and shows its reason plainly. Same layout as Failed, none of the alarm. See §2.
- **There is no Finalizing and no Delivered.** Releasing the hotkey ends capture instantly; transcription and pasting continue in the background with nothing on screen. The pill is gone the moment the key is up. **Failed re-appears** a second or two later if the background half could not finish, which is the only reason the pill is ever seen again after a release.
- **IDLE never blanks the view.** Rust hides the window on IDLE and the same transition also reaches the webview over IPC; those do not land on the same frame. A view that cleared on IDLE would leave an empty capsule on screen until the native hide caught up. The view renders the last state that had something to say and lets the window take it away.

**How long Failed stays on screen is enforced in Rust, not in CSS**, because the FSM owns whether the pill is visible. It is deliberately absent from `tokens.css`: it was there once, unused, and an unused token that looks authoritative is worse than no token.

**Never:** a close button, a title bar, a CSS shadow, or any control. It is an indicator, not a window. Every interaction is via keyboard.

## 8. Dashboard layout

### The update notice

A slim bar docked BELOW the scroll area, on every dashboard page, absent entirely until there is a
newer version.

- **It must never take the screen.** The person using this app is usually dictating into something
  else and the dashboard may not even be open. An update prompt that interrupts that is worse than
  one found late, because the product's whole promise is "talk and the words appear" and anything
  stealing focus mid-sentence breaks it. Not a modal, not a toast, not an overlay — docked below the
  content so it covers nothing and moves nothing.
- **It asks as well as listens**, and that is what makes the feature visible at all. The backend
  checks at launch and every 24h and emits an event, but the dashboard opens from the menu bar and
  may not exist when that event fires — a listener alone would miss nearly every one and the feature
  would appear broken while working perfectly. So: `check_for_update` once on mount for the answer
  that already exists, plus the event for a check that lands while it is open. Same shape as the
  pill — a command for first paint, events after that.
- **The restart is in the button label**, not in a sentence beside it. Installing swaps the running
  app, and the label is the last thing read before committing.
- The button is a bordered secondary. The `primary` (accent) fill is reserved for the view's one
  primary action, and a bar whose entire purpose is to stay quiet must not carry the loudest control
  in the product.

### Registry-declared icons

`NavDef.icon` is a STRING that crosses from Rust into TypeScript and is resolved through a
hand-maintained map in `src/lib/icons.ts`. A name with no entry does not fail to build, does not
warn and does not throw — it renders the fallback. Two shipped that way (`CreditCard`,
`ChartNoAxesColumn`) and sat in the sidebar wearing a plausible placeholder until the operator
reported it.

- **The fallback is deliberately ugly.** It was `CircleDot`, which looks like a designed icon and is
  exactly why the bug survived. It is now `CircleHelp`: a fallback's job is not to look acceptable,
  it is to be read as "this is missing" in about a second.
- **The guarantee is a test, not a glyph.** `src-tauri/src/registry/icons.rs` reads `icons.ts` and
  asserts every declared nav icon resolves. It lives on the side that declares the name because
  TypeScript cannot know what strings Rust will send — the same arrangement as `reachability.rs`
  and as `tray.rs` reading `tokens.css`. Adding a capability with an unmapped icon now fails
  `cargo test` with the route and the icon name in the message.


### The page header, the scroll fades, and no scrollbars

- **One header component, every page** (`PageShell`). The page title lives here, not inline in each
  view, and it is read from the capability registry's `NavDef.label` — a page added to the registry
  arrives titled with no frontend edit. A view that draws its own header (the dictation editor)
  tells the shell `title={null}` and the shell adds nothing.
- **Two rows, because the lights and the title are different jobs.** macOS places the traffic lights
  at a fixed inset from the window's TOP-LEFT (roughly 20–72pt). They know nothing about our layout
  and cannot be moved from CSS, so the first row is an empty `--traffic-light-band` (28pt, the
  standard title bar) that exists only to be a drag handle beside them. The title sits on its own
  row beneath, on the page's own grid at `--page-padding-x`, set large in the display face. The old
  arrangement squeezed the title into the lights' row, which forced it small and indented.
- **The header is the drag region**, and it is deliberately generous (`--page-header-height`, 92pt).
  It never moves while the page scrolls, so the window is always easy to grab. Interactive children
  opt out with `data-tauri-drag-region={false}` or they drag the window instead of firing.
- **The header OVERLAYS the scroll area** rather than sitting above it, so content tucks under it
  and fades. `ScrollArea`'s `headerInset` pads the content down past the bar — that padding is the
  guarantee that content is never hidden, and it lives in one place so no page has to remember it.
  `DataList` takes the same `headerInset` for the same reason.
- **The fades are a MASK, not a painted gradient**, so the content itself dissolves. Smart means each
  edge is present only when something is actually hidden behind it — a fade with nothing behind it
  reads as a rendering artefact. Driven by `useScrollEdges`, which watches scroll position, viewport
  size AND content size.
- **No scrollbars anywhere.** The fade is the affordance that replaces them. This hides the BAR,
  never the scrolling.

### The window

```
+------------------------------------------------------+
| o o o                                                |  traffic lights, 28pt band
|                                                      |
|  [mark]  Dictations                    (display face)|  title row, then content
|  [list]  +----------------------------------------+  |
|  [bars]  | THIS WEEK   STREAK     TIME SAVED      |  |  904 x 640 default
|  [gear]  | 52 words    1 day      1m              |  |  min 784 x 560
|  [heart] +----------------------------------------+  |
|          TODAY                                    8  |
|          [doc] Let's keep a meeting…       3:10 PM  |
+------------------------------------------------------+
   rail (own window)                       [ search · export ]  <- the dock
```

**The window IS the floating panel.** The native window is cut to `--radius-window` and the page
paints `--page-glow` over `--surface-glass` edge to edge, so there is one surface and no second panel
drawn inside it.

- Hidden title bar with inset traffic lights (`titleBarStyle: Overlay`).
- **The rail is its own window** — a capsule of icons hanging off the dashboard's left edge by
  `--rail-detach-gap`, content-sized (its height is a formula over the registry's nav entries; see
  `tokens.css`). It paints its own solid `--surface-elevated`, because the page is no longer a
  see-through surface and a bare rail would be the only see-through object in the app. The mark is
  at the top; below it one icon per registry nav entry. **Selection is the accent on its soft wash**
  (`--accent-soft` + `--accent`); hover is `--surface-sunken`. It has no drag region — dragging a
  child window would tear it from its parent.
- **Dictations is the home of the app** (nav order 10): the notebook of everything said. Stats is
  second, Settings third.
- **Stats:** a grid of numbered cards (§11). Time saved is the one hero card (it alone carries the
  dot lattice); the streak, the headline counts, the languages and the latency table are plain
  cards, numbered in reading order by the view.
- **Settings:** a two-column page — a section list on the left, the section's cards on the right
  (§14).

## 9. Empty and error states

Every list and stat has a designed empty state. A blank panel reads as broken.

- **No transcriptions yet:** the hotkey rendered as a keycap, and one line: "Press ⌥Space anywhere to start."
- **Model downloading:** progress with real bytes and a real estimate. Never an indeterminate spinner for a 574MB download.
- **Permission missing:** a single sentence naming what is missing and one button that deep-links to the right settings pane.
- **Failed transcription:** the reason in plain language, and the audio duration that was captured, so the user knows what was lost.

## 10. Accessibility

- Every interactive element reachable by keyboard, with a visible `--accent` focus ring.
- The pill is marked as a status/live region so VoiceOver announces state changes rather than staying silent.
- Text contrast meets 4.5:1 against the *effective* background — glass over a bright desktop is the worst case, which is why `--surface-glass` sits at 55% rather than lower. **Verify against a white desktop background, not a neutral gray one.**
- Full `prefers-reduced-motion` and `prefers-reduced-transparency` support. Under reduced transparency, glass becomes an opaque solid surface — that is a supported look, not a degraded one.

## 11. Component metrics

Values the primitives need that the sections above imply but do not state. Same rule: if a component needs a number that is not here, it goes here first.

### Surfaces

```
--border-width-hairline:  0.5px           /* §4 — the hairline, stated once */
--noise-opacity:          0.03            /* §3 */
--glass-backdrop-blur:    24px            /* the non-vibrancy fallback only */
--card-padding:           20px            /* a Card's body and header inset */
--card-header-height:     20px            /* the eyebrow row, so a card without actions is as tall */
```

### Card

The one surface for a unit of content (`components/global/card`). Optional **eyebrow** (`index` in `--accent`, then the title in mono capitals), optional **actions** on the right of the header, and a body. `tone="hero"` lays the dot lattice behind the content, fading toward the bottom-left — **one per view**, on the card that holds the figure the view exists to show; repeating it stops it meaning anything. `flush` drops the body padding for a card that brings its own (a list, a table). The index is supplied by the caller, in reading order, because a card that numbered itself would renumber when another one hid.

```
--dot-grid-radius: 0.9px      --dot-grid-pitch: 14px     /* the lattice, decoration only */
```

### Chip

A rounded `hairline` label for a fact about the thing on the page — a date, a duration, a language — with an optional leading icon (`components/global/chip`). Display only: it carries no handler, so a row of chips cannot be mistaken for a row of buttons. Height 28px (`h-7`), `--text-label`, `--surface-sunken`.

### Button

`default` is a hairline-bordered sunken control; `primary` is the accent fill with `--accent-ink` — **at most one per view**, the thing the view wants you to do; `ghost` is transparent until hovered, for toolbars and the dock; `danger` is `--danger` with `--accent-ink`, used for the confirming step of a destructive action, never as the first click.

### Focus

```
--focus-ring-width:   2px
--focus-ring-offset:  2px
```

Colour is `--accent` (§10). Offset rather than inset, so the ring never sits on top of a hairline border and muddies it.

### List rows and the dock

```
--row-height:        64px    /* a Dictations row, and a day header — both fill one slot */
--row-overscan:      6       /* rows rendered outside the viewport, each side */
--dock-height:       48px    /* the floating search bar */
--dock-offset:       20px    /* its distance from the window's bottom edge */
--dock-clearance:    88px    /* dead space under the last row, so it scrolls clear of the dock */
--dock-max-width:    560px
--content-max-width: 720px   /* the reading column */
--history-summary-height: 112px  /* the summary strip plus the gap under it */
--history-filters-height: 56px   /* the period + status pickers under it */
```

A uniform row height is a constraint, not a limitation: it is what lets a list of tens of thousands of rows resolve its scroll position by arithmetic instead of by measurement. A day header is therefore a **row like any other** — it fills one 64pt slot with its label at the bottom, which reads as spacing — and the list steps over it (`isSelectable`) for the keyboard and gives it no hover. A list that needs genuinely variable-height rows needs a different component and a reason. Content above the rows that scrolls with them (the summary strip) is a `lead` of known `leadHeight`; the virtualiser reads the scroll position past it.

### Charts

**A chart in this app is data drawn as a lattice of dots, with no chrome around it.** Default chart-library styling is the single fastest way to make a product look like every other product, because it is literally the same styling every other product ships. So:

- **No gridlines, axes, frame, tick marks, legend or tooltip.** The dim dots are not chrome: they are what makes a column read as a column of fixed height, so "half full" is visible without a scale.
- **No coloured fills.** Lit dots are `--text-primary` at reduced opacity; **the newest column is `--accent`** — colour carries one fact in a chart, where the series ends.
- **No animation on load.** The number is the point; a chart that grows into place is a toy.
- **Numbers are tabular** everywhere, so a value does not reflow as it updates.
- **No chart library.** A dot is a `<circle>`; the whole chart is a few hundred of them.

```
--dot-row-pitch: 11px    /* vertical distance between dot centres in a DotColumns chart */
--dot-radius:     3px    /* the largest dot a column chart draws; narrow columns draw smaller */
--dot-size:       5px    /* a DotMeter dot's diameter */
--chart-row-height: 36px /* one categorical row */
--chart-label-gutter: 72px
```

- **`DotColumns`** — one column per value, lit dots rising from the floor, 0-to-busiest and never stated. A value above zero always lights at least one dot, because "something happened" and "nothing happened" must not look the same. Measured in real pixels (`useElementBox`) rather than drawn into a stretched viewBox, which would turn every circle into an ellipse.
- **`DotMeter`** — a row of a fixed number of dots with the first `value` share lit: the proportional bar.
- **The activity chart** is sessions per day over the real calendar, **zero days included** (`fillCalendar`): the backend sends only days with a session, and a chart drawn from those puts Monday next to Friday. The range is chosen with a segmented control, never a dropdown — the options are three, mutually exclusive, and a select would hide two of them. Counts are plotted raw: a trailing mean would be a number this app did not measure.

### Segmented control

```
--segment-height:    28px   /* = --control-height-sm */
--segment-radius:    var(--radius-input)
--segment-padding:   2px    /* the track's inset, so the thumb clears the track edge */
```

Low contrast on purpose: the track is `--surface-sunken`, the selected segment is `--surface-sunken-strong` with `--text-primary`, and the unselected labels are `--text-secondary`. Choosing a range is not the view's primary action, so it is never accented.

### Interaction timings

Not motion — these are how long the interface waits before acting, and they belong here for the same reason a duration does: improvised per view, the app feels inconsistently responsive.

```
--search-debounce:  180ms   /* keystroke to query dispatch */
--autosave-delay:   700ms   /* the editor waits this long after the last keystroke before saving */
--feedback-hold:   1200ms   /* how long a confirmation glyph stays after an action */
--titlebar-height:  52px    /* the window bar: lights, centred title and toolbar share its centre line; drag region fills it */
```

180ms is under the ~250ms at which a person perceives a pause, and long enough that a typed word costs one query rather than five. 700ms is long enough that a sentence being typed is one write and short enough that closing the window right after typing loses nothing (leaving flushes anyway).

### Keycap

```
--keycap-height-sm: 18px   --keycap-height-md: 24px
--keycap-min-width-sm: 18px --keycap-min-width-md: 24px
--keycap-radius: var(--radius-input)
```

Square at minimum so a single glyph (`⌥`) reads as a key rather than as a letter, and widening with content so a word (`Space`) stays one key.

### Spacing, as Tailwind sees it

The §4 scale is a 4px grid, so it is published as Tailwind's spacing base (`--spacing: 4px`) rather than as seven named steps. `p-1 p-2 p-3 p-4 p-6 p-8 p-12` are exactly `--space-1` … `--space-8`. Naming them twice would let two names for 24px both be correct, which is how a scale stops being one.

## 12. Identity

The app had no identity, and that is why it read as an MVP: nothing on screen was specific to *this* product.

### The mark and the icon

**A head that is whispering.** The app icon is a black, head-shaped blob rising from the bottom-left of a pale squircle, with two white quote marks beside its crown — the words leaving the head. The operator supplied the reference; the vector in `assets/icon.svg` is a redrawing of it, authored in a normalised 0..100 space so the head, the quotes and the squircle are one set of numbers.

**One drawing, three renderings, and they share their path.**

| Rendering | File | Notes |
|---|---|---|
| App icon | `assets/icon.svg` → `src-tauri/icons/*` | 1024 canvas, 824pt squircle, soft shadow, paper-to-ink gradients. Rendered with Chrome headless and cut to `.icns`/`.ico` with `tauri icon`. |
| Menu-bar glyph | `assets/mark.svg` → `icons/tray.png`, `tray@2x.png` | One solid colour, tight to the head, the quotes **cut out as holes** — as a macOS template image the system recolours it for light, dark and pressed, so it must carry no colour of its own. |
| In-app mark | `components/global/mark` | The same path inline, `currentColor`. Ids come from `useId`, because two marks on one page would otherwise share a mask id. |

Change the path in `assets/mark.svg` and `Mark.tsx` together or not at all. The mark is `--text-primary` in the rail and `--pill-ink` in the pill. It appears in exactly four places: the **top of the rail**, **onboarding's first screen** at `--mark-size-lg`, the **pill**, and — as the SVG renderings — the **menu-bar glyph and the bundle icon**. It does NOT go on the "no transcriptions yet" empty state: that state's job is to say which key to press, and a logo above the keycap makes the user read a brand before they read the instruction. An identity repeated on every surface is a watermark.

```
--mark-size-sm: 18px   --mark-size-md: 26px   --mark-size-lg: 48px   /* width; height follows the 91.2 : 75.5 aspect */
--rail-mark-height: 22px   /* the mark's drawn height at md — the rail's height formula reads it */
```

Onboarding shows the mark on the tour and nowhere else. The setup steps that follow carry **no progress dots**: they are derived from backend state rather than a fixed-length sequence, so a "1 of 3" would announce a length that varies per machine.

### Voice

Sentence case everywhere in running UI — never Title Case, which reads as a marketing site. The exception is the **eyebrow**, which is mono capitals by design. Labels are quiet: `--text-secondary` at label size, never bold. The only heavy type on a screen is the page title and the one figure each card exists to show.

Numbers are the loudest thing in the interface, because the product's whole claim is a measurement. That is why the display size exists and why almost nothing else is above Heading.

### What "premium" means here, concretely

It is not more effects. It is: two typefaces and no more, one accent hue, large radii, soft shadows, quiet labels, one loud figure per card, a numbered grid that reads in order, and generous spacing that is still grouped tightly enough to read as deliberate. Spread everything out evenly and it stops looking minimal and starts looking unfinished.

---

## 13. Dictations: the list and the editor

The home of the app is a notebook of everything said, and its two pages are a list and a document.

### The list

- **Rows are documents.** A 40pt rounded tile (`--surface-sunken-strong`, a document glyph), the dictation's name over a line of facts (`12 words · English · 0:08`), the time of day at the right in mono. The name is the user's title if they gave one, else the first line of the text. A single click opens it. A pencil glyph marks a dictation the user has edited.
- **Outcome is carried by the tile as well as the words.** A failed session gets the `--danger-soft` tile and a warning glyph and leads its facts with the stored reason, verbatim; a *recovered* session (words saved after an interruption) is not a failure and gets the `--warning-soft` tile. A session where nothing was said reads "Nothing was said" in tertiary text — it is shown, never hidden, because a recording that left no trace is the worst outcome this app has.
- **Day headers** — "Today", "Yesterday", "Wed, Oct 15" (the year only once it stops being this one) — are eyebrows with the day's count at the right. Days are compared by local midnight.
- **The summary strip** is one short card at the top (`HistorySummary`): words this week, the streak and time saved, in the display face, divided by dashed rules. It scrolls away with the list and hides for a brand-new user — three zeros are not encouragement, they are a form.
- **The search dock** floats over the foot of the list: a pill-shaped `material-elevated` bar with the search field and, inside it, the export action. ⌘F focuses it from anywhere; Escape clears it; ↓ or ⏎ moves into the list. Search is the server's (an FTS query), never a filter over the loaded pages.
- **The list stays mounted while a dictation is open.** Coming back to the same scroll position, query and loaded pages is what makes the pair feel like one notebook.

### The editor

A dictation opens like a note — `#/history/<id>`.

- **Header** (its own, in the traffic-light row): a ghost back button after the lights, then at the right a save status (*Saved / Editing… / Saving… / Not saved*), Copy, and Delete. Delete is two clicks: the first turns it into a `danger` "Delete it" that disarms on blur.
- **Title** — large, in the display face, wrapping, editable. Empty shows the dictation's first six words as the placeholder, so an unnamed dictation still has a name. Enter moves to the body and never inserts a newline.
- **Chips** under the title: date and time, duration, language, word count.
- **Body** — the text in the reading face (`--text-reading-*`), a column no wider than `--content-max-width`, auto-growing so the page scrolls rather than the field. It is a plain textarea: spellcheck on, no toolbar, no formatting. A dictation is prose.
- **It saves itself** (`useAutosave`): debounced, serialised, flushed on leave. The edit goes to the *fixed* text and leaves *what was said* untouched, so the **Said → fixed** card at the bottom stays meaningful after the user rewrites half the sentence. "Use original" is a flag, not an overwrite, and the editor saves any unsaved words before it flips.

---

## 14. Settings

Seven short pages, not one long scroll.

```
Settings                                  (display face)
[ search settings ]   Your microphone, the shortcut, and how a dictation starts.
 ◉ Recording         ┌───────────────────────────────────────────────┐
   Transcription      │ 01 · RECORDING                                 │
   Output             │ Dictation hotkey        The key you press…  [⌥Space] │
   Vocabulary         │ ───────────────────────────────────────────── │
   Per-app            │ Hotkey behaviour        Toggle presses once…  [Toggle ▾] │
   Privacy            └───────────────────────────────────────────────┘
   General
```

- **A section list on the left, cards on the right** (`--settings-nav-width`, 188pt). The list is fixed while the cards scroll, so the page you are on is always in view. The selected page is the accent on its soft wash — the same selection language as the rail — and is **the route** (`#/settings/<page>`), so error actions and the menu bar deep-link to a page and the window's back/forward history is the history of the screen.
- **Search is the other way in.** The field sits above the list and searches every setting's label and description; while it has text the right side shows the matches, grouped under their page's name, rendered as the very same rows — a setting behaves identically whether found by browsing or searching. Escape clears it.
- **A page is one card of rows, plus a second card for the panel that belongs with it** — Models under Transcription, the Dictionary under Vocabulary, Your data under Privacy, Backup under General. Cards are numbered in reading order by position among those shown. A one-line blurb above the cards says what the page is for.
- **A row is a label with its description on the left and the control on the right**, separated by hairlines. Rows come from the registry's `SettingDef`s; nothing in the frontend lists a setting, so adding one in Rust puts it on its page with no frontend change. Pages are a *layout* judgement (`settings-pages.ts`); `pageForSection` is total over `SettingSection` so a section with no page is a compile error, not a setting that silently vanishes.
- **Advanced settings sit behind a disclosure at the foot of their card** — they exist to unstick a specific machine, not to be browsed.
- **A missing OS permission is announced once per page**, above the cards, with the button that opens the right System Settings pane — not once per blocked row.
- **An ON switch is the accent**; an OFF one is a sunken track with a quiet knob. A settings screen is a wall of switches and the lit ones are the ones doing something.

---

## 15. The caption HUD

One floating window for two features, because they are one object at two moments of a dictation: the panel that shows what Vocs is **hearing** while you speak is the one that shows what it is about to **write** when you stop.

```
 ╭──────────────────────────────────────────────╮
 │ ● LIVE                                       │   520 × 132 pt, above the pill
 │ HEARD um so we should ship it comma today    │   dark in both colour schemes
 │ So we should ship it, today.                 │
 ╰──────────────────────────────────────────────╯
 ╭──────────────────────────────────────────────╮
 │ PASTING IN A MOMENT              Esc to keep it out │
 │ ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │   a line, draining
 │ So we should ship it, today.                 │
 ╰──────────────────────────────────────────────╯
```

- **Same family as the pill.** A near-black `--pill-surface` panel with the `--pill-ring` outline, `--pill-ink` text, `--pill-signal` for the one live thing, cut to `--radius-caption` (24pt). It floats on every Space and over full-screen apps, is click-through (it can never take a click or focus from the app being dictated into), and is created once and only shown and hidden.
- **A fixed size the text is laid out into.** `--caption-width` × `--caption-height`, stood `--caption-offset-bottom` above the bottom edge — above the pill, which is `--pill-offset-bottom` + `--pill-height` + a gap. `src-tauri/src/tray.rs` reads these by name from `tokens.css`, exactly as it does for the pill, and places it **once per appearance** on the display under the cursor. It has no animated exit: the pill's exit carries the outcome ("committed" rises, "cancelled" drops) and a caption has no outcome to carry.
- **Live** — a signal-green dot that breathes, the eyebrow "LIVE", then the text. It shows the **end** of a long transcript (the last ~150 characters, cut at a word, with a leading ellipsis): while you speak, the newest words are the ones that matter. **"Heard" is shown only when it differs from "fixed"** — repeating identical text is noise. The words are the committed transcript plus a snapshot of the open chunk, so they arrive within a couple of seconds, not in ten-second lumps; see `session/captions.rs` for why that costs a little extra work and is therefore opt-in.
- **Preview** — the eyebrow "PASTING IN A MOMENT", "Esc to keep it out" at the right, a horizontal draining line (the same `CountdownLine`, created once per preview), and the **start** of the finished text on up to three lines — that is how you recognise a paragraph. It shows what will be pasted (after enhancement), not what was heard. Escape withholds the paste; the words are still saved in Dictations, marked "Not pasted".
- **It never blanks itself.** Rust hides the window and the same news reaches the webview over IPC on a different frame; a view that cleared on `null` would flash an empty panel at the end of every caption. It keeps the last caption and lets the window be taken away.
- **Escape belongs to a live recording first.** While a preview is up Escape means "keep this out"; the moment a new recording is live, Escape cancels *that* and the preview runs its course.

