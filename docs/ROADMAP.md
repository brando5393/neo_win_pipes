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

- [x] `pipes-app` grows a `winit` window + `wgpu` renderer.
- [x] Geometry generation: `pipes-app::geometry` turns segments/joints into
      cylinder / cuboid / sphere meshes (pure functions, unit-tested on
      vertex/index counts, normal validity, and radius bounds — no GPU
      needed to verify shape correctness).
- [x] Instancing: `pipes-app::instance` converts a live `Scene` into
      per-mesh GPU instance buffers (round segments, square segments,
      joints/caps), unit-tested for degenerate-direction NaN safety.
- [x] Slowly drifting orbit camera around the scene.
- [x] Manual visual verification: confirmed rendering live —
      [`docs/screenshots/phase2-first-render-seed3.png`](screenshots/phase2-first-render-seed3.png).
- [ ] True chrome material: current shading is Lambertian + Blinn-Phong
      specular (looks decent, reads as "shiny," but isn't an environment
      reflection like the original's chrome). Revisit with an env/cube map
      if it's worth the complexity.
- [ ] Elbow joints currently render as a (slightly smaller) sphere rather
      than a smooth torus bend — visually fine, not geometrically accurate
      to "elbow." Torus geometry is a nice-to-have polish item.
- [x] Checked-in reference screenshot in `docs/screenshots/` (one so far;
      add more as the renderer evolves for visual regression comparison).

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
