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

- ~~**Multi-monitor behavior that's actually configurable**~~ — **shipped**: a three-way Pipes Settings toggle under "Multi-monitor" — `MonitorMode::AllMonitors` (default, one independent instance per display), `MonitorMode::Span` (one shared scene, rendered via an off-axis per-monitor projection so pipes visually travel from one display onto the next), and `MonitorMode::PrimaryOnly`. See `docs/ARCHITECTURE.md#multi-monitor-behavior` for the tile-projection technique and `docs/ROADMAP.md` for the verification caveat (the projection math is precisely unit-tested; the actual seam hasn't been watched on real multiple displays).
- **High-DPI correctness** — not found as an explicit complaint in research, but implied by "modern systems" in the project's original goal; worth a deliberate check once real displays are being tested against, not just assumed.

## Ideas not yet validated by outside sources (ours, lower confidence)

- ~~Preset "themes" bundling palette + style + speed together~~ — **shipped**: "Classic '96"/"Neon"/"Monochrome" one-click bundles in the Themes row.
- A pause/resume hotkey in the live preview (distinct from the runtime keyboard shortcuts `pipes.sh` has for adjusting parameters). Still open — not picked when the rest of this batch was chosen.
- ~~Config export/import~~ — **shipped**: "Export…"/"Import…" buttons using a native file dialog (`rfd`).
- ~~The classic screensaver's teapot easter egg~~ — **shipped**: a rare, separate roll (`JointKind::Teapot`, `teapot_easter_egg_enabled` + `teapot_probability`) renders a procedural teapot mesh (`pipes_render::geometry::teapot()` — lathed body/spout, torus-arc handle, sphere knob; not the exact historical Utah teapot control-point dataset, an honest approximation) at a joint instead of the normal ball/elbow.
- ~~Multi-monitor configurable behavior (promoted up from "Modern-platform expectations" below)~~ — **shipped**, see above.

## From the real original source (verified 2026-08-23, not clone-inferred)

Reading the actual 1994-95 OpenGL C++ source directly (see
`docs/RESEARCH.md`'s "What the real original source actually does"
section) turned up several real original features neo_win_pipes doesn't
have, ranked by my own read of effort vs. payoff:

1. **Per-pipe randomized straight/turn "personality" instead of one fixed
   global ratio** — highest value, lowest effort of this batch. The
   original rolls each pipe as either a rare (1-in-20) long-straight-runner
   or a common turny/zigzaggy pipe at spawn time, not one fixed ratio
   applied uniformly to every pipe every step the way `straight_weight`/
   `turn_weight` work today. This is a `Pipe`-construction-time change in
   `pipes-core` (no new rendering, no new config surface required to get
   the *behavior* right, though it could still stay tunable) and would
   make the simulation's pacing look more like the real original's mix of
   long runs and busy clusters, rather than uniform. Worth doing.
2. **A "Cycle" joint type**, distinct from "Mixed" — cycles through joint
   styles deterministically rather than randomizing per joint. Cheap:
   `JointKind` already exists, this is one more variant and a spawn-order
   counter, similar in shape to the already-shipped
   `lock_colors_across_resets` deterministic-cycling logic.
3. **A tessellation-quality slider** — real original had one
   (`fTesselFact`, 0.0-2.0). Moderate effort: today's segment/cylinder
   resolution is a fixed constant in `pipes-render`; exposing it means
   plumbing a quality knob through geometry generation and validating it
   doesn't blow up instance buffer sizes at the high end.
4. **A wireframe rendering mode** — a real, distinct `SURFSTYLE_WIREFRAME`
   option in the original, not something we or any clone surveyed has.
   Moderate effort (a different `MeshBasicMaterial`-equivalent/fill mode
   per pipe style), clearly-scoped, and cheap to make optional.
5. **Textured pipe surfaces** (`SURFSTYLE_TEX`, up to 8 textures, a
   quality toggle) — real, but meaningfully more work: texture loading,
   UV mapping along the pipe's cylindrical surface, and a way to ship or
   let users supply texture assets. Lower priority than the above; solid
   chrome-style materials are already the more iconic/recognizable look
   anyway.
6. **"Flex" pipes** (continuously-curved NURBS-bent tube geometry instead
   of rigid segments + joints) — the most ambitious item found. A
   genuinely different rendering mode from the segment/joint model this
   whole codebase (and every clone surveyed) is built around; would mean
   new curve-evaluation geometry code in `pipes-render`, not a small
   tweak. Interesting, but I'd treat this as a "someday" idea, not a
   near-term one, given the size of the lift relative to the other items
   here.

## My own assessment, for what it's worth (2026-08-23 review)

Asked to review the repo and record genuine opinions, not just relay
sourced facts, alongside the research above:

- **The single most valuable near-term feature-idea item is #1 above**
  (per-pipe randomized personality). It's a real, sourced, small
  simulation-logic change that would visibly change how the default
  simulation looks and feels closer to the actual original, for very
  little implementation risk — the kind of change that's easy to get
  right and easy to unit-test (`pipes-core` already has strong test
  discipline per `CLAUDE.md`'s founding requirement).
- **Everything else I looked at in `crates/` looked genuinely solid.** No
  `TODO`/`FIXME`/`XXX` markers anywhere in the Rust source, which either
  means the team (well - just this AI-assisted project, but still)
  doesn't leave loose threads lying around, or resolves them before
  moving on — either way, a good sign for a repo this size.
- **The GPU device-loss crash already logged in `docs/ROADMAP.md`**
  (found 2026-08-23 testing the color-palette feature on Windows-on-ARM64)
  remains the most concrete *robustness* gap I know of in the actual
  shipped code, separate from the feature ideas above. I'd rank fixing
  that above any of the new-feature items here if forced to choose,
  simply because it's a real crash a real user hit, not a hypothetical.
- I did not find anything in the original developers' own words (beyond
  the origin story itself) about features they wished they'd shipped or
  regretted not doing — that specific angle of the research request came
  up empty across every source I checked, and I'd rather say so plainly
  than invent something plausible-sounding.
- I'm intentionally not promoting any of the six items above into
  `docs/ROADMAP.md` myself — per this file's own stated purpose ("items
  get promoted into ROADMAP.md phases when someone decides to build
  them"), that's a call for the project owner to make, not something to
  do unilaterally just because research turned it up.

## Where this feeds in

The v1 settings app (in progress) starts from the *validated* list above:
pipe style & count, speed, camera behavior, color palette, and grid
size/reset threshold. Multi-monitor and the "keep palette across resets"
toggle are natural next additions once v1 ships.
