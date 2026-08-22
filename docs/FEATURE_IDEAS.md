# Feature ideas (community-sourced backlog)

A running wishlist, not a commitment — items get promoted into
[ROADMAP.md](ROADMAP.md) phases when someone decides to build them. Sourced
from actual user requests on prior pipes-screensaver projects (not just our
own guesses), so we're building toward things people have actually asked
for, not just what seemed neat.

## Sources consulted

- [`pipeseroni/pipes.sh`](https://github.com/pipeseroni/pipes.sh) ([man page](https://pipeseroni.github.io/pipes.sh/pipes.sh.6.html)) — a mature, actively-configured terminal pipes clone. Its option surface is the single best evidence of "what do people actually want to tune" for this exact genre of screensaver, since it's had years of real usage and PRs.
- [`1j01/pipes`](https://github.com/1j01/pipes/issues) — a web-based 3D clone; its open issues are direct, unfiltered user requests/complaints.
- General screensaver multi-monitor research (Actual Tools, DisplayFusion, Microsoft Q&A threads) — evidence that multi-monitor behavior is a long-standing, still-unresolved pain point across the whole screensaver category, not specific to pipes.

## Settings/tuning requests (validated — already informing our v1 settings app)

- **Number of concurrent pipes**, adjustable — requested directly ([1j01/pipes #6](https://github.com/1j01/pipes/issues/6): "how do i add more pipes going at the same time"); `pipes.sh` exposes this as `-p`.
- **Speed control** — requested directly ([1j01/pipes #4](https://github.com/1j01/pipes/issues/4): "Pipe speed is too rapid"); `pipes.sh` exposes frame rate (`-f`) and has live keyboard shortcuts to adjust it without restarting.
- **"Probability of straight vs. turn"** as a first-class tunable, not just a fixed internal constant — `pipes.sh -s` exposes exactly this (our `straight_weight`/`turn_weight` ratio). **Shipped**: "Pipe behavior" section exposes `straight_weight`/`turn_weight`/`elbow_probability` sliders.
- **Color set selection**, including the ability to pick specific colors, not just "random" — `pipes.sh -c` takes a list of color indices. Shipped as palette presets + custom per-color editing.
- **Pipe style selection** (`pipes.sh -t` has 10 built-in styles plus fully custom glyph sets) — our analog is `PipeStyle`/joint style; validates giving the user real style choice rather than a fixed 50/50 mix. Shipped as the Round/Square/Mixed radio.
- **Reset behavior control** — `pipes.sh -r` sets how full the screen gets before clearing (shipped as `reset_occupancy_ratio`), and `-K` locks color/style across a reset instead of randomizing. **Shipped**: `lock_colors_across_resets` toggle — when on, color/style assignment cycles deterministically by spawn order instead of randomizing, so every generation reproduces the identical pattern (see `Scene::spawn_index_this_generation`).
- **Live/runtime adjustment**, not just a config screen you set once and restart — `pipes.sh` supports adjusting several of the above via keyboard shortcuts *while it's running*. Worth keeping in mind for the settings app's live-preview design: changes should apply immediately, not require a restart.

## Stability/performance cautions (learn from others' bug reports)

- **Memory growth over long runs** — [1j01/pipes #11](https://github.com/1j01/pipes/issues/11) reports unbounded memory growth ("memory gets full after a few minutes"), plausibly from that clone never freeing old geometry across resets. Directly relevant to us: `Scene::reset()` must actually drop old `Pipe` data (it does — `Vec::clear()`), and the renderer must not accumulate GPU buffers frame over frame (current design recreates instance buffers per frame and lets old ones drop — worth a deliberate long-run memory check once Phase 2 stabilizes, precisely because another project got bitten by this exact failure mode).

## Modern-platform expectations (not present in the original, but expected of anything shipping today)

- ~~**Multi-monitor behavior that's actually configurable**~~ — **shipped**: `MonitorMode::AllMonitors` (default, one independent instance per display) vs. `MonitorMode::PrimaryOnly`, a Pipes Settings toggle under "Multi-monitor". Decided in favor of independent per-display instances over one spanning canvas — see `docs/ARCHITECTURE.md#multi-monitor-behavior` for the reasoning and `docs/ROADMAP.md` for the verification caveat (unit-tested and code-reviewed, not yet watched on real multiple displays).
- **High-DPI correctness** — not found as an explicit complaint in research, but implied by "modern systems" in the project's original goal; worth a deliberate check once real displays are being tested against, not just assumed.

## Ideas not yet validated by outside sources (ours, lower confidence)

- ~~Preset "themes" bundling palette + style + speed together~~ — **shipped**: "Classic '96"/"Neon"/"Monochrome" one-click bundles in the Themes row.
- A pause/resume hotkey in the live preview (distinct from the runtime keyboard shortcuts `pipes.sh` has for adjusting parameters). Still open — not picked when the rest of this batch was chosen.
- ~~Config export/import~~ — **shipped**: "Export…"/"Import…" buttons using a native file dialog (`rfd`).
- ~~The classic screensaver's teapot easter egg~~ — **shipped**: a rare, separate roll (`JointKind::Teapot`, `teapot_easter_egg_enabled` + `teapot_probability`) renders a procedural teapot mesh (`pipes_render::geometry::teapot()` — lathed body/spout, torus-arc handle, sphere knob; not the exact historical Utah teapot control-point dataset, an honest approximation) at a joint instead of the normal ball/elbow.
- ~~Multi-monitor configurable behavior (promoted up from "Modern-platform expectations" below)~~ — **shipped**, see above.

## Where this feeds in

The v1 settings app (in progress) starts from the *validated* list above:
pipe style & count, speed, camera behavior, color palette, and grid
size/reset threshold. Multi-monitor and the "keep palette across resets"
toggle are natural next additions once v1 ships.
