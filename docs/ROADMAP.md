# Roadmap

Phased so that every phase leaves the repo in a state with passing tests,
current docs, and working (if limited) software — never a half-wired
feature branch.

## Phase 1 — Simulation core (in progress)

- [x] `pipes-core`: grid, direction, pipe growth/turning/termination,
      scene lifecycle, seeded determinism.
- [x] Unit test suite for all of the above (22 tests as of this writing).
- [x] `pipes-app`: originally a headless CLI runner with human-readable
      `tracing` logs — superseded by the Phase 2 windowed renderer below.
- [x] Repo scaffold: `CLAUDE.md`, docs set, MIT license, `.gitignore`.
- [ ] CI (GitHub Actions): build + test on `windows-latest`,
      `macos-latest`, `ubuntu-latest` for every push/PR.

## Phase 2 — Rendering

- [x] `pipes-app` grows a `winit` window + `wgpu` renderer (later factored
      out into the shared `pipes-render` crate — see Phase 2.5).
- [x] Geometry generation: `pipes-render::geometry` turns segments/joints
      into cylinder / cuboid / sphere meshes (pure functions, unit-tested
      on vertex/index counts, normal validity, and radius bounds — no GPU
      needed to verify shape correctness).
- [x] Instancing: `pipes-render::instance` converts a live `Scene` into
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

## Phase 2.5 — Settings app (shipped)

- [x] Extracted `geometry`/`instance`/`renderer` out of `pipes-app` into a
      shared `pipes-render` library crate, with `AppConfig` (persisted as
      TOML in the OS's standard per-user config dir) added alongside them.
- [x] `AppConfig::sanitize()` clamps every field to a safe range on load
      (unit-tested: missing file, corrupt file, save/load round-trip,
      out-of-range clamping) so a hand-edited config can't break the app.
- [x] `pipes-settings`: a standalone window — live 3D preview on the left
      (rendered into a sub-viewport via `Renderer::render_with`), an egui
      settings drawer on the right. Covers all four validated categories
      from `docs/FEATURE_IDEAS.md`: pipe style & count, speed & camera,
      color palette (presets + custom per-color editing), grid size &
      reset threshold. Autosaves on every change; "Reset to defaults"
      button. `pipes-app` (the actual screensaver) reads the same file.
- [x] Manual visual verification: launched, screenshotted, confirmed the
      live preview actually updates and isn't hidden behind egui's default
      opaque panel background (a real bug hit and fixed during
      development — `CentralPanel` needs `Frame::none()` when something
      else is drawing underneath it in the same frame).
- [ ] Wire this UI up as the real Windows `.scr` `/c <HWND>` config dialog
      once Phase 3's Windows wrapper exists (currently standalone-only, by
      deliberate choice — see the project's design questions).
- [ ] Not yet exposed in the drawer: `straight_weight`/`turn_weight` ratio
      and `elbow_probability`, even though both are configurable in
      `SimConfig` and validated as wanted (`pipes.sh -s`) per
      `docs/FEATURE_IDEAS.md`. Small follow-up.

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
- Multi-monitor-specific configuration (span vs. per-display) — flagged as
  a real modern-platform expectation in `docs/FEATURE_IDEAS.md`, deferred
  to Phase 3 since it's tied up with each OS's screensaver contract.

Revisit this list if a phase reveals it was wrong — this is a plan, not a
contract.
