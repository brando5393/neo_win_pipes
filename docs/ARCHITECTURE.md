# Architecture

## Guiding principle

The simulation (grid, pipe growth, joints, scene lifecycle) must never
depend on a GPU, a window, or a display. That's what makes it possible to
run the full test suite headlessly on every CI runner (Windows, macOS,
Linux) on every commit, and it's what makes bugs in pipe-growth logic
reproducible with a plain `cargo test` instead of "run the app and eyeball
it." Everything platform-specific — rendering, windowing, native
screensaver packaging — is layered on top as a separate crate that depends
on the core, never the other way around.

```
crates/
  pipes-core/     engine: grid, pipe growth, joints, scene lifecycle.
                  No rendering/windowing deps. Fully unit-tested.
  pipes-render/   shared layer: geometry generation, GPU instancing, the
                  wgpu Renderer, and AppConfig (the persisted settings file
                  both binaries below read/write). No app/window of its own.
  pipes-app/      the actual screensaver: a fullscreen-friendly window
                  that loads AppConfig and renders via pipes-render.
  pipes-settings/ "Pipes Settings": a window with a live preview (via
                  pipes-render, rendered into part of the window) next to
                  an egui settings drawer, editing the same AppConfig file.
  (future) pipes-win-scr/    Windows .scr wrapper around pipes-app's engine
  (future) pipes-mac-saver/  macOS .saver (ScreenSaverView) wrapper
  (future) pipes-xscreensaver/ Linux xscreensaver module wrapper
```

`pipes-render` exists specifically so `pipes-app` and `pipes-settings`
never duplicate rendering or config-loading code — both are thin
`winit`/event-loop shells around the same `Renderer`, `build_instances`,
and `AppConfig`.

See [ROADMAP.md](ROADMAP.md) for why the native wrappers are a later phase
rather than day one.

## `pipes-core`

The engine, expressed as four small modules:

- **`direction`** — the six axis-aligned `Direction`s a pipe can travel,
  and the invariant that a direction's opposite is well-defined (used to
  forbid immediate backtracking).
- **`grid`** — `GridPos` (an integer 3D coordinate), `GridBounds` (the
  simulation volume), and `OccupancyGrid` (which cells are filled — this is
  what gives pipes collision detection against themselves and each other,
  unlike some prior clones; see [RESEARCH.md](RESEARCH.md)).
- **`pipe`** — a single `Pipe`'s growth: at each `step()`, it weighs
  "continue straight" heavily against "turn" (see
  `SimConfig::straight_weight` / `turn_weight`), filters candidate
  directions down to ones that are both non-reversing and lead to a free
  cell, and terminates (`TerminationReason::Stuck` or `MaxLengthReached`)
  when it can't move.
- **`scene`** — `Scene` owns the `OccupancyGrid` and the set of live
  `Pipe`s, and drives one simulation tick at a time: step every pipe, reap
  the dead ones, spawn replacements up to `max_pipes`, and reset the whole
  scene once occupancy crosses `reset_occupancy_ratio`.

Randomness is seeded (`rand_pcg::Pcg32`) and threaded explicitly through
`Scene`/`Pipe`, never pulled from thread-local/OS entropy inside the
engine. That's the whole trick behind the `same_seed_produces_identical_*`
tests in `pipe.rs` and `scene.rs`: given a seed, the entire simulation
(every pipe's path, every joint choice, every spawn/reset) is bit-for-bit
reproducible. That determinism is also why a bug report can eventually
just say "seed 12345, tick 800" instead of a screen recording.

## `pipes-render`

A `winit` + `wgpu` rendering/config layer with no `main()` of its own,
structured in four parts:

- **`geometry`** — pure procedural mesh generation (unit cylinder, cuboid,
  UV sphere), no GPU handle involved, so shape correctness (vertex/index
  counts, unit normals, radius bounds) is unit-tested without a window.
- **`instance`** — converts a live `pipes_core::Scene` into per-mesh GPU
  instance data: one cylinder or cuboid instance per path segment
  (depending on `PipeStyle`), one sphere instance per joint (scaled up for
  `JointKind::Ball`, slightly smaller for `Elbow`) plus start/end cap
  spheres. Also unit-tested (e.g. every cardinal direction must produce a
  finite model matrix — a regression guard on `Quat::from_rotation_arc`'s
  degenerate antiparallel case).
- **`renderer`** — the actual wgpu setup: instanced pipeline, a
  Lambertian + Blinn-Phong shader (`shader.wgsl`) for a "shiny metal" look,
  depth testing, and a camera that slowly orbits the scene per
  `docs/RESEARCH.md`'s note on the original's optional rotation.
  `Renderer::render` draws into the whole surface (the screensaver's use
  case); `Renderer::render_with` additionally takes a viewport rect and an
  `extra` closure called with the same device/queue/encoder/surface view,
  which is how `pipes-settings` layers an egui pass into the same frame
  without `pipes-render` knowing anything about egui.
- **`config`** — `AppConfig`: everything a settings session can tune
  (`SimConfig`, `PipeVisuals`, `CameraConfig`, tick speed), serialized as
  TOML to the OS's standard per-user config directory (via the
  `directories` crate) so both binaries below read/write the exact same
  file. `AppConfig::sanitize()` clamps every field to a safe range on
  load, so a hand-edited or stale config file can't produce degenerate
  geometry or a divide-by-zero — unit-tested directly (missing file,
  corrupt file, save/load round-trip, out-of-range clamping).

`wgpu` was chosen over raw OpenGL/Direct3D because one API target compiles
to Vulkan (Linux), Metal (macOS), and DirectX12/Vulkan (Windows) — what
makes one Rust codebase realistic across all three OSes.

Known simplifications, tracked in [ROADMAP.md](ROADMAP.md): the material
is specular-only, not a true chrome environment reflection; elbow joints
render as a small sphere rather than a smooth torus bend.

## `pipes-app`

The screensaver itself: loads `AppConfig`, ticks the `Scene` on a fixed
interval (`AppConfig::tick_interval_ms`, independent of frame rate),
rebuilds instance buffers every frame, and calls `Renderer::render` with
`viewport: None` (the whole window).

## `pipes-settings`

"Pipes Settings" — a live preview next to a settings drawer, in one
window. Two libraries are combined manually (not via `eframe`, which owns
its own render loop and doesn't leave room for a custom wgpu pass):
`egui` + `egui-winit` (input/platform integration) and `egui-wgpu` (paints
egui's tessellated output into a `wgpu::RenderPass`) sit alongside
`pipes-render`'s `Renderer`, sharing the same `wgpu::Device`/`Queue`.

Per frame (`main.rs`): run the egui UI closure first (`ui.rs`) to compute
the settings drawer *and* learn how much space is left for the preview
(`CentralPanel`'s `available_rect_before_wrap()` — critically with
`Frame::none()`, since `CentralPanel`'s default frame paints an opaque
background that would otherwise hide the 3D content drawn under it in the
same frame); convert that rect from egui's logical points to physical
pixels; then call `Renderer::render_with` with that rect as the viewport
(so the 3D pass is scissored to just the preview pane) and an `extra`
closure that runs the egui render pass on top, using `wgpu`'s `Load` op so
it composites over the pipes instead of clearing them.

`ui::draw` reports back an `Outcome`: whether a change affects the
simulation itself (`sim_changed` — e.g. style, pipe count, palette, grid
size; requires rebuilding the live-preview `Scene`, since `SimConfig` is
baked in at `Scene::new`) versus only rendering (`other_changed` — pipe
thickness, camera, speed; takes effect next frame with no rebuild), plus
`reset_to_defaults` for the drawer's reset button. `main.rs` acts on
`Outcome` and autosaves `AppConfig` on any change.

## Native screensaver wrappers (Phase 3)

Each OS has a different screensaver contract. Status differs sharply by
platform because only Windows is buildable *and* testable on the
development machine this was written on — see
[DEVELOPMENT.md](DEVELOPMENT.md) for why that matters for what "done"
can honestly mean per platform right now.

### Windows — done, tested

`pipes-app` *is* the `.scr`: no separate wrapper crate, because the
contract is simple enough to live directly in `pipes-app`'s `main.rs` plus
two small modules:

- **`screensaver_args.rs`** — pure, OS-independent parsing of the four
  ways Windows invokes a `.scr`: `/s` (fullscreen), `/c[:<hwnd>]`
  (settings), `/p <hwnd>` (live preview thumbnail), `/a <hwnd>` (legacy
  change-password, no-op). Unit-tested.
- **`winsaver.rs`** (`#[cfg(windows)]`) — the one bit of real Win32 FFI:
  `/p <hwnd>` reparents our own winit window as a child of the HWND
  Windows gives us (`SetParent` + `SetWindowLongPtrW(GWL_STYLE, WS_CHILD)`
  + `SetWindowPos` to fit its client rect), so the exact same renderer
  draws live inside Settings' screensaver dropdown thumbnail.
- `/c` spawns `pipes-settings` (found next to the running executable) as
  a child process and returns immediately, rather than reimplementing a
  config UI inside the `.scr` itself.
- `/s` mode goes real borderless fullscreen and hides the cursor, and —
  like every real screensaver — exits on any keypress, click, or mouse
  movement. A **750ms startup grace period** on that exit-on-input check
  is load-bearing, not cosmetic: window creation itself generates a
  synthetic `CursorMoved` (the OS reporting where the cursor already was)
  and can replay a stray input event, which made the very first
  implementation exit within milliseconds of opening, before a single
  frame rendered — caught by actually running it, not just by reading
  the code.

Packaging is genuinely simple: the built `pipes-app.exe`, renamed to
`.scr`, *is* the installable artifact (a `.scr` is just a normal PE
executable Windows treats specially by convention). Verified locally end
to end: `/c` launching `pipes-settings` and exiting, `/p <hwnd>` embedding
live into a real test window, `/s` going fullscreen and exiting correctly
on input after the grace period — including confirming the renamed
`neo_win_pipes.scr` file itself behaves identically to `pipes-app.exe`
when invoked directly (as Windows' own screensaver mechanism would).

### Linux — argument parsing done and tested; rendering not wired up

`pipes-xscreensaver` parses the xscreensaver "hack" invocation contract
(`args.rs`: `-root` or `-window-id <id>`, decimal or hex) with the same
pure/tested approach as the Windows side. What's **not** implemented:
opening an X11 connection, resolving/embedding into the target window via
a raw Xlib/XCB window handle, and wiring that into `pipes-render`. That
needs `x11rb` (or `x11-dl`) and careful `raw-window-handle` construction —
deliberately left unwritten rather than shipped as unverified guesswork,
because this project has no Linux machine to compile or run it against.
`docs/FEATURE_IDEAS.md`'s research note on xscreensaver's exact CLI
contract is itself a best-effort reading of third-party ports, not a
confirmed fact — the single biggest thing to verify first on real Linux.

### macOS — design only, no code yet

A `.saver` is a `NSBundle` implementing `ScreenSaverView`
(`animateOneFrame`/`drawRect:`), which means Objective-C/Cocoa bridging
(via `objc2` + `objc2-screen-saver`) and a bundle structure
(`Info.plist`, principal class, linked as a `cdylib` with `-bundle`)
that Xcode's toolchain — not plain `cargo build` — normally produces. No
code was written for this: unlike Linux's argument parsing, there's no
meaningfully "pure" sub-piece of this that's both real progress and
testable without a Mac, so writing Rust/ObjC glue here would just be
unverified guesswork with extra steps. This is intentionally the least
complete of the three platforms.

Each wrapper's job, on every platform, is to be a *thin* shell — handle
the OS's window-embedding/preview/config contract, then hand off to the
exact same `pipes-core` `Scene` and `pipes-render` rendering Phase 2
built. That's why the core/render split earlier in this document isn't
optional future-proofing — it's what let the Windows wrapper above ship
as a couple hundred lines instead of a fork of the whole engine.

## Windows installer (Phase 4)

`installer/main.wxs` (built with WiX Toolset v7, `wix build`) installs
two things to two different, deliberately separate places:

- `pipes-app.exe`, renamed `neo_win_pipes.scr`, into `System32` — where
  Windows' own "Screen saver" dropdown scans for `.scr` files. There's no
  way around this location; it's the OS's convention, not a choice we
  made.
- `pipes-settings.exe`, into `%ProgramFiles%\neo_win_pipes\` with a Start
  Menu shortcut — installed like any normal app, not dumped into
  `System32` alongside the `.scr` just because that would've been easier.
  This is why `pipes-app`'s `/c` handler
  (`settings_app_candidates()` in `main.rs`) checks *two* locations, not
  one: next to itself (dev/testing, both binaries in the same
  `target/debug`) and this Program Files path (installed).

Deliberately **not** done automatically at install time: selecting
`neo_win_pipes` as the active screensaver, or touching
`HKCU\Control Panel\Desktop` at all. Installing only makes it available
in the dropdown — a real, if small, restraint: the installer's job is to
put the software where it belongs, not to silently override whatever
screensaver setting was already there.

Building this needs WiX v7, not the older WiX v3 `cargo-wix` normally
wraps — v3 needs the .NET Framework 3.5 Windows Feature, which is
admin-only to enable, while v7 installs per-user as a `dotnet` tool. That
tradeoff is a real EULA: WiX v7 requires accepting its "Open Source
Maintenance Fee" terms once (free below a $10,000/year revenue
threshold, which doesn't apply to this project, but still a genuine
legal acceptance) — worth knowing since it's the reason the build
prerequisites in [DEVELOPMENT.md](DEVELOPMENT.md) include an explicit
`wix eula accept` step rather than a silent one.

Validating the built `.msi` doesn't require an elevated install (which
would need an interactive UAC click, not automatable): `wix msi
validate` runs ICE checks (one expected/benign warning, `ICE09`, about
the `System32` file being "non-permanent" — correct, since uninstall
should remove it), and `msiexec /a ... TARGETDIR=...` does a non-elevated
administrative extract that confirms the exact file layout without
installing anything for real.

An "Uninstall neo_win_pipes" shortcut sits next to "Pipes Settings" in the
Start Menu, invoking `msiexec /x [ProductCode]` — genuinely just a more
discoverable path to what Programs & Features (Settings → Apps) already
does automatically for any MSI-installed product; not something this file
had to build from scratch.

## Auto-update (`pipes-settings::update`)

The goal, stated plainly: updates should reach installed copies without a
paid update host, without a background service, and without a fully
silent (zero-prompt) mechanism that isn't actually achievable here — the
screensaver lives in `System32`, so *any* update touching it needs one UAC
prompt, same as the original install. What's built is the realistic
version of "automatic": `pipes-settings` checks in the background, the
human clicks once.

- On startup, `pipes-settings` spawns a background thread
  (`spawn_update_check` in `main.rs`) that calls
  `update::check_for_update`, which GETs
  `https://api.github.com/repos/brando5393/neo_win_pipes/releases/latest`
  and compares its tag against `env!("CARGO_PKG_VERSION")` via `semver`.
  Any failure — offline, GitHub down, rate-limited, no releases yet —
  yields `None`, silently; a background version check failing is correct,
  boring behavior, not something that should ever interrupt using the
  app. The JSON-parsing-and-comparison logic (`parse_update`) is pure and
  unit-tested against sample API responses; the actual HTTP call isn't
  (hitting a real external API on every `cargo test` run would be slow
  and flaky, not a good test).
- If a newer version with a `.msi` release asset is found, the UI shows a
  dismissible top banner: "A new version is available" + **Update Now** +
  **Release notes** + **Dismiss**. Clicking **Update Now** downloads the
  `.msi` to a temp file and launches `msiexec /i ... /passive /norestart`
  (skips the wizard's Welcome/License/Finish clicks since the user
  already saw those on first install — but not the UAC prompt, which
  can't be skipped), then exits `pipes-settings` itself so the running
  `.exe` isn't locked while its own file gets replaced.
- This only checks when `pipes-settings` happens to be open — there's no
  background service polling on a schedule. Given this is a screensaver
  utility people open occasionally to tweak settings, that's judged a
  reasonable cadence; revisit if it isn't in practice.

### The supply side: `.github/workflows/release.yml`

The updater above only works if there's actually a newer GitHub Release
to find. Pushing a tag matching `v*.*.*` triggers a workflow that: patches
`Cargo.toml`'s `[workspace.package]` version to match the tag (so the
binaries' `CARGO_PKG_VERSION` — what the updater compares against — is
never out of sync with the release that ships them), runs the full test
suite, builds release binaries and the `.msi` (with
`-d ProductVersion=<tag>`, so the installer's version matches too), and
publishes a GitHub Release with the `.msi` attached via
`softprops/action-gh-release`. One version number, sourced from one git
tag, flows into the compiled binary, the installer, and the release page
— not three things to keep in sync by hand.

Cutting a release is then just: `git tag v0.2.0 && git push origin
v0.2.0`. See [DEVELOPMENT.md](DEVELOPMENT.md) for the full step.

## Logging

Structured, human-readable-first (not JSON-first) `tracing` output, so
`cargo run` output and log files read like sentences, not blobs. Full
event catalog and rationale: [LOGGING.md](LOGGING.md).

## Testing

Unit tests live next to the code they test (`#[cfg(test)] mod tests` in
each `pipes-core` module) and run headlessly — no display, no GPU, no
mocked window. Full test philosophy and how to run things:
[DEVELOPMENT.md](DEVELOPMENT.md).
