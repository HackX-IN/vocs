# Contributing to Vocs

Thanks for wanting to help. Vocs is a small, local-first dictation app for
macOS, and it stays small by being strict about how it is built. This page is
the short version of that strictness. Bug reports, ideas and pull requests are
all welcome.

By taking part you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Before you start

- **Found a bug?** Open an issue with the bug template. If it involves a paste
  that went nowhere or a wrong transcript, attach **Settings → Your data →
  Export diagnostics**. The file has no dictation text and no home path in it,
  and you send it yourself.
- **Want a feature?** Open an issue first and say what you are trying to do. A
  feature that fights the product's promises (no account, nothing leaves the
  Mac, no telemetry) will not be merged however well it is written, and it is
  kinder to find that out before you spend a weekend on it.
- **Found a security problem?** Do not open an issue. See [SECURITY.md](SECURITY.md).

## Set up

You need macOS 13 or later (the crate links CoreAudio, AppKit and Carbon and does
not build anywhere else), plus:

- Rust (stable) with `rustup`
- Node 20 and [pnpm](https://pnpm.io) 10
- Xcode command line tools: `xcode-select --install`
- CMake: `brew install cmake` (whisper.cpp builds from source)

```bash
git clone https://github.com/HackX-IN/vocs.git
cd vocs
pnpm install
pnpm tauri dev
```

The first Rust build compiles whisper.cpp and takes a few minutes. After that
it is incremental. The first launch walks you through the microphone
permission and the model download.

Building a bundle needs no secrets: `pnpm tauri build --bundles app`.
Only a release is signed; see the README's "Building a release".

## Finding your way around

There are two things to learn, and both are quick.

**Read `docs/00-START-HERE.md`.** The `docs/` set explains what the app is and
why it is shaped this way. Skim `docs/06-CONVENTIONS-AND-GREP.md` before writing
your first file.

**Search by keyword, not by reading the tree.** Every file and every non-obvious
block opens with a `SOURCE OF TRUTH KEYWORDS` line. Two read-only commands query
them:

```bash
pnpm sot SessionState        # files whose keywords mention it
pnpm sot:show AudioChunk     # the same, printing each file's header
```

Search before you create a type, function or component. Most of what you need
already exists, and a duplicate is the most common reason a PR is sent back.

## The rules that matter

[`CLAUDE.md`](CLAUDE.md) is the full rulebook. It is written for AI coding
agents, so it is terse. The rules a human contributor will meet:

1. **Layers point downward only.** Registry → command factory → commands and
   pipeline → ports → adapters → services. An upward import fails the build, by
   test, not by code review.
2. **Features are registry entries.** A setting, permission, hotkey, nav item or
   metric is one entry in `src-tauri/src/registry/`. The settings UI is
   generated from it. If you are writing a `match` on a feature name outside
   the registry, the branch belongs in the registry.
3. **Every IPC command goes through the factory** (`src-tauri/src/ipc/factory.rs`),
   which already does validation, permission checks, reentrancy guarding,
   tracing and error mapping. Never write `#[tauri::command]` on its own.
4. **No `unwrap()`, `expect()`, `panic!`, `any` or `@ts-ignore`** outside tests.
   Clippy runs at deny level and TypeScript is strict.
5. **Types that cross to TypeScript are generated.** `src/lib/bindings.ts` is
   written by `cargo test` through specta. Never edit it by hand.
6. **No hardcoded theme values.** No hex, no `text-white`, no improvised radius,
   duration or spring. Use the tokens in `docs/04-DESIGN-SYSTEM.md`, and add a
   value there first if you need a new one.
7. **Reuse a component before writing one.** Reusable ones live in
   `src/components/global/`; a route's own live in its `_components/`.
8. **One file, one responsibility.** Past roughly 400 lines it is doing two
   things.
9. **Give new files a header** with a `SOURCE OF TRUTH KEYWORDS` line (five to
   ten specific keywords) and WHAT / WHY / WHERE. WHY is the one that matters:
   say what forced the choice. A file without a header is invisible to the next
   person.
10. **Validate input at the boundary.** Rust commands declare a schema to the
    factory; forms use Zod with React Hook Form.

## Checks to run before you push

CI runs the same four gates. Run them locally so it is green the first time:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test   --manifest-path src-tauri/Cargo.toml     # also regenerates bindings
pnpm typecheck
pnpm build
git diff --exit-code -- src/lib/bindings.ts           # bindings must be committed
```

`cargo test` regenerates `src/lib/bindings.ts`. If you changed a command or
event, commit that file with your change; CI fails if it drifts.

Engine tests that need the 574 MB speech model skip themselves when it is
absent, so a clean checkout passes without it.

## Pull requests

- Branch from `main`. Keep a PR to one idea; two unrelated fixes are two PRs.
- Explain **why** in the description, not only what changed. If you changed
  behaviour a user can see, say so and add a line under `Unreleased` in
  [CHANGELOG.md](CHANGELOG.md), written for the person using the app.
- Add or update a test for behaviour you changed. The suite already encodes the
  architecture, so a failing layering or reachability test is telling you
  something real.
- Do not commit scripts, seed files or probe files. If you need a one-off to
  investigate, keep it out of the tree.
- Commit messages use the `type(scope): summary` form seen in `git log`
  (`feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `ci`).

## Licence

By contributing you agree that your work is released under the project's
[MIT licence](LICENSE).
