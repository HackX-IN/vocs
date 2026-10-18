# Changelog

All notable changes to Vocs. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **Context prompt** (Settings → Transcription): tell the recogniser what you usually talk about so jargon and names are heard correctly. It leads the prompt, ahead of your dictionary terms.
- **Microphone level meter** (Settings → Recording): test the selected microphone with the same level curve as the pill. It closes itself after 60 seconds.
- **Back up and restore** your dictionary and per-app profiles as a single file. Importing merges, never deletes, and every entry is re-validated; bad entries are skipped and counted.
- **Said vs fixed** in History: see what you said next to what was delivered, with a reversible **Use original** that Copy and Export respect.
- **Filler removal strength** (Light / Standard / Strict). Hesitations ("um", "uh") always go; ambiguous words ("like", "you know") only when punctuation marks them as filler.
- **Excluded apps**: turn Vocs off in chosen apps. Recording is refused before the microphone is touched.
- **Copy Last Transcript** in the menu bar, and an optional global shortcut (off by default).
- **Crash-safe recording**: the transcript is checkpointed as you speak, so a crash mid-dictation no longer loses it.
- **Free-disk check** before the model download.
- **Latency view by stage**: capture, recognition, correction, output, and stop-to-pasted.
- **Diagnostics export**: an opt-in report with home-directory paths and free-text settings redacted. Nothing is sent anywhere.

### Changed
- Filler removal no longer deletes real words. "I like pizza" is left alone.
- A spoken "new line" at the very end of a dictation now keeps its trailing newline. In a single-line field this presses Enter.
- Emphatic repeats ("very very", "no no") are kept as spoken; other repeated words collapse.
- Sentence capitalisation no longer touches file names, domains or version numbers (`main.rs`, `example.com`, `2.5.1`).
- Quoted speech is never edited by the correction rules.
- Text settings now enforce their declared length limit.

### Fixed
- A crash when a dictionary term contained a character whose lowercase form has a different byte length (for example Turkish "İ").
- A dangling comma or lost sentence end after a trailing hesitation ("wait um…").
- Brackets: spacing inside them and a missing full stop after a closing bracket.

### Security
- Updated `rustls`, `ringbuf` and `quinn-proto` to patched releases.
