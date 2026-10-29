# Changelog

All notable changes to Vocs. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/).

## [1.1.0] - 2026-10-28

### Fixed
- **A fresh install no longer says "Vocs is still starting up" forever.** After the first-run model download the speech model was never loaded until Vocs was quit and reopened, so the setup's "Try it" step (and every hotkey press) was refused. The model now loads as soon as the download finishes, and pressing the hotkey on a model that is on disk but not loaded starts loading it instead of leaving you stuck. If you are on 1.0.2 and hit this, quit Vocs from the menu bar and open it again once.
- **"I spoke and nothing was pasted."** Stopping in a pause, while the last piece of speech was still being recognised, could finish the dictation before those words arrived and then drop them. Vocs now waits for everything you said to be recognised before it pastes, however you stop.
- **No more silently dropped speech on a slow Mac.** If recognition fell behind, a finished piece of speech could be thrown away because the queue was full. Vocs now holds it and recognises it as soon as there is room, in order.
- **The on-device grammar model can no longer make a paste slow.** Its time limit is now enforced inside each correction, not only between sentences; a sentence it has no time for goes out as the rules wrote it, in half a second at most.
- **Checking for updates is quicker and more dependable.** A check now gives up after ten seconds and says why instead of sitting on "Checking"; opening a window reuses a recent answer instead of asking GitHub again; installing starts from the update that was already found; and a check that fails at launch (the Mac was still joining wifi) is retried within minutes instead of tomorrow.
- **Installing an update shows real progress at the bottom of the window** and Vocs restarts by itself when it finishes.
- **Live captions no longer flicker through half-heard words.** The last word of a caption is shown once two snapshots agree on it, and the language is pinned from the first few seconds instead of being guessed again on every caption.

### Added
- **What's new.** After an update, the dashboard shows a short carousel of what the release added, once. Settings → Updates can open it again.
- **Language hotkeys** (Settings → Recording): up to two extra dictation shortcuts that each always listen for one language, so you never depend on auto-detection when you switch between languages. Off until you turn them on.
- **Voice commands that take words back.** Say "scratch that" to drop the sentence you just spoke, "undo last sentence" to drop the one before, or "scratch everything" to start the dictation again. English; part of Spoken formatting.
- **Dictation scratchpad.** A floating notepad (menu bar → Open Scratchpad, or an optional shortcut): ramble into it, press Return to have it turned into bullets, a task list or a clean paragraph, and ⌘Return to send the result to the app you came from.
- **Speed or accuracy** (Settings → Transcription): Fast is the default and unchanged; Balanced and Accurate trade a little time for fewer mistakes, live captions included.

## [1.0.2] - 2026-10-24

### Fixed
- **No more `*Scoffs*`, `[BLANK_AUDIO]` or `(laughs)` in your text.** Whisper sometimes writes a sound effect for a breath or a click; Vocs now removes those tags instead of pasting them. A stray noise with nothing else in it pastes nothing.
- **No more runaway repetition.** A recording could come back as the same phrase repeated dozens of times ("the one who is the one who is…"). Vocs now collapses the loop, retries the recording once, and discards it if it loops again, so silence no longer turns into a pasted phrase.
- **Pastes no longer land late, or paste your old clipboard.** The previous clipboard used to be put back a fraction of a second after pasting, which beat apps that read the clipboard slowly. It now comes back half a second later, in the background, so the paste is never held up for it. (Settings → Output → Clipboard restore delay.) A paste delay below 30 ms is raised to 30.

### Faster
- **Stop to paste is much quicker.** Every piece of speech is now decoded with a window sized to its length instead of the full 30 seconds (about 8 times faster on a short fragment), and the first piece is closed at the first pause after four seconds instead of eight. Stopping while a background piece was still decoding no longer makes the final words wait behind it.

### Changed
- **A tighter, livelier sidebar.** Smaller gaps and one clean centre line, and the logo now glances left and right.
- **The recording pill moves with your voice.** The three bars rise fast and fall slowly, and keep a gentle ripple between words.
- **Smoother navigation.** Pages fade in, lists dim while a filter loads instead of jumping, and pages centre on wide windows.
- **Page titles are truly centred**, and an open dictation's back, save status, copy and delete controls now sit directly above its title.
- **The website** works on phones without sideways scrolling, shows how to open the app in a dialog when you press Download, and presents a long release history as a searchable timeline.

## [1.0.1] - 2026-10-24

### Fixed
- **Opening Vocs now shows the window.** Double-clicking the app used to start it silently in the menu bar, so it looked as if nothing had happened and you had to find **Open Vocs** in the menu bar item. The dashboard now opens when you open the app, and again if you open it while it is already running. When Vocs starts by itself at login it still stays quiet in the menu bar. (An existing launch-at-login entry made before this fix shows the window at login until you switch **Launch at login** off and on once.)

## [1.0.0] - 2026-10-24

The first public release.

### Highlights
- **Talk, and it types.** Press ⌥Space anywhere (toggle it, or hold to talk), say it, press again. The text is pasted into whatever had focus and your clipboard is put back.
- **Everything stays on your Mac.** Recognition runs on-device with Whisper. No account, no subscription, no word cap, no telemetry.
- **Transcribes while you speak**, so finishing is as fast after a five-minute monologue as after five seconds.
- **99 languages**, auto-detected or pinned.
- **Quiet in meetings.** While you record, Vocs mutes your Mac's built-in speakers so the other people on a call are not typed out with your words, and restores them when you stop, even after a crash. Headphones and AirPods are left alone.
- **Dictations**, a notebook of everything you said: searchable, filterable, editable like a note (⌘F to find, ⌘S to save). **Said → fixed** shows what the rules changed, and **Use original** takes your wording instead.
- **Writing styles**: Natural, Casual, Formal or Literal.
- **Filler removal that keeps real words**, at three strengths. "I like pizza" is never touched.
- **Your dictionary and a context line** teach it the names and jargon it gets wrong.
- **Grammar, your way.** Quick rule-based fixes for slips like "a idea" and "she don't", and an optional on-device model (about 120 MB, downloaded once) for agreement and tense. It never rewrites you, and it learns the fixes you keep.
- **Preview before pasting** and **live captions**, both off by default.
- **Escape to cancel**, with a three-second countdown. Press it again and the recording resumes with nothing lost.
- **Crash-safe**: the transcript is saved as you speak.
- **Updates you control** (Settings → General → Updates): check any time, read what changed, and install with a progress bar. Every update is signature-verified and never installs mid-dictation.
- **Honest stats**, including the latency Vocs actually achieves at each stage.
- **Back up and restore** your dictionary, and **export diagnostics** as a plain-text file you send yourself.
- **A real menu bar**: start or stop dictating, copy the last transcript, and jump to any page.

### Notes
- Vocs is free and independent, so it is not notarised by Apple. macOS asks you to approve the first launch once: System Settings → Privacy & Security → **Open Anyway**.
- Requires macOS 13 or later, on Apple Silicon or Intel. Needs about 600 MB of disk for the speech model.
