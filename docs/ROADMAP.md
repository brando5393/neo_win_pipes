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
- [x] Dissolve-on-reset: pipes shrink away over `dissolve_duration_ticks`
      before the scene clears, echoing the original's transition, instead
      of vanishing instantly — toggleable (`dissolve_on_reset`, default
      on). Purely a render-time effect (`pipes-render::instance` scales
      geometry by `Scene::dissolve_progress()`); `pipes-core` only tracks
      a countdown and freezes growth during it. Verified two ways: unit
      tests proving the shrink math (radius scales exactly
      1.0→0.5→0.0 proportionally) and a live run's logs showing several
      clean dissolve→reset cycles at the configured duration (a Windows
      Hello lock screen interrupted the visual/screenshot check —
      unrelated to the app, and the other two verifications were judged
      sufficient rather than fighting the lock screen).
- [x] Found and fixed a real forward-compatibility bug while building
      this: `SimConfig`/`PipeVisuals`/`CameraConfig`/`AppConfig` didn't
      have container-level `#[serde(default)]`, so a config file saved
      before any of these two new fields existed would fail to parse
      *entirely* and silently discard every other setting in it, not just
      fall back for the fields that were actually missing. Fixed on all
      four types; regression-tested with a fixture file missing the new
      fields but customizing others, confirming the others survive.
- [x] Teapot easter egg: a rare, separate roll (`JointKind::Teapot`,
      `SimConfig::teapot_easter_egg_enabled` + `teapot_probability`,
      checked before the elbow/ball roll) renders a procedural teapot
      mesh (`pipes_render::geometry::teapot()` — lathed body/spout,
      torus-arc handle, sphere knob) at a joint instead of the normal
      ball/elbow. Not the exact historical Utah teapot control-point
      dataset (not available to copy correctly) — an honest procedural
      approximation. Two real bugs only caught by actually rendering it
      and looking (unit tests checked well-formedness and vertex
      distance-from-origin, neither of which catches these):
      (1) `lathe()`'s triangle winding turned out backwards relative to
      the mesh's own vertex normals, which — combined with backface
      culling — silently discarded most of the body/spout, leaving only
      a stray sliver of the (correctly-wound) handle visible. Fixed by
      disabling backface culling in the shared pipeline entirely rather
      than hand-deriving winding per mesh: lighting here uses each
      vertex's authored normal directly, not a winding-derived face
      normal, so culling buys negligible fill-rate savings at this
      scene's tiny polygon counts against a real, easy-to-reintroduce
      failure mode. (2) Even after that, the first spout profile was a
      full 1.0-unit-long cone translated out past the body's own 0.5
      radius, so the whole mesh's bounding box was ~2 units wide against
      ~1.1 tall — a long flat bar, not a teapot. Fixed by shortening the
      spout and moving its base back to the body's actual surface;
      added a bounding-box aspect-ratio regression test
      (`teapot_is_well_formed_and_roughly_teapot_sized`) since the
      existing per-vertex distance check didn't catch it.

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
- [x] "Pipe behavior" drawer section exposes `straight_weight`/
      `turn_weight` ratio and `elbow_probability` sliders (validated as
      wanted — `pipes.sh -s` — per `docs/FEATURE_IDEAS.md`), plus the
      teapot easter egg toggle/probability.

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
- [x] A discoverable "Uninstall neo_win_pipes" Start Menu shortcut
      (`msiexec /x [ProductCode]`) alongside "Pipes Settings" — on top of
      the automatic Programs & Features listing every MSI gets for free.
- [x] Auto-update: `pipes-settings` checks GitHub Releases in the
      background and offers a one-click "Update Now" when a newer `.msi`
      is published — see [ARCHITECTURE.md](ARCHITECTURE.md#auto-update-pipes-settingsupdate)
      for the full design and why a fully silent updater isn't realistic
      given the `System32` requirement (still one UAC prompt per update).
      Free: no paid update host, no background service — just GitHub's
      own Releases API.
- [x] `.github/workflows/release.yml`: pushing a `v*.*.*` tag now builds
      and publishes the `.msi` to a GitHub Release automatically — the
      "supply side" the updater above depends on. One version number
      (the git tag) flows into `Cargo.toml`, the compiled binaries'
      `CARGO_PKG_VERSION`, and the installer's `ProductVersion`, so
      there's nothing to keep in sync by hand across three places.
- [ ] Not yet done: code signing (currently unsigned — Windows SmartScreen
      will warn on first run/update until there's a code-signing
      certificate, which costs money — deliberately out of scope for a
      free hobby project unless that changes).
- [x] Persistent file logging + a human-readable fatal-error dialog.
      Prompted by the `windows_subsystem` fix above: a release build has
      no console, so `stdout` (where logs used to go) now goes nowhere —
      without this, a real failure would be completely invisible instead
      of just hard to see. `pipes_render::diagnostics` (shared by both
      binaries) now writes a daily-rotating log file alongside the config
      file, and installs a panic hook that shows a native `MessageBoxW`
      dialog with a plain-English summary before the process exits. See
      `docs/LOGGING.md`.

#### Known issues (found testing the real v0.2.0 install — fixed in v0.2.1)

- [x] Clicking "Settings" from the Windows screensaver dialog didn't open
      Pipes Settings after a real install. Real root cause (confirmed by
      actually building and validating the MSI locally, not just reading
      code): `.github/workflows/release.yml`'s `wix build` never passed
      `-arch x64`, so WiX v7 defaulted to x86 and resolved
      `ProgramFilesFolder` to `Program Files (x86)` — but `cargo build
      --release` on the runner produces native 64-bit binaries, and a
      native 64-bit `pipes-app.exe`'s `%ProgramFiles%` resolves to plain
      `Program Files`. The two disagreed, so `settings_app_candidates()`
      never found the real (x86-installed) `pipes-settings.exe`. Fixed:
      `wix build` now passes `-arch x64`; `installer/main.wxs`'s
      `StandardDirectory` refs switched to `ProgramFiles64Folder` /
      `System64Folder` (required once components are 64-bit — caught by
      `wix msi validate`'s ICE80 check, again only by actually running it).
      `settings_app_candidates()` in `crates/pipes-app/src/main.rs` also
      hardened to check `ProgramFiles`, `ProgramW6432`, and
      `ProgramFiles(x86)`, so a future build/installer arch mismatch
      degrades to a redundant lookup instead of silent failure.
- [x] Dissolve and teapot settings didn't show up when Pipes Settings was
      opened normally. Not actually missing from the code — `ui.rs`'s
      `egui::SidePanel` had no `ScrollArea`, and the window is a fixed
      1200×760 (`main.rs`), so newly-added sections were pushed below the
      visible area with no way to scroll to them. Fixed: the drawer's
      contents are now wrapped in `egui::ScrollArea::vertical()`.
- [x] A console window (the tracing log output, green/yellow text) popped
      up whenever Pipes Settings launched, came to the front, and closing
      it killed the whole app — because closing a console window sends
      its default control handler a close event that terminates the
      attached process. Root cause: neither `pipes-app` nor
      `pipes-settings` set `#![windows_subsystem = "windows"]`, so both
      defaulted to the console subsystem on Windows. Fixed:
      `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
      in both crates' `main.rs` — keeps the console for `cargo run` dev
      builds, drops it from the shipped release build. Verified by
      inspecting the built `.exe`'s PE header directly (`file` reports
      "console" for debug builds, "GUI" for release), not just by reading
      the attribute.
- [x] No app icon. Fixed: a "classic chrome elbow" design (a stylized
      pipe corner, chosen from three concepts after checking legibility
      down to 16px — a busier tri-color concept looked great at 256px but
      turned into an illegible blob at taskbar size) lives at
      `assets/icon/` as a 1024px master plus generated `windows/icon.ico`
      (multi-resolution), `macos/icon.icns`, and a Linux `hicolor` PNG
      theme set + scalable SVG. Wired into the actual build via `build.rs`
      + the `winres` crate in both `pipes-app` and `pipes-settings`
      (Windows-only, gated via `[target.'cfg(windows)'.build-dependencies]`
      so it doesn't affect macOS/Linux builds) — confirmed embedded by
      extracting the icon back out of the compiled `.exe`, not just by
      reading the build script. `installer/main.wxs` also references it
      for `ARPPRODUCTICON` (Programs and Features listing) and both Start
      Menu shortcuts.

### macOS / Linux — not started

- [ ] macOS: signed `.pkg`/`.dmg` installing the `.saver` bundle
      (unsigned/ad-hoc-signed builds until there's an Apple Developer ID)
      — blocked on the `.saver` itself not existing yet (Phase 3).
- [ ] Linux: `.deb` and an AppImage — blocked on the X11 embedding not
      existing yet (Phase 3).
- [ ] `.github/workflows/release.yml` only builds the Windows `.msi` so
      far. Once the `.saver`/`.deb`/AppImage exist, add matching jobs
      there (same tag trigger, same one-version-number-from-the-tag
      approach) so the end state is: pick your OS, download one file,
      install it, select "neo_win_pipes" in your screensaver settings.

## Explicitly out of scope for now

- Non-Rust language bindings.
- Mobile/tablet screensaver equivalents.
- Multi-monitor-specific configuration (span vs. per-display) — flagged as
  a real modern-platform expectation in `docs/FEATURE_IDEAS.md`, deferred
  to Phase 3 since it's tied up with each OS's screensaver contract.

Revisit this list if a phase reveals it was wrong — this is a plan, not a
contract.
