## What and why

<!-- What changed, and why. The reason matters more than the diff. -->

## Checks

- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` (and `src/lib/bindings.ts` is committed if it changed)
- [ ] `pnpm typecheck` and `pnpm build`
- [ ] New files have a `SOURCE OF TRUTH KEYWORDS` header
- [ ] A user-visible change has a line under `Unreleased` in `CHANGELOG.md`
