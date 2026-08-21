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
  pipes-core/    engine: grid, pipe growth, joints, scene lifecycle.
                 No rendering/windowing deps. Fully unit-tested.
  pipes-app/     Phase 1: headless CLI runner + human-readable logging.
                 Phase 2: adds the wgpu-based windowed renderer.
  (future) pipes-win-scr/    Windows .scr wrapper around pipes-app's engine
  (future) pipes-mac-saver/  macOS .saver (ScreenSaverView) wrapper
  (future) pipes-xscreensaver/ Linux xscreensaver module wrapper
```

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

## `pipes-app`

Phase 1 (current): a headless CLI (`cargo run -p pipes-app -- --ticks N
--seed S`) that runs `Scene` and logs it with `tracing` in human-readable
form — see [LOGGING.md](LOGGING.md). It exists to exercise and observe the
engine end-to-end before there's a renderer to look at.

Phase 2 (planned): the same binary grows a `wgpu` + `winit` window that
turns each `Pipe`'s path + joints into geometry (cylinders for straight
runs, spheres/torus-elbows for joints) and renders it with a chrome-like
material, plus a slowly drifting camera — matching the original's look per
[RESEARCH.md](RESEARCH.md). `wgpu` was chosen over raw OpenGL/Direct3D
because one API target compiles to Vulkan (Linux), Metal (macOS), and
DirectX12/Vulkan (Windows), which is what makes one Rust codebase realistic
across all three OSes.

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
