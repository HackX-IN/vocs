# Changelog

All notable changes to Vocs. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **A new look.** A warm paper light theme and an ember dark theme, the Doto dot-matrix face for titles and figures, numbered instrument cards and dot-matrix charts. New app icon (a head whispering quotes) and matching menu-bar glyph.
- **Dictations**, the new home of the app: your history as a notebook grouped by day, with a summary strip and a floating search bar. Open a dictation to edit it — it has a title you can change and text that saves itself — while the **Said → fixed** comparison stays beside it.
- **Writing style** (Settings → Output, and per app): Natural, Casual (contractions stay, a one-line message ends without a full stop), Formal (contractions written out, "gonna" becomes "going to") and Literal (exactly what was heard, for terminals and code editors). Vocs suggests a style when you add an app such as Slack, Mail or Terminal.
- **Preview before pasting** (Settings → Output, off by default): the finished text is shown for a moment and Escape keeps it out of the document. It is still saved in Dictations.
- **Live captions** (Settings → Recording, off by default): a small window shows what Vocs hears, and what it will write, while you speak.
- **Context prompt** (Settings → Transcription): tell the recogniser what you usually talk about so jargon and names are heard correctly. It leads the prompt, ahead of your dictionary terms.
- **Microphone level meter** (Settings → Recording): test the selected microphone with the same level curve as the pill. It closes itself after 60 seconds.
- **Back up and restore** your dictionary and per-app profiles as a single file. Importing merges, never deletes, and every entry is re-validated; bad entries are skipped and counted.
- **Said vs fixed** in History: see what you said next to what was delivered, with a reversible **Use original** that Copy and Export respect.
- **Fix common grammar slips** (Settings → Output, off by default): "a idea" → "an idea", "she don't" → "she doesn't", "we was" → "we were", "they has" → "they have", "their going to" → "they're going to", "its a" → "it's a", "could of been" → "could have been". English only; skips quoted speech and dictionary words.
- **Filler removal strength** (Light / Standard / Strict). Hesitations ("um", "uh") always go; ambiguous words ("like", "you know") only when punctuation marks them as filler.
- **Excluded apps**: turn Vocs off in chosen apps. Recording is refused before the microphone is touched.
- **Copy Last Transcript** in the menu bar, and an optional global shortcut (off by default).
- **Crash-safe recording**: the transcript is checkpointed as you speak, so a crash mid-dictation no longer loses it.
- **Free-disk check** before the model download.
- **Latency view by stage**: capture, recognition, correction, output, and stop-to-pasted.
- **Diagnostics export**: an opt-in report with home-directory paths and free-text settings redacted. Nothing is sent anywhere.

### Changed
- **The recording pill is now a vertical capsule**: the mark above three audio bars. It no longer shows a timer.
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
- **A single audio buffer underrun no longer ends a recording** as "the microphone disconnected". CoreAudio reports one under ordinary load, most often right after a stream starts.
- A crash when a dictionary term contained a character whose lowercase form has a different byte length (for example Turkish "İ").
- A dangling comma or lost sentence end after a trailing hesitation ("wait um…").
- Brackets: spacing inside them and a missing full stop after a closing bracket.

### Security
- Updated `rustls`, `ringbuf` and `quinn-proto` to patched releases.
