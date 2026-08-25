# CLAUDE.md

Guidance for Claude Code (or any AI assistant) working in this repository.

## What this project is

A cross-platform (Windows/macOS/Linux) recreation of the classic Windows
"3D Pipes" screensaver, in Rust. End goal: a genuinely installable,
OS-selectable screensaver package on all three platforms — not just a
fullscreen demo app. Full context: `docs/RESEARCH.md` (what the original
did and why), `docs/ARCHITECTURE.md` (how this codebase is structured),
`docs/ROADMAP.md` (what phase we're in).

**Current phase**: Phase 4. Windows is fully there:
`neo_win_pipes.msi` (`installer/main.wxs`, built with WiX v7) is a real,
double-click installer: `pipes-app.exe` → `System32\neo_win_pipes.scr`,
`pipes-settings.exe` → Program Files + Start Menu shortcuts (including
uninstall). `pipes-settings::update` checks GitHub Releases in the
background and offers a one-click update. On top of Phase 3's `.scr`
contract (`/s`/`/c`/`/p <hwnd>` — see `screensaver_args.rs`/`winsaver.rs`)
and Phase 2.5's `pipes-render` + `pipes-settings`.

**Linux is verified on real hardware.** `pipes-xscreensaver` renders
(real X11 window/display resolution via `x11-dl`, reusing
`pipes-render::Renderer` through a raw-window-handle bridge — see
`x11_target.rs`), and `installer/linux/` builds a real `.deb` (the hack +
`pipes-settings`, installed via `xscreensaver`'s own config-XML
convention) plus a `pipes-settings`-only AppImage. Beyond compiling and
passing clippy on `ubuntu-latest` CI, this has now actually been watched
rendering on a real machine (NVIDIA RTX 3060 Ti, wgpu Vulkan backend):
into a bare X server's root window, and into the window a live
`xscreensaver` 6.08 daemon hands it. That second case needed a real fix —
the daemon passes no CLI args at all, only the `XSCREENSAVER_WINDOW`
env var (`args::resolve_target_window` now reads both, argv taking
precedence — see `docs/ROADMAP.md` for the full writeup). Still no macOS
verification story, and no one's yet confirmed the packages install
cleanly from a stock desktop's package manager end to end, but the
core "does it render" question — this project's previous single
biggest open assumption on Linux — is closed.

**macOS is still design-only** — no code at all, no `.saver` bundle, not
even argument parsing, because there's no way to compile Objective-C/Swift
or link a Mach-O binary from this project's Windows dev machine at all
(unlike Linux, which at least cross-compiles/type-checks here). See
`docs/ROADMAP.md` before claiming otherwise about either platform.

## Commands

```sh
cargo build --workspace
cargo test --workspace              # must pass before any change is done — 123 tests as of v0.4.5 (Phase 4, Windows + Linux)
cargo run -p pipes-app -- --seed 1  # the screensaver, dev mode (behaves like /s)
cargo run -p pipes-app -- /s        # exercise the real Windows contract directly
cargo run -p pipes-app -- /c        # launches pipes-settings, exits
cargo run -p pipes-settings         # live preview + settings drawer + update checker
RUST_LOG=debug cargo run -p pipes-app -- --seed 1
```

Cutting a release: `git tag vX.Y.Z && git push origin vX.Y.Z` — see
`docs/DEVELOPMENT.md#cutting-a-release`. Don't hand-edit
`Cargo.toml`'s `[workspace.package]` version for a release; the release
workflow derives it from the tag.

Building the `.msi` needs WiX v7 (a per-user `dotnet` tool, plus a
one-time EULA acceptance — see
`docs/DEVELOPMENT.md#building-the-windows-installer-msi`). **Never accept
a tool's EULA/licensing terms on the user's behalf without asking first**
— this came up for real with WiX v7's Open Source Maintenance Fee terms;
free for this project, but still a legal acceptance to check with the
user about, not something to click through silently.

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
7. **Never ship unverifiable platform-specific code as if it were
   tested.** If you can't compile or run something on the machine you're
   working on (e.g. macOS/Linux-specific FFI from a Windows box), split
   the work: write and test the OS-independent pure-logic part for real
   (argument parsing, etc. — see `pipes-xscreensaver::args`), and
   document the untestable native part as an explicit plan/TODO rather
   than guessed code. Say so plainly rather than implying it's done.
8. **Actually run what you build before calling it done.** Two real bugs
   in this repo were only caught by launching the app and looking/testing
   directly, not by reading the code: `egui::CentralPanel`'s opaque
   default background hiding the 3D preview, and `/s` mode exiting within
   milliseconds because window creation itself fires a synthetic
   `CursorMoved`. Compiling and passing `cargo test` is necessary, not
   sufficient, for anything involving a window, an event loop, or Win32.
9. **Check whether `site/` (the neowinpipes.com splash page) needs
   updating** whenever a change is genuinely user-facing — a new feature,
   a platform going from unverified to confirmed-working, a new install
   method. It's a hand-maintained static site, not generated from these
   docs or the wiki, so nothing keeps it in sync automatically. Use
   judgment (an internal refactor doesn't need this); the point is to
   check, not to update it reflexively for everything.
10. **Update the relevant `docs/*.md` files and the GitHub wiki in the
    same change as any genuinely user-facing or architecture-affecting
    change** — a new feature, a changed default, a new security-relevant
    behavior, a platform status change. Not "consider whether to" like
    the `site/` guidance above (that one bites for cosmetic/marketing
    content where judgment genuinely varies) — this one is a
    requirement, the same weight as the other non-negotiable items on
    this list, because nothing else keeps `docs/`, the wiki (cloned from
    `https://github.com/brando5393/neo_win_pipes.wiki.git`, not part of
    this repo's own git history), and reality in sync automatically. A
    change that's purely internal (a refactor with no behavior change,
    a test-only commit) doesn't need this; if in doubt, treat it as
    user-facing. This was written down explicitly after a session where
    several real features shipped (the color-palette hex/RGB input, the
    installer artwork fix, the update-notification/checksum work,
    `SECURITY.md`) before the wiki and some `docs/` pages had been
    touched at all — catching up after the fact instead of doing it in
    the same change it belonged in.

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
| What does the public splash site look like/say? | `site/` (deployed at neowinpipes.com) |
