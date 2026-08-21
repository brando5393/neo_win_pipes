# Roadmap

Phased so that every phase leaves the repo in a state with passing tests,
current docs, and working (if limited) software — never a half-wired
feature branch.

## Phase 1 — Simulation core (in progress)

- [x] `pipes-core`: grid, direction, pipe growth/turning/termination,
      scene lifecycle, seeded determinism.
- [x] Unit test suite for all of the above (22 tests as of this writing).
- [x] `pipes-app`: headless CLI runner with human-readable `tracing` logs.
- [x] Repo scaffold: `CLAUDE.md`, docs set, MIT license, `.gitignore`.
- [ ] CI (GitHub Actions): build + test on `windows-latest`,
      `macos-latest`, `ubuntu-latest` for every push/PR.

## Phase 2 — Rendering

- [ ] `pipes-app` grows a `winit` window + `wgpu` renderer.
- [ ] Geometry generation: turn a `Pipe`'s path + joints into cylinder /
      sphere / torus meshes (pure functions in `pipes-core` or a small
      `pipes-geometry` crate, still unit-testable without a GPU: assert on
      vertex/index counts and bounds, not pixels).
- [ ] Chrome-like material (specular + simple reflection or an env map),
      slowly drifting camera, per [RESEARCH.md](RESEARCH.md).
- [ ] Manual visual verification against the research doc's description of
      the original (screenshots checked into `docs/screenshots/` for
      regression reference).

## Phase 3 — Native screensaver wrappers

- [ ] Windows: `.scr` wrapper (`/s`, `/c`, `/p <HWND>` contract).
- [ ] macOS: `.saver` bundle (`ScreenSaverView`).
- [ ] Linux: `xscreensaver` hack executable + `.desktop`/config
      registration.
- [ ] Each wrapper gets its own smoke test appropriate to its platform
      (e.g. "the `.scr` responds to `/p` without crashing").

## Phase 4 — Installable packages

- [ ] Windows: `.msi` via `cargo-wix`, installs+registers the `.scr`.
- [ ] macOS: signed `.pkg`/`.dmg` installing the `.saver` bundle
      (unsigned/ad-hoc-signed builds until there's an Apple Developer ID).
- [ ] Linux: `.deb` and an AppImage.
- [ ] GitHub Actions builds all three on tagged releases and attaches them
      to a GitHub Release, so the end state is: pick your OS, download one
      file, install it, select "neo_win_pipes" in your screensaver
      settings.

## Explicitly out of scope for now

- Non-Rust language bindings.
- Mobile/tablet screensaver equivalents.
- Config UI beyond what each OS's native screensaver settings contract
  requires (no separate GUI settings app).

Revisit this list if a phase reveals it was wrong — this is a plan, not a
contract.
