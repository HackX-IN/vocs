# Vocs — Release Sign-off Checklist

Everything that needs a person, a real Mac or a real app before a tag is cut.
Automated gates (clippy, tests, `tsc`, build, bindings drift) run in CI and are
not repeated here. Tick a box only after doing it on the **built, signed DMG**,
not a dev build — permissions and signing behave differently.

**Machine for every run:** note macOS version and chip here: ____________

## 1. Clean install (a Mac that has never had Vocs)

- [ ] Open the DMG, drag to Applications, launch. Gatekeeper blocks it (no notarisation, by decision); **Open Anyway** in System Settings → Privacy & Security lets it run, and it never asks again.
- [ ] Onboarding opens. Microphone prompt appears once; granting it works.
- [ ] Accessibility step deep-links to the right System Settings pane; granting it enables pasting.
- [ ] Model step: the download completes with progress shown. Turn Wi-Fi off mid-download, turn it back on: it resumes rather than restarting. Quit mid-download and relaunch: it resumes.
- [ ] Hotkey step: the test dictation produces text.
- [ ] Quit, relaunch: no onboarding again, hotkey still works, permissions still held.

## 2. Dictation in real apps

For each, dictate a short sentence and a 5-minute passage; text lands in the right place.

- [ ] Microsoft Teams chat box
- [ ] ClickUp (task comment and description)
- [ ] Slack · Chrome text field · Notes · Terminal · VS Code
- [ ] A secure/password field: nothing is pasted, nothing is stored
- [ ] Say "new line" at the end of a sentence in a multi-line field (newline) and a single-line field (Enter is pressed — known, documented behaviour)
- [ ] Fillers: "um, I like pizza" → "I like pizza". "it was, like, fine" at Standard → "it was, fine"
- [ ] A dictionary term is spelled as saved; the context prompt changes a hard-to-hear word

## 3. New features, by hand

- [ ] Settings → Recording: mic meter moves with your voice, stops by itself after 60 s, and refuses to start while dictating
- [ ] Switch microphones (AirPods ↔ built-in) with the meter running
- [ ] History: open an entry that was changed; the diff matches what you said; **Use original** flips Copy and Export; flipping back restores
- [ ] Backup: export, wipe the dictionary, import (entries return); import a hand-corrupted file (clear error, nothing lost)
- [ ] Excluded app: turn Vocs off in an app; the hotkey does nothing there and does not open the microphone (orange dot never appears)
- [ ] Copy Last Transcript from the menu bar; and the optional shortcut once enabled (default ⌃⌥C); it does nothing when there is no history
- [ ] Diagnostics export: open the file; no home path, no dictation text, no context prompt text

## 4. Failure paths

- [ ] Unplug the microphone mid-recording: clear message, no hang, history intact
- [ ] Revoke Microphone in System Settings while running: clear recovery message
- [ ] Revoke Accessibility: text goes to the clipboard instead, with an explanation
- [ ] Force-quit (`kill -9`) mid-dictation: relaunch shows the partial transcript in History
- [ ] Sleep the Mac mid-session and wake it: next dictation works
- [ ] Fill the disk below the model size: the download refuses with a clear message

## 5. Updates

- [ ] Install `v1.0.0-rc1`, publish `rc2`; **Settings → General → Updates → Check for updates** finds it, shows the release notes, installs it with a progress bar, relaunches, signature verified
- [ ] With Wi-Fi off, **Check for updates** says Vocs could not reach the update server (and does not say "up to date"); back online it finds the update
- [ ] With **Check for updates** turned off in Settings, the card says checks are turned off
- [ ] Pressing **Install and restart** mid-dictation is refused with a clear message
- [ ] After the update, permissions are still granted (the signing identity did not change) and Gatekeeper does not block the updated app

## 6. Soak and performance

- [ ] About 50 hours of normal use, no crash and no lost transcript
- [ ] Settings → latency view: *Correction* p95 under 50 ms; *Stop to pasted* feels instant
- [ ] Idle with the app running: CPU near zero, memory flat over a day
- [ ] Intel Mac / macOS 13, if that floor is kept: dictation works and latency is acceptable

## 7. Accessibility

- [ ] Whole app reachable by keyboard alone
- [ ] VoiceOver reads the settings controls, the pill state and the History diff sensibly

## Sign-off

| Gate | Result | By | Date |
|---|---|---|---|
| Automated (CI green on `main`) | | | |
| Sections 1–5 | | | |
| Section 6 | | | |
| Section 7 | | | |

## Cutting a release

1. Make the three version numbers equal: `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `package.json` (then `cargo check` so `Cargo.lock` follows). The tag must be `v` plus that number.
2. In `CHANGELOG.md`, rename `[Unreleased]` to `[x.y.z] - YYYY-MM-DD` and open a fresh `[Unreleased]` above it. The release workflow publishes that section as the release notes, which are also what the in-app Updates card shows, so trim it to what a user should read.
3. Push the tag: `git tag vX.Y.Z && git push origin vX.Y.Z`. The Release workflow builds, signs and creates a **draft** release.
4. Download the DMG from the draft, open it and launch it once (sections 1–2 above).
5. **Publish** the draft. This is the moment updates go live: `latest.json` only resolves once the release is published, and the website's download button picks the new DMG up by itself.
6. On an older install, **Check for updates** finds it.

