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
- [x] CI (GitHub Actions): build + test + fmt + clippy on `windows-latest`,
      `macos-latest`, `ubuntu-latest` for every push/PR. First real run
      passed on all three (`windows-latest` 4m30s, `macos-latest` 1m29s,
      `ubuntu-latest` 2m6s) — including confirming the Linux dev-library
      install step (`libx11-dev`/`libxkbcommon-dev`/etc., added blind since
      there's no local Linux machine) was actually sufficient for
      `winit`/`wgpu` to compile there.

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
- [x] Wired up as the Windows `.scr`'s `/c` handler (Phase 3): `pipes-app`
      spawns `pipes-settings` as a child process and exits, rather than
      reimplementing a config UI inside the `.scr` itself.
- [ ] Not yet exposed in the drawer: `straight_weight`/`turn_weight` ratio
      and `elbow_probability`, even though both are configurable in
      `SimConfig` and validated as wanted (`pipes.sh -s`) per
      `docs/FEATURE_IDEAS.md`. Small follow-up.

## Phase 3 — Native screensaver wrappers

Status differs sharply by platform, honestly: only Windows was buildable
*and* testable on the machine this was developed on. See
[ARCHITECTURE.md](ARCHITECTURE.md#native-screensaver-wrappers-phase-3)
for the full writeup per platform.

### Windows — done, tested

- [x] `/s`, `/c`, `/p <HWND>` contract parsed (`screensaver_args.rs`,
      unit-tested) and acted on in `pipes-app::main`.
- [x] `/s`: real borderless fullscreen, cursor hidden, exits on any
      keypress/click/mouse-movement (after a 750ms startup grace period —
      see ARCHITECTURE.md for the exact bug this fixed).
- [x] `/c`: spawns `pipes-settings` next to the running executable, exits.
- [x] `/p <hwnd>`: reparents our window into the given HWND via Win32
      `SetParent`/`SetWindowLongPtrW`/`SetWindowPos` (`winsaver.rs`).
- [x] Smoke-tested locally, end to end: `/c` launches `pipes-settings` and
      exits; `/p <hwnd>` embeds live into a real test window (screenshot-
      verified); `/s` goes fullscreen and exits correctly on input after
      the grace period; the built `.exe` renamed to `.scr` behaves
      identically to the `.exe` when invoked the way Windows itself would
      (`CreateProcess`, not a shell's file-association-aware launch).
- [ ] Not yet done: no live resize handling for the `/p` preview thumbnail
      (assumed fixed-size for the dialog's lifetime — untested whether
      that assumption holds on every Windows version); no `.msi`
      installer yet (Phase 4).

### Linux — argument parsing done and tested; rendering not wired up

- [x] `pipes-xscreensaver` crate: parses `-root` / `-window-id <id>`
      (decimal or hex), unit-tested.
- [x] As of the first real CI run, this is no longer just "should build
      cross-platform" — GitHub Actions' `ubuntu-latest` runner actually
      compiled and passed tests for the *whole* workspace (including
      `winit`/`wgpu`-based `pipes-render`/`pipes-app`/`pipes-settings`,
      not just this crate) in 2m6s, confirming the blind guess at which
      apt packages `winit`/`wgpu` need (`libx11-dev`, `libxkbcommon-dev`,
      etc.) was sufficient. That's a real, verified data point now, not
      an assumption — though it's compilation + unit tests, not a live
      screensaver running on a real X server.
- [ ] Not done: X11 connection, resolving/embedding into the target
      window via a raw Xlib/XCB handle, wiring into `pipes-render`. Needs
      `x11rb`/`x11-dl`, and CI can compile it but can't smoke-test actual
      window embedding without a display server — deliberately left
      unwritten rather than shipped as unverified guesswork.
- [ ] The exact xscreensaver hack CLI contract itself
      (`docs/FEATURE_IDEAS.md`'s research) needs to be checked against a
      live xscreensaver install/its `screenhack.c` source — treat it as a
      documented best guess until then.

### macOS — design only, no `.saver` code yet

- [x] Confirmed by the first real CI run: the existing workspace (`pipes-core`,
      `pipes-render`, `pipes-app`, `pipes-settings`, `pipes-xscreensaver` —
      everything that exists so far) builds and passes its tests on
      `macos-latest` in 1m29s. Useful, but not the same thing as a `.saver`
      — no `ScreenSaverView`/bundle code exists yet.
- [ ] Not started. A `.saver` is an `NSBundle` implementing
      `ScreenSaverView` (`objc2` + `objc2-screen-saver`), built as a
      `cdylib` with `-bundle` and an `Info.plist` — normally an Xcode-
      toolchain job, not plain `cargo build`. No code was written here:
      unlike Linux's argument parsing, there's no piece of this that's
      both real progress and verifiable without a Mac.

### Cross-platform

- [x] CI now builds and tests the whole workspace on all three OSes on
      every push — see the Phase 1 checkbox above for the first run's
      results.
- [ ] Each wrapper gets its own smoke test appropriate to its platform —
      done for Windows (see above); no longer blocked on CI access for
      macOS/Linux (that's fixed), but still blocked on the actual `.saver`
      and X11-embedding code not existing yet to test.
- [ ] Multi-monitor behavior (span vs. per-display) — not addressed on
      any platform yet; see "explicitly out of scope for now" below.

## Phase 4 — Installable packages

### Windows — done

- [x] `neo_win_pipes.msi` (`installer/main.wxs`, built with WiX Toolset
      v7 — not `cargo-wix`, which targets the older WiX v3 that needs an
      admin-only Windows Feature; WiX v7 installs per-user as a `dotnet`
      tool instead). One double-click installs both `pipes-settings.exe`
      (Program Files + Start Menu shortcut, like a normal app) and
      `pipes-app.exe` (renamed `neo_win_pipes.scr`, into `System32` — the
      OS's own convention for where the screensaver dropdown looks).
- [x] Deliberately does **not** auto-select itself as the active
      screensaver or touch `HKCU\Control Panel\Desktop` — installing only
      makes it available in the dropdown, the same restraint any
      well-behaved installer should have toward existing user settings.
- [x] Validated without needing an actual elevated install (which would
      need an interactive UAC click, not automatable): `wix msi validate`
      (clean ICE pass, one expected/benign ICE09 warning about the
      System32 file being non-permanent — correct, since we want it
      removed on uninstall) and an administrative extract
      (`msiexec /a ... TARGETDIR=...`) confirming the exact file layout.
      The final elevated install itself still needs a human click-through
      — see [DEVELOPMENT.md](DEVELOPMENT.md#building-the-windows-installer-msi).
- [x] `pipes-app`'s `/c` handler updated (`settings_app_candidates()`) to
      find `pipes-settings.exe` in its installed Program Files location,
      not just next to the running exe — needed since the `.scr` and the
      settings app now live in different directories once installed.
- [ ] Not yet done: code signing (currently unsigned — Windows SmartScreen
      will warn on first run until there's a code-signing certificate);
      CI doesn't build this on tagged releases yet (still manual).

### macOS / Linux — not started

- [ ] macOS: signed `.pkg`/`.dmg` installing the `.saver` bundle
      (unsigned/ad-hoc-signed builds until there's an Apple Developer ID)
      — blocked on the `.saver` itself not existing yet (Phase 3).
- [ ] Linux: `.deb` and an AppImage — blocked on the X11 embedding not
      existing yet (Phase 3).
- [ ] GitHub Actions builds all three on tagged releases and attaches them
      to a GitHub Release, so the end state is: pick your OS, download one
      file, install it, select "neo_win_pipes" in your screensaver
      settings. CI already builds/tests on all three OSes per-push (see
      Phase 1) — this would extend that to produce release artifacts.

## Explicitly out of scope for now

- Non-Rust language bindings.
- Mobile/tablet screensaver equivalents.
- Multi-monitor-specific configuration (span vs. per-display) — flagged as
  a real modern-platform expectation in `docs/FEATURE_IDEAS.md`, deferred
  to Phase 3 since it's tied up with each OS's screensaver contract.

Revisit this list if a phase reveals it was wrong — this is a plan, not a
contract.
