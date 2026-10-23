# Third-party notices

Vocs is MIT-licensed ([LICENSE](LICENSE)). It is built on the work below.
Nothing here is bundled in the app except the code; the two models are
downloaded to the user's Mac, from their publishers, on first use.

## Models (downloaded at runtime)

| Model | Used for | Source | Licence |
|---|---|---|---|
| Whisper (`ggml-large-v3-turbo-q5_0`, `ggml-small-q5_1`) | Speech recognition | OpenAI Whisper weights, converted to ggml by the [whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp) repository | MIT |
| T5-small grammar correction | Optional on-device grammar fixes | [AventIQ-AI/T5-small-grammar-correction](https://huggingface.co/AventIQ-AI/T5-small-grammar-correction), pinned to a commit and verified against a SHA-256 | **No licence declared** on the model card. It is fine-tuned from `t5-small` (Apache-2.0). |

Vocs does not host or redistribute either model. It downloads each file
directly from Hugging Face, and the grammar model only if the user turns Smart
grammar on.

## Engine

| Component | Licence |
|---|---|
| [whisper.cpp](https://github.com/ggml-org/whisper.cpp) and ggml, built through the `whisper-rs` crate | MIT |

## Fonts

| Font | Licence |
|---|---|
| [Doto](https://fonts.google.com/specimen/Doto) via `@fontsource-variable/doto` | SIL Open Font License 1.1 |

## Application framework

[Tauri](https://tauri.app) 2 and its plugins (updater, global shortcut,
autostart, process, dialog, opener) are licensed MIT OR Apache-2.0.

## Dependencies

The complete, exact list is `src-tauri/Cargo.lock` (Rust) and `pnpm-lock.yaml`
(JavaScript). Every Rust crate in the build declares one of the following
licences, and for crates offering a choice (for example `MIT OR Apache-2.0`)
Vocs uses them under the MIT option where one is offered:

- MIT, Apache-2.0, and combinations of the two (the great majority)
- BSD-2-Clause, BSD-3-Clause, ISC, Zlib, 0BSD, Unlicense, BSL-1.0
- Unicode-3.0 (Unicode data tables)
- MPL-2.0, for `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext` and
  `selectors`. These are used unmodified as dependencies; MPL-2.0 is a
  file-level licence, and their source is available from their crates.io pages.

The JavaScript runtime dependencies (React, Framer Motion, Zustand, Zod,
React Hook Form, Tailwind CSS, lucide-react, class-variance-authority, clsx and
tailwind-merge, among others) are MIT or ISC licensed, except where a package
states otherwise in its own `package.json`.

To regenerate the Rust list yourself:

```bash
cargo metadata --manifest-path src-tauri/Cargo.toml --format-version 1 --locked
```

and read the `license` field of each package.
