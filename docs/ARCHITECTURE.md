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

## Native screensaver wrappers (Phase 3, not yet started)

Each OS has a different screensaver contract:

- **Windows**: a renamed `.scr` PE executable, invoked with `/s`
  (fullscreen), `/c` (config dialog), `/p <HWND>` (preview thumbnail in
  Settings). Selectable once the `.scr` is placed in `System32` (or
  registered) — installed via an `.msi`.
- **macOS**: a `.saver` bundle implementing `ScreenSaverView`
  (Objective-C/Swift interop from Rust, or a thin native shim that embeds
  the Rust engine as a static library), installed into
  `~/Library/Screen Savers` or `/Library/Screen Savers`.
- **Linux**: no single standard — the common target is an `xscreensaver`
  "hack" (a plain executable xscreensaver `exec`s into a window ID it
  hands you), packaged as a `.deb`/AppImage that registers itself in
  `/usr/share/xscreensaver/config/`.

Each wrapper's job is to be a *thin* platform-specific shell — handle the
OS's window-embedding/preview/config contract, then hand off to the exact
same `pipes-core` `Scene` and the exact same `pipes-app` rendering code
Phase 2 builds. This is why the core/app split above isn't optional
future-proofing — it's the only way three native wrappers stay thin instead
of forking the whole engine three times.

## Logging

Structured, human-readable-first (not JSON-first) `tracing` output, so
`cargo run` output and log files read like sentences, not blobs. Full
event catalog and rationale: [LOGGING.md](LOGGING.md).

## Testing

Unit tests live next to the code they test (`#[cfg(test)] mod tests` in
each `pipes-core` module) and run headlessly — no display, no GPU, no
mocked window. Full test philosophy and how to run things:
[DEVELOPMENT.md](DEVELOPMENT.md).
