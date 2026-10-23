# Security policy

## Reporting a vulnerability

Please **do not open a public issue** for a security problem.

Report it privately through GitHub:
**[Report a vulnerability](https://github.com/HackX-IN/vocs/security/advisories/new)**
(the repository's Security tab → *Report a vulnerability*).

Include what you found, the version of Vocs and macOS, and the steps to
reproduce it. A proof of concept helps but is not required.

We aim to acknowledge a report within a few days and to tell you what we plan to
do about it within two weeks. Vocs is maintained by a small team, so please be
patient, and we will credit you in the release notes if you would like that.

## What is in scope

- The app and its update path: the updater signature check, the release
  workflow, and anything that could let a build be replaced or tampered with
- Anything that makes Vocs send audio, transcripts or settings off the Mac
- Reading or writing files outside the app's own data directory
- Pasting or keystroke injection reaching somewhere the user did not intend
- The local database, the diagnostics export (it must never contain dictation
  text or the home path), and backup import

## What Vocs does and does not do

This is the context a report is judged against:

- **Audio never leaves the machine.** Recognition runs on-device.
- **The only network requests** are the model download (from Hugging Face, over
  HTTPS, verified against a pinned SHA-256) and the optional update check
  against GitHub Releases. Both are visible, and the update check can be turned
  off. There is no telemetry.
- **Updates are signed.** The app refuses any bundle that does not verify
  against the minisign public key compiled into it.
- **Vocs is not notarised by Apple.** Builds are signed with a self-signed
  certificate, so macOS asks the user to approve the first launch. That is a
  known, documented trade-off, not a vulnerability.

## Supported versions

Only the latest release receives fixes. Updating is one click under
**Settings → Updates**.
