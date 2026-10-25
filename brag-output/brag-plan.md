# Vocs — brag plan

## Answers

- **What is it?** Free, open-source speech-to-text for macOS: press ⌥Space, talk, press it again, and your words are pasted where you were typing.
- **Who is it for?** Anyone who thinks faster than they type, especially people writing long prompts to AI agents, in any app.
- **What sets it apart?** It runs entirely on the Mac. No account, no subscription, no word cap, no telemetry. It transcribes while you speak, so a five-minute monologue finishes as fast as a sentence.
- **Most impressive claim:** "Nothing leaves your Mac." Second: "Escape takes it back" (a three-second countdown, and a second Escape resumes with nothing lost).
- **Visual hook:** a slow typed sentence ("You think faster than you type.") bursts into the brand's dot-matrix dots when ⌥Space is pressed, and the dots collapse into the pill.
- **Real UI to show:** the pill (104×52 capsule, 2px ring, the mark breathing, three bars), the live caption rising out of it as a gradient, the countdown line, the app icon, the Doto display face, and the "New message / To: Design team" hero mock from the site.
- **Tone:** `polished` energy with `cinematic` scale. The look is a showreel: ember dark, one paper-light scene, and an accent-orange finale.
- **Share caption:** see `share-copy.txt`.

## Visual identity (from `src/styles/tokens.css`, `site/tokens.css`)

- Ember `#13110F` page with the top-left `--page-glow` bloom; paper `#F4F3EE`; cards `#FFFFFF` / `#1F1D1A`.
- One accent: `#FF6A2B` (dark) / `#E8501A` (light). Pill: `#1D1D1D` surface, `#3A3A3A` ring, `#E6E3DC` ink, `#9D4E20` bars.
- Doto (bundled, OFL) is used only for display figures. UI text uses Inter as the SF stand-in (the site's own fallback). Eyebrows use mono capitals: "01 · HOW IT WORKS".
- Dot lattice (`--dot-grid-pitch` 14px) is the recurring texture.
- Springs follow the app tokens (300/25 for state changes, 260/26 for panels).

## Music

Original score, synthesized for this cut. 120 BPM, so 1 beat = 0.5s = 15 frames, and every cut lands on a beat counted from frame 0. Key of F minor (Fm – D♭ – A♭ – E♭). Sound effects (key taps, the ⌥Space press, whooshes, chimes) are tuned to the same key and mixed under the music.

## Storyboard (25.0s, 30fps, 1920×1080)

| # | Time | Beats | Scene | On screen | Motion / SFX |
|---|---|---|---|---|---|
| 1 | 0.0–3.0 | 0–6 | **Hook** | "You think faster than you type." is typed slowly with a caret on ember. Two keycaps, ⌥ and Space, rise and press. | Soft key taps. On the press (2.5s) the sentence bursts into dots that get sucked to centre. Sub-drop, then the beat comes in. |
| 2 | 3.0–5.5 | 6–11 | **Reveal** | The pill appears instantly, as it does in the app, with a ring flash. It slides left, and "Vocs" lights up dot by dot in Doto, with "Talk. It types." under it. | The bars run on a fake level signal synced to the music. The mark breathes. |
| 3 | 5.5–11.0 | 11–22 | **The product in use** | A paper Mail compose window ("New message · To: Design team") sits over ember. The pill is at the bottom centre. The live caption rises out of the pill word by word: "Push the launch to Thursday. Design needs one more pass on the onboarding screens." On ⌥Space the pill exits upward and the text pastes into the message body. Corner eyebrows count 01 · PRESS ⌥SPACE → 02 · SAY IT → 03 · PRESS IT AGAIN. | Caption gradient (accent to ink). A paste sweep highlight, and a soft chime on the paste. |
| 4 | 11.0–14.0 | 22–28 | **Everywhere** | Headline "Works wherever you can type." A rapid 3D card deck of generic app windows (Slack, VS Code, Terminal, Notes), one per beat, each receiving a pasted line. | A whoosh per card on the beat. |
| 5a | 14.0–16.0 | 28–32 | **99 languages** | "99" in Doto, with greetings cycling on every beat (Hindi, Arabic, Japanese, Spanish, Korean, French). | Ticks on the beat. |
| 5b | 16.0–18.5 | 32–37 | **Escape takes it back** | An Esc keycap press: the pill's bars become the countdown line, draining linearly. A second Esc springs it back to full, and the bars return. "Escape takes it back." | A tick on each Esc, and a riser that reverses on the refill. |
| 6 | 18.5–21.5 | 37–43 | **Privacy** | A paper-light wipe. Chips slam in on beats: No account · No subscription · No word cap · No telemetry. Then "Nothing leaves your Mac." in Doto. | Hits on the beats, and the chips stack with springs. |
| 7 | 21.5–25.0 | 43–50 | **Outro** | An accent-orange flood. The app icon springs in, then "Vocs", "Talk. It types.", "Free · Open source · macOS", and "hackx-in.github.io/vocs". | The final hit on 22.0, then the sound rings out. |

Durations: 3.0 + 2.5 + 5.5 + 3.0 + 2.0 + 2.5 + 3.0 + 3.5 = **25.0s**.

## Notes

- All claims come from the README or site copy. No numbers or testimonials are invented.
- App names in scene 4 are shown as plain text labels on generic windows. No third-party logos are used.
- The demo message is the site's own hero mock copy. No real names or data are used.
