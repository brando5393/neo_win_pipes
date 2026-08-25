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
structured in seven parts:

- **`geometry`** — pure procedural mesh generation (unit cylinder, cuboid,
  UV sphere, plus `lathe`/`torus_arc`/`merge_meshes` — the building blocks
  behind the `teapot()` easter-egg mesh), no GPU handle involved, so shape
  correctness (vertex/index counts, unit normals, radius bounds) is
  unit-tested without a window.
- **`instance`** — converts a live `pipes_core::Scene` into per-mesh GPU
  instance data: one cylinder or cuboid instance per path segment
  (depending on `PipeStyle`), one sphere instance per joint (scaled up for
  `JointKind::Ball`, slightly smaller for `Elbow`) plus start/end cap
  spheres. Also unit-tested (e.g. every cardinal direction must produce a
  finite model matrix — a regression guard on `Quat::from_rotation_arc`'s
  degenerate antiparallel case).
- **`renderer`** — the actual wgpu setup: instanced pipeline, a chrome
  material (`shader.wgsl` — Lambertian diffuse + a procedural
  environment-reflection term, see below), depth testing, and a camera
  that slowly orbits the scene per `docs/RESEARCH.md`'s note on the
  original's optional rotation. `Renderer::render` draws into the whole
  surface (the screensaver's use case); `Renderer::render_with`
  additionally takes a viewport rect and an `extra` closure called with
  the same device/queue/encoder/surface view, which is how
  `pipes-settings` layers an egui pass into the same frame without
  `pipes-render` knowing anything about egui; `Renderer::render_tile`
  (used only by `MonitorMode::Span`, see `pipes-app` below) takes a
  pre-computed off-axis projection in place of the ordinary symmetric one.
  Also owns GPU device-loss recovery: `Device::set_device_lost_callback`
  alone isn't reliable (it can fire too late relative to the frame that
  actually hits the dead device — confirmed by reproducing this for real,
  see `docs/ROADMAP.md`), so the calls that can panic are additionally
  wrapped in `std::panic::catch_unwind`, with the fatal-error panic hook
  (below) told to suppress its dialog for a panic caught this specific
  way. Every caller checks `Renderer::is_device_lost()` before calling
  `render`/`resize` again once it's set.
- **`tile`** — pure off-axis ("asymmetric frustum") projection math for
  `MonitorMode::Span`, with no `wgpu`/window types anywhere in the module
  — see "Multi-monitor behavior" under `pipes-app` below for the technique
  and why it's tested this precisely.
- **`config`** — `AppConfig`: everything a settings session can tune
  (`SimConfig`, `PipeVisuals`, `CameraConfig`, tick speed), serialized as
  TOML to the OS's standard per-user config directory (via the
  `directories` crate) so both binaries below read/write the exact same
  file. `AppConfig::sanitize()` clamps every field to a safe range on
  load, so a hand-edited or stale config file can't produce degenerate
  geometry or a divide-by-zero — unit-tested directly (missing file,
  corrupt file, save/load round-trip, out-of-range clamping).
- **`diagnostics`** — shared by both binaries: `init_logging()` sets up
  logging to stdout *and* a daily-rotating file next to the config file
  (release builds have no console — see "Windows installer" below — so
  without the file, release-build logging would go nowhere), and
  `install_panic_hook()` shows a native fatal-error dialog on an unhandled
  panic (plain-English explanation, the real log file path, and the raw
  technical detail, in that order — see [LOGGING.md](LOGGING.md)).
  `run_suppressing_fatal_dialog()` lets a caller that's about to
  `catch_unwind` its own expected panic (currently just the renderer's
  device-loss guard above) skip the dialog for that specific panic
  without disabling it globally — the hook still always logs.
- **`app_icon`** — the window/taskbar icon, built from an embedded raw
  RGBA pixel dump via `winit::window::Icon`. Kept separate from the
  `.ico` embedded into the `.exe` itself (via `build.rs` + `winres`,
  Windows-only): that resource icon is what Explorer/Start Menu/Programs-
  and-Features show for the *file*, but a running window needs its own
  `with_window_icon`/`with_taskbar_icon` calls (the latter Windows-only)
  to show the same icon on the title bar and taskbar button — winit
  doesn't do that automatically from the exe's embedded resource.

`wgpu` was chosen over raw OpenGL/Direct3D because one API target compiles
to Vulkan (Linux), Metal (macOS), and DirectX12/Vulkan (Windows) — what
makes one Rust codebase realistic across all three OSes.

The chrome material (`shader.wgsl`'s `sample_environment`) reflects a
hand-rolled analytic sky gradient sampled by the reflection vector
(`reflect(-view_dir, normal)`, using the real per-fragment view direction
toward `camera.eye` — a uniform field added specifically so the fragment
shader has a real position to compute that from, not the fixed placeholder
vector the old Blinn-Phong term used) — not a real cubemap texture. That
keeps it consistent with everything else in this crate being generated
procedurally (geometry, palettes) rather than adding the first texture/
image-asset dependency to the project just for this. The environment
color is tinted toward each pipe's own instance color rather than staying
a flat grey mirror, so the reflection and the color palette read together
instead of the environment washing the palette out.

Known simplification, tracked in [ROADMAP.md](ROADMAP.md): elbow joints
render as a small sphere rather than a smooth torus bend.

## `pipes-app`

The screensaver itself: loads `AppConfig`, ticks the `Scene` on a fixed
interval (`AppConfig::tick_interval_ms`, independent of frame rate),
rebuilds instance buffers every frame, and calls `Renderer::render` with
`viewport: None` (the whole window).

### Multi-monitor behavior

Flagged for a while in `docs/ROADMAP.md`/`docs/FEATURE_IDEAS.md` as a
decision every multi-monitor-aware screensaver has to make deliberately —
research into other screensaver frameworks found this handled
inconsistently across the whole category, not something to inherit from
whatever the naive per-OS default does.

`AppConfig::monitor_mode` (`pipes_render::MonitorMode`, a Pipes Settings
toggle under "Multi-monitor") picks between three behaviors, consulted
only by `pipes-app`'s `/s` (fullscreen) mode — the live preview in Pipes
Settings and the `/p` thumbnail always render into one caller-provided
window regardless of this setting:

- **`AllMonitors`** (default) — `main.rs` calls `event_loop.available_monitors()`
  before entering the event loop and spawns one borderless fullscreen
  window per display, each pinned to its monitor via
  `Fullscreen::Borderless(Some(monitor))`. Every window gets its own
  `Renderer` (own `wgpu::Instance`/adapter/device/surface — simplest
  correct thing, not shared) and its own `Scene`, seeded via
  `seed_for_monitor(base_seed, index)` — `base_seed + index * 104729`, an
  arbitrary large prime chosen only to spread seeds apart — so displays
  don't render identical mirrored scenes while a given `--seed` still
  reproduces the exact same multi-monitor run every time (determinism is
  load-bearing project-wide).
- **`Span`** — "pipes travel across displays": one *shared* `Scene`,
  rendered as if every monitor were a tile of one big virtual screen. See
  "Span mode: tile projections" below for how.
- **`PrimaryOnly`** — today's pre-multi-monitor behavior: a single window,
  `Fullscreen::Borderless(None)` (whatever the OS/windowing system treats
  as current), for anyone who prefers only one display active.

`AllMonitors` and `Span` differ structurally, not just in a rendering
flag, so `main.rs` models them as an enum built once at startup:

```rust
enum Rendering {
    Independent(Vec<Instance>),        // AllMonitors: own window+renderer+Scene each
    Span { windows: Vec<SpanWindow>, scene: Box<Scene>, last_tick: Instant },
}
```

(`scene` is boxed purely to keep the enum's variants close in size —
clippy's `large_enum_variant` lint — not for any semantic reason.) Both
variants funnel into the same event loop, keyed by a `HashMap<WindowId,
usize>` built once at startup from whichever variant is active —
`WindowEvent`s are dispatched to the window/instance that owns that
`window_id`; any window's `CloseRequested` or (in `/s` mode, past the
750ms input-grace period) any keyboard/mouse/cursor event calls
`elwt.exit()`, which tears down every window at once, matching how a real
screensaver exits fully on any input regardless of which monitor received
it.

### Span mode: tile projections

The trick (`pipes_render::tile` — the same principle behind physical
multi-projector/tiled-display rigs, a generalization of Kooima's
"Generalized Perspective Projection"): give every monitor's window the
*same* view matrix (same orbiting eye/target — automatic here, since every
`Renderer` derives its view purely from the shared sim bounds, so they're
numerically identical by construction), but a **projection** matrix that
only covers that monitor's own slice of one shared, wider frustum. Render
each tile with its own slice and — since the windows sit on physically
separate, edge-to-edge monitors — they reconstruct one continuous wide
scene with no visible seam, entirely without a single surface spanning
multiple windows or GPUs.

Concretely: `pipes_render::tile::tile_projection(fov_y, near, far,
canvas_wh, tile_rect)` builds an off-center ("asymmetric frustum")
perspective matrix — the standard formula generalizing a symmetric
`perspective_rh`, implemented by hand since `glam` has no off-center
built-in (verified to reduce to `glam::Mat4::perspective_rh` exactly when
the bounds are symmetric — see `tile.rs`'s tests). `main.rs`'s
`virtual_canvas` computes the union bounding box of every monitor's
`(position, size)` rect (winit's `MonitorHandle`, in physical pixels) —
the "virtual canvas" — and translates each monitor's rect into that box's
own local coordinate space; `Renderer::frustum_params()` supplies the same
`(fov_y, near, far)` a single non-spanned window would use, so spanning
changes how the field of view is *sliced*, never the field of view itself.
Each window then renders via `Renderer::render_tile` (parallel to
`render`/`render_with`, sharing their GPU submission code via a private
`draw_frame` helper) instead of the ordinary symmetric-projection path.

Only one `Scene` exists in `Span` mode, so it only steps once per
`tick_interval` regardless of how many windows fire `RedrawRequested` in
the same tick — gated on the `Rendering::Span` variant's single shared
`last_tick`, not a per-window one (contrast `AllMonitors`, where each
`Instance` keeps its own `last_tick` precisely because each has its own
independent `Scene`).

Geometric caveat, stated plainly rather than silently: this only looks
seamless when the OS's virtual-desktop monitor arrangement (Windows
Display Settings' drag-to-arrange, or the equivalent) matches physical
reality, and works best with same-resolution/DPI displays — normal
assumptions, not guaranteed ones. A mismatched arrangement just produces a
differently-cropped view of the same wide scene, not a broken one; nothing
about the underlying math depends on the assumption holding.

**Verification status**: this project's only dev machine has a single
monitor, so all three modes have been run for real and confirmed via their
own log output (`multi-monitor: spawning one independent instance per
display count=1` / `multi-monitor: spanning one shared scene across every
display count=1` / neither line + a single instance, respectively) — each
producing a successful `scene created`/`window(s) opened` with no crash.
The tile-projection math itself is unit-tested precisely (not just
code-reviewed): a tile covering the *whole* virtual canvas is asserted
numerically identical to the ordinary symmetric projection (the exact
single-monitor case), and adjacent tiles are asserted to share matching
frustum boundaries (the geometric definition of "no seam") — see
`tile.rs`'s tests. What hasn't been done is watching Span mode's actual
seam on real multiple displays; same honesty convention as the Linux X11
verification gap elsewhere in this doc.

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

### Feedback popup ("Report Issue / Feedback…")

A floating (not a dimmed/blocking modal — the rest of the UI stays
interactive behind it) `egui::Window`, opened from a button in the
drawer: category (Bug/Feature request/Question — sets the issue's real
GitHub label via `labels=`, never mangled into the title as text),
separate title + description fields (mirroring GitHub's own issue form,
not one combined text box), and — for Bug only — an on-by-default,
visible "include recent log output" checkbox (see
[LOGGING.md](LOGGING.md) for where that log file lives). Submitting
builds a pre-filled `github.com/.../issues/new` URL and opens it via
`ShellExecuteW` directly (Windows) — deliberately not `cmd /c start`,
which re-parses its command tail through cmd.exe's own shell grammar and
silently truncates any URL at the first `&` (every one of these URLs has
one, at its first query-string separator — a real bug, found by actually
clicking through a generated link, not by reading the code). No account
or token on our end: the person reporting still submits it themselves
once the browser opens.

Log inclusion runs through a best-effort sanitizer that redacts the
current user's home directory path — including the double-backslash
form `tracing`'s `{:?}` (Debug) formatting produces on Windows, which is
what real log lines actually contain and is easy to miss (a sanitizer
that only replaces the raw single-backslash form silently redacts
nothing, with no error to notice — this shipped broken once already for
exactly that reason).

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

### Linux — real rendering code; compiles and lints clean on real CI, unverified at runtime

`pipes-xscreensaver` parses the xscreensaver "hack" invocation contract
(`args.rs`: `-root` or `-window-id <id>`, decimal or hex) with the same
pure/tested approach as the Windows side, and — unlike earlier phases of
this project — actually renders into it. Two pieces made that possible:

- **`pipes_render::Renderer::new` became generic.** It used to take a
  concrete `Arc<winit::window::Window>`; it now takes
  `Arc<W> where W: HasWindowHandle + HasDisplayHandle + Send + Sync +
  'static` (the exact bound `wgpu::Instance::create_surface` itself
  needs — re-exported as `pipes_render::rwh` so callers don't need their
  own direct `raw-window-handle`/`wgpu` dependency just for these types).
  `winit::window::Window` already satisfies this, so `pipes-app`/
  `pipes-settings` are unaffected; `pipes-xscreensaver` supplies its own
  raw X11 handle wrapper instead of a winit window.
- **`x11_target.rs`** owns the X11 side: opens the default display via
  `x11-dl` (Xlib functions loaded with `dlopen` *at runtime*, not linked
  at build time — the reason this compiles on Windows/macOS CI too,
  unlike a hard link-time X11 dependency would), resolves `-root`/
  `-window-id` into a concrete window, selects for
  `StructureNotifyMask` so resizes are observed via `ConfigureNotify`,
  and hand-implements `HasWindowHandle`/`HasDisplayHandle` for the
  `RawWindowHandle::Xlib`/`RawDisplayHandle::Xlib` variants. `main.rs`'s
  Linux branch then runs a plain loop (no winit event loop, since there's
  no winit window here) — step the `Scene` on the tick interval, poll for
  a resize, call `Renderer::render` — the same pipeline `pipes-app` uses.

**What "real" means here, precisely**: every `x11-dl`/`raw-window-handle`
API call was checked against the actual fetched crate source (exact
field names and function signatures, the `XEvent` union's `get_type()`
helper, `XConfigureEvent`'s field layout — not recalled from memory), and
the whole thing compiles and passes `clippy -D warnings` against
`x86_64-unknown-linux-gnu`, both locally (`cargo check --target
x86_64-unknown-linux-gnu`, since a Windows host can't link a Linux
binary but *can* type-check one) and for real on `ubuntu-latest` CI
(which does fully compile and link it, being an actual Linux host). What
that does **not** prove: that a GPU surface actually comes up correctly
inside a window `xscreensaver`'s driver hands us, or even inside a bare
X server — nobody has watched this render. Treat that as the single
biggest open question, same spirit as the CLI-contract caveat below.

`docs/FEATURE_IDEAS.md`'s original research note on xscreensaver's exact
CLI contract was a best-effort reading of third-party ports; the config
XML in `installer/linux/xscreensaver-config/pipes-xscreensaver.xml` is a
step up from that (built from xscreensaver's own real upstream
`hacks/config/pipes.xml` and `hypercube.xml`, fetched and read directly,
not paraphrased), but the driver's actual invocation behavior is still
unconfirmed against a live install.

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
  (`settings_app_candidates()` in `main.rs`) checks locations beyond just
  next to itself (dev/testing, both binaries in the same `target/debug`):
  it also checks `ProgramFiles`, `ProgramW6432`, and `ProgramFiles(x86)`,
  not just one. That redundancy is load-bearing, not defensive
  over-engineering: `wix build` must be passed `-arch x64` (and
  `main.wxs`'s `StandardDirectory` refs must be `ProgramFiles64Folder`/
  `System64Folder`, not the plain 32-bit ones) so that the installer's
  declared platform actually matches the native 64-bit binaries `cargo
  build --release` produces — a real, shipped bug (v0.2.0) where this
  drifted caused `%ProgramFiles%` (as seen by a native 64-bit
  `pipes-app.exe`) to disagree with where WiX had actually put
  `pipes-settings.exe`, so the Settings button silently did nothing.

`.exe`/`.scr` file icons come from a `.ico` embedded as a PE resource
(`build.rs` + the `winres` crate, in both `pipes-app` and
`pipes-settings`, Windows-only) built from the same source image as the
running window's icon (see `pipes-render`'s `app_icon` module above) —
`main.wxs` also points `ARPPRODUCTICON` (the icon Programs and Features
shows) and both Start Menu shortcuts at the same `.ico`.

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

## Linux packages (Phase 4)

`installer/linux/build-deb.sh` builds a real `.deb`, and
`installer/linux/build-appimage.sh` a `pipes-settings`-only AppImage:

- **`.deb`**: `pipes-xscreensaver` → `/usr/libexec/xscreensaver/`
  (Debian's real convention for hack binaries, not a guess — confirmed
  via Debian's own packaging documentation), its xscreensaver config XML
  → `/usr/share/xscreensaver/config/` (this is the actual registration
  mechanism xscreensaver-demo uses to discover a hack and know its
  settings-dialog shape, not just a nice-to-have), `pipes-settings` →
  `/usr/bin/`, a `.desktop` launcher entry, and the existing
  `assets/icon/linux/` hicolor PNG set + scalable SVG (generated back
  when the Windows app icon was built — reused here, not duplicated)
  into `/usr/share/icons/hicolor/`, with `postinst`/`postrm` refreshing
  the icon cache and desktop database (`|| true`, since
  `gtk-update-icon-cache`/`update-desktop-database` are themselves
  optional tools this package has no real reason to hard-depend on).
- **AppImage — deliberately not the hack too.** An AppImage is an
  isolated, non-installed bundle by design; `xscreensaver`'s driver finds
  hacks by locating real files in real system locations, which a portable
  bundle structurally cannot provide no matter how it's built — the same
  category of limitation as a portable `.zip` not being able to register
  itself in Windows' Screen Saver dropdown without a real installer (see
  above). So the AppImage wraps only `pipes-settings`, a genuinely
  standalone app with no such requirement, as a distribution option for
  non-Debian distros.
- **CI structure**: `release.yml`'s `windows-installer` and
  `linux-packages` jobs each build and upload their own artifacts
  independently (own version-patch step, own `cargo build --release`);
  a third `publish-release` job (`needs: [windows-installer,
  linux-packages]`) downloads both artifact sets and does one
  `softprops/action-gh-release` call with everything attached. This
  isn't incidental structure — two jobs each independently calling
  `action-gh-release` for the *same* tag would race to create/update the
  same release concurrently, so the two build jobs upload artifacts and
  only the one downstream job actually touches the release.
- **Validated for real in CI** (`dpkg-deb --info`/`--contents` on the
  actual built `.deb`, on real `ubuntu-latest`), but — same caveat as the
  rendering code above — nobody has installed either package on a real
  machine and watched `xscreensaver-demo` list/run "Neo Pipes", or run
  the AppImage and watched a window render.

## Splash site (`site/`)

A React + Vite + Tailwind CSS v4 landing page, entirely separate from
the Cargo workspace (its own `package.json`/`node_modules`, gitignored) —
deployed to GitHub Pages at the custom domain **neowinpipes.com**
(`site/public/CNAME`; DNS is four `A` + four `AAAA` records at the apex
pointing at GitHub Pages' fixed IPs, plus a `www` `CNAME` to
`brando5393.github.io`, all in a Route 53 hosted zone). `.github/
workflows/deploy-pages.yml` builds and deploys on every push to `main`
that touches `site/**`, via `actions/upload-pages-artifact` +
`actions/deploy-pages` (Pages itself is configured with
`build_type=workflow` via the GitHub API, not the legacy branch-based
source).

Download buttons fetch `api.github.com/repos/.../releases/latest`
client-side and match assets by file extension, rather than
hardcoding filenames — the `.deb`/AppImage names embed the version
number, so a static link would go stale every release.

**This needs to stay in sync with real project changes by hand** — it's
a separate static site, not generated from `docs/*.md` or the wiki, so a
shipped feature worth mentioning to an end user (the feedback popup,
a new platform going from unverified to confirmed-working, etc.) doesn't
appear here automatically. See `docs/DEVELOPMENT.md`'s conventions list.

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
  **Release notes** + **Dismiss** — and, the first time, a native Windows
  toast notification too (`notify.rs`), clickable to bring the window to
  the front; see the "Update notification" subsection below. Clicking
  **Update Now** downloads the `.msi` to a temp file, verifies it against
  GitHub's own SHA-256 digest for that asset (`update::verify_checksum` —
  see `SECURITY.md`), and launches `msiexec /i ... /passive /norestart`
  (skips the wizard's Welcome/License/Finish clicks since the user
  already saw those on first install — but not the UAC prompt, which
  can't be skipped), then exits `pipes-settings` itself so the running
  `.exe` isn't locked while its own file gets replaced. A checksum
  mismatch aborts the update instead of launching the installer, and (a
  real bug found and fixed the same day it was written) any failure on
  this path — download, checksum, or launch — reports back over a
  channel so the banner drops out of "Downloading…" and **Update Now**
  becomes clickable again, instead of leaving the UI stuck forever with
  no way to retry.
- This only checks when `pipes-settings` happens to be open — there's no
  background service polling on a schedule. Given this is a screensaver
  utility people open occasionally to tweak settings, that's judged a
  reasonable cadence; revisit if it isn't in practice.

### Update notification (`pipes-settings::notify`)

Windows-only for now (see `docs/ROADMAP.md` for why: no Linux/macOS
machine to verify a notification actually renders/is clickable, and
`CLAUDE.md`'s conventions rule out shipping unverified platform code).
Fires a real Windows toast (WinRT `ToastNotification`, not just the
in-app banner) the first time a check finds an update, using an
in-process `Activated` event handler to bring the window to the front on
click — deliberately *not* a background/tray app that checks while the
app is closed, which would mean adding an always-running autostart
process purely to reverse the "no background service" design choice
above for very little real benefit. Requires
`installer/main.wxs`'s Start Menu shortcut to carry a
`System.AppUserModel.ID` `ShortcutProperty` matching `notify.rs`'s AUMID
constant exactly — Microsoft's documented requirement for an unpackaged
desktop exe's toast to display at all; without it the WinRT call
succeeds but nothing visibly appears (confirmed by actually running a
bare dev build and watching the screen — real "run what you built"
practice, not something inferred from docs).

A second, separate gotcha shipped in the first version of this and was
only caught by a real user on a real install (clicking the toast did
nothing): the `ToastNotification` object was a local variable inside
`try_notify`, dropped the instant the function returned — but a toast's
`Activated` event only has something to fire on for as long as that
object stays alive. Fixed by returning it wrapped in a `ToastHandle` that
`main.rs` holds in a variable scoped to the whole event loop, not the
function call that created it.

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

Cutting a release is then just: `git tag vX.Y.Z && git push origin
vX.Y.Z`. See [DEVELOPMENT.md](DEVELOPMENT.md) for the full step.

## Logging

Structured, human-readable-first (not JSON-first) `tracing` output, so
`cargo run` output and log files read like sentences, not blobs. Full
event catalog and rationale: [LOGGING.md](LOGGING.md).

## Testing

Unit tests live next to the code they test (`#[cfg(test)] mod tests` in
each `pipes-core` module) and run headlessly — no display, no GPU, no
mocked window. Full test philosophy and how to run things:
[DEVELOPMENT.md](DEVELOPMENT.md).
