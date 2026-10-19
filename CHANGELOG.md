# Changelog

All notable changes to Vocs. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **A new look.** A warm paper light theme and an ember dark theme, the Doto dot-matrix face for titles and figures, numbered instrument cards and dot-matrix charts. New app icon (a head whispering quotes) and matching menu-bar glyph.
- **Dictations**, the new home of the app: your history as a notebook grouped by day, with a summary strip and a floating search bar. Open a dictation to edit it like a notepad — a title you can change, text that saves itself, **⌘F** to find (⌘G for the next match) and **⌘S** to save now — while the **Said → fixed** comparison floats over the foot of the page. Filter the list by period (today, 7 or 30 days) and by status (edited, failed); searches and filters page as you scroll.
- **Writing style** (Settings → Output, and per app): Natural, Casual (contractions stay, a one-line message ends without a full stop), Formal (contractions written out, "gonna" becomes "going to") and Literal (exactly what was heard, for terminals and code editors). Vocs suggests a style when you add an app such as Slack, Mail or Terminal.
- **Preview before pasting** (Settings → Output, off by default): the finished text is shown for a moment and Escape keeps it out of the document. It is still saved in Dictations.
- **Live captions** (Settings → Recording, off by default): a small window shows what Vocs hears, and what it will write, while you speak.
- **A real menu bar** while Vocs is in front: Settings (⌘,), start or stop a dictation, copy the last one, jump to Dictations, Stats, Settings and Support (⌘1–4), Check for Updates, and a Help menu with the documentation, issue reporting and the logs folder. Edit gives the editor undo, redo, cut, copy, paste and select all.
- **A startup screen**: the mark, breathing bars and a dot-matrix wordmark while the window loads.
- **Hover tooltips on the Stats charts** (activity, streak and languages).
- **Per-app profiles as cards**, with one-click suggestions for the apps you have dictated into. Turning Vocs off in an app hides the settings that no longer apply.
- **Onboarding** has a four-step progress row and a picture for each tour slide.
- **Context prompt** (Settings → Transcription): tell the recogniser what you usually talk about so jargon and names are heard correctly. It leads the prompt, ahead of your dictionary terms.
- **Microphone level meter** (Settings → Recording): test the selected microphone with the same level curve as the pill. It closes itself after 60 seconds.
- **Back up and restore** your dictionary and per-app profiles as a single file. Importing merges, never deletes, and every entry is re-validated; bad entries are skipped and counted.
- **Said vs fixed** in History: see what you said next to what was delivered, with a reversible **Use original** that Copy and Export respect.
- **Smart grammar** (Settings → Output, off by default): an on-device model (T5-small, about 120 MB, downloaded once from the same screen) fixes agreement, tense and wrong word forms the rules cannot see — "she don't know what we was doing" becomes "she doesn't know what we were doing". Every proposed change is judged on its own and only safe word-level fixes are kept: names, numbers, code, acronyms and your dictionary words are never touched, a word is only swapped for another form of itself, and a rewrite that changes too much is discarded whole. Finished sentences are corrected while you are still talking, so the stop adds roughly a tenth of a second; past a time budget the rest is pasted as the rules wrote it. English only, skipped in the Literal style, and History shows every change.
- **Voice mode** (Settings → Voice, off by default): press the Speak shortcut (⌃⌥Space), say what you mean, press it again — Vocs fixes it up and SPEAKS it instead of pasting it, in your own Personal Voice if you have made one (macOS 14 and later), into the audio device you choose. Pick a virtual device such as BlackHole and choose it as the microphone in Zoom, Meet or Teams, and the meeting hears your corrected words in your voice. Fillers are always removed and self-corrections applied for speech; the voice is loaded while you talk, so the first sound takes about 0.4 s after you stop. A Test button reports exactly how long the first sound took, and Settings explains what to install when no virtual device is found.
- **Meeting safe mode** (⌃⌥M, the menu bar, or the Voice page): one switch that passes your real microphone straight through to the same device, on a path that shares nothing with speech. It also switches on by itself if speaking fails or takes too long to start (turn that off under Voice), and says why. A backlog is dropped rather than letting the meeting fall behind your lips.
- **Learn from my corrections** (Settings → Output, on): when Smart grammar fixes something and you leave it, Vocs remembers. A fix you have kept three times and never undone applies itself — instantly, without the model. "Use original", or editing the fix out, counts as a no and switches it off straight away. Everything learned is listed under Settings → Vocabulary → Learned corrections, with how often you kept or undid each one, where you can turn any of it on or off, delete it, or forget everything.
- **Fix common grammar slips** (Settings → Output, off by default): "a idea" → "an idea", "she don't" → "she doesn't", "we was" → "we were", "they has" → "they have", "their going to" → "they're going to", "its a" → "it's a", "could of been" → "could have been". English only; skips quoted speech and dictionary words.
- **Filler removal strength** (Light / Standard / Strict). Hesitations ("um", "uh") always go; ambiguous words ("like", "you know") only when punctuation marks them as filler.
- **Excluded apps**: turn Vocs off in chosen apps. Recording is refused before the microphone is touched.
- **Copy Last Transcript** in the menu bar, and an optional global shortcut (off by default).
- **Crash-safe recording**: the transcript is checkpointed as you speak, so a crash mid-dictation no longer loses it.
- **Free-disk check** before the model download.
- **Latency view by stage**: capture, recognition, correction, output, and stop-to-pasted.
- **Diagnostics export**: an opt-in report with home-directory paths and free-text settings redacted. Nothing is sent anywhere.

### Changed
- The window title is larger. The Support page's Buy Me a Coffee link goes to the project's page.
- **The recording pill is a horizontal capsule**: the mark beside three audio bars, with no timer. Live captions are gradient text that rises out of it instead of a box.
- **A window bar** puts the title (centred) and the toolbar buttons on the same line as the traffic lights, with a proper safe area. The side rail is a rounded pill with more room.
- **Keep history for** is a choice (forever, 1 week, 1 month, 3 months, 1 year) rather than a number of days; long text settings such as Context get the full width; a select shows what its chosen option means.
- The app icon has three small sound bars beside the head.
- **Settings is seven short pages** with a section list and search instead of one long scroll.
- **History is now called Dictations** and is the first page.
- The Stats page is a grid of numbered cards, and its activity chart is drawn over the real calendar, zero days included.
- Filler removal no longer deletes real words. "I like pizza" is left alone.
- A spoken "new line" at the very end of a dictation now keeps its trailing newline. In a single-line field this presses Enter.
- Emphatic repeats ("very very", "no no") are kept as spoken; other repeated words collapse.
- Sentence capitalisation no longer touches file names, domains or version numbers (`main.rs`, `example.com`, `2.5.1`).
- Quoted speech is never edited by the correction rules.
- Text settings now enforce their declared length limit.

### Fixed
- **A second launch no longer crashes the running app.** Double-clicking Vocs while it was already running asked AppKit to move a window from the wrong thread.
- **Recording a new shortcut no longer starts a dictation.** The old shortcut is released while Settings is listening, then the new one is registered.
- **A recording that heard nothing is not stored.** A tap of the hotkey or a silent microphone no longer leaves a blank entry in Dictations or a count in Stats; entries already stored that way are cleaned up on update.
- **The hotkey works while a failure is still on screen.** It used to be ignored for the three seconds the reason stayed up.
- **The first dictation after launch is as fast as the rest**: the model's short-fragment and language-detection paths are warmed at startup (the first one took 2.8 s, later ones about 0.5 s).
- **A single audio buffer underrun no longer ends a recording** as "the microphone disconnected". CoreAudio reports one under ordinary load, most often right after a stream starts.
- A crash when a dictionary term contained a character whose lowercase form has a different byte length (for example Turkish "İ").
- A dangling comma or lost sentence end after a trailing hesitation ("wait um…").
- Brackets: spacing inside them and a missing full stop after a closing bracket.

### Security
- Updated `rustls`, `ringbuf` and `quinn-proto` to patched releases.
