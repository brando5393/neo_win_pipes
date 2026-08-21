# CLAUDE.md

Guidance for Claude Code (or any AI assistant) working in this repository.

## What this project is

A cross-platform (Windows/macOS/Linux) recreation of the classic Windows
"3D Pipes" screensaver, in Rust. End goal: a genuinely installable,
OS-selectable screensaver package on all three platforms — not just a
fullscreen demo app. Full context: `docs/RESEARCH.md` (what the original
did and why), `docs/ARCHITECTURE.md` (how this codebase is structured),
`docs/ROADMAP.md` (what phase we're in).

**Current phase**: Phase 2.5 — windowed `wgpu` rendering (`pipes-render`,
shared by `pipes-app` and `pipes-settings`) plus a settings app
(`pipes-settings`) with a live preview and an egui drawer, backed by a
persisted `AppConfig`. No native screensaver packaging or installers yet.
Check `docs/ROADMAP.md` before assuming Phase 3/4 (native
`.scr`/`.saver`/xscreensaver wrappers, installers) exist.

## Commands

```sh
cargo build --workspace
cargo test --workspace              # must pass before any change is done — 36 tests as of Phase 2.5
cargo run -p pipes-app -- --seed 1  # the screensaver
cargo run -p pipes-settings         # live preview + settings drawer
RUST_LOG=debug cargo run -p pipes-app -- --seed 1
```

If linking fails on Windows-on-ARM64, see the caveat in
`docs/DEVELOPMENT.md` before assuming the code is broken — it's a known
local-toolchain issue, not a bug in this repo.

## Non-negotiable conventions for this repo

These were established explicitly at project start and apply to every
change, not just the initial scaffold:

1. **Every change to simulation logic (`pipes-core`) ships with tests in
   the same commit.** Not a follow-up, not "add tests later." This is the
   project's founding requirement — see `docs/DEVELOPMENT.md#testing-philosophy`.
2. **`pipes-core` stays free of rendering/windowing dependencies**, and
   `pipes-render` stays free of `pipes-app`/`pipes-settings`-specific
   window/event-loop code. If something can only be verified by looking at
   a window, it belongs in one of the two binary crates, not `pipes-core`
   or `pipes-render`. This is what keeps the simulation testable headlessly
   in CI and keeps the two apps from duplicating rendering/config code.
3. **Determinism is load-bearing, not incidental.** `Scene`/`Pipe` take an
   explicit seeded RNG; never reach for thread-local/OS randomness inside
   `pipes-core`. If you touch RNG call order, expect the
   `same_seed_produces_identical_*` tests to need updating — treat that as
   a real behavior change to review, not noise to silence.
4. **New logged events get a row in `docs/LOGGING.md` in the same
   change.** Logging here is a human-readable design surface (see that
   doc's rationale), not an afterthought.
5. **Keep `docs/ROADMAP.md` checkboxes truthful.** If you ship part of a
   phase, check it off there in the same change.
6. Prefer property-based test assertions ("never re-enters an occupied
   cell") over one golden output value, except where reproducibility
   itself is the property under test.

## Where to look

| Question | Doc |
|---|---|
| What does the original screensaver actually do? | `docs/RESEARCH.md` |
| What features have users of similar projects actually asked for? | `docs/FEATURE_IDEAS.md` |
| How is this codebase organized, and why? | `docs/ARCHITECTURE.md` |
| What's built vs. planned? | `docs/ROADMAP.md` |
| How do I build/test/contribute? | `docs/DEVELOPMENT.md` |
| How do I run what exists today? | `docs/USAGE.md` |
| What do the logs mean? | `docs/LOGGING.md` |
