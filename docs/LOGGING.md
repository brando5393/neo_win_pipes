# Logging

## Why human-readable-first

This project logs for a person reading a terminal or a log file, not for a
log-aggregation pipeline. Every event is one line, in plain English, with
`key=value` fields appended — not JSON. If a machine-readable format is
ever needed (e.g. for a future crash-report uploader), that's an additive
second output, not a replacement — human-readable stays the default. This
mirrors how the project treats documentation generally: legible first.

We use the [`tracing`](https://docs.rs/tracing) crate for structured
events and [`tracing-subscriber`](https://docs.rs/tracing-subscriber)'s
`fmt` layer (compact, non-JSON formatter) to render them.

## How to see logs

```sh
# default: info level and above
cargo run -p pipes-app

# everything, including per-pipe spawn/turn/terminate detail
RUST_LOG=debug cargo run -p pipes-app -- --ticks 500 --seed 1

# PowerShell
$env:RUST_LOG = "debug"; cargo run -p pipes-app -- --ticks 500 --seed 1
```

`RUST_LOG` follows `tracing-subscriber`'s standard `EnvFilter` syntax (e.g.
`RUST_LOG=pipes_core=debug,pipes_app=info` to set levels per crate).

## Example output

```
2026-08-21T02:14:41.054249Z  INFO neo_win_pipes starting (headless simulation) ticks=60 seed=5
2026-08-21T02:14:41.057357Z  INFO scene created seed=5 config.bounds=GridBounds { width: 24, height: 16, depth: 24 } max_pipes=6
2026-08-21T02:14:41.059156Z DEBUG pipe spawned pipe_id=0 p=GridPos { x: 4, y: 0, z: 23 } dir=NegZ style=Square
2026-08-21T02:14:41.059533Z DEBUG pipe spawned pipe_id=1 p=GridPos { x: 20, y: 15, z: 0 } dir=NegZ style=Round
2026-08-21T02:14:41.060247Z  INFO tick summary tick=0 live_pipes=6 occupancy=0.0006510416860692203
2026-08-21T02:14:41.063394Z  INFO neo_win_pipes finished elapsed_ms=6 total_pipes_spawned=6 total_pipes_terminated=0 total_resets=0
```

Every line reads as a sentence: what happened, to what, with what
identifying details, at what time.

## Event catalog

| Event                | Level | Emitted from            | Fields                                              | Meaning |
|-----------------------|-------|--------------------------|------------------------------------------------------|---------|
| `scene created`       | INFO  | `Scene::new`             | `seed`, `config.bounds`, `max_pipes`                 | A new simulation started (or restarted). |
| `pipe spawned`        | DEBUG | `Scene::try_spawn_pipe`  | `pipe_id`, `p` (start position), `dir`, `style`       | A new pipe began growing. |
| `pipe terminated`     | DEBUG | `Scene::step`            | `pipe_id`, `reason` (`Stuck`/`MaxLengthReached`), `len` | A pipe stopped growing and will be reaped/replaced. |
| `scene dissolving (grid filled)` | INFO | `Scene::step` | `tick`, `ratio`, `total_ticks`                        | The grid crossed `reset_occupancy_ratio` and `dissolve_on_reset` is enabled; pipes now shrink away over `total_ticks` before the actual clear (see `scene reset (dissolve complete)`). Growth is frozen for the duration. |
| `scene reset (grid filled)` | INFO | `Scene::step`      | `tick`, `ratio`                                       | The grid crossed `reset_occupancy_ratio` and `dissolve_on_reset` is disabled; cleared immediately (no dissolve). |
| `scene reset (dissolve complete)` | INFO | `Scene::step` | `tick`                                                | The dissolve countdown from `scene dissolving (grid filled)` reached zero; everything cleared and restarts. |
| `tick summary`        | INFO  | `pipes-app` main loop    | `tick`, `live_pipes`, `occupancy`                     | Periodic (every 50 ticks) heartbeat so a long run is still legible without DEBUG noise. |
| `neo_win_pipes starting` / `finished` | INFO | `pipes-app` main | `ticks`, `seed` / `elapsed_ms`, totals | Process lifecycle bookends. |

## Levels, and when to use which

- **INFO**: state changes worth seeing by default — scene lifecycle,
  periodic summaries, process start/stop. This is what a user watching the
  screensaver's log would want.
- **DEBUG**: per-pipe detail (spawn, turn, terminate). Useful when
  diagnosing a specific pipe's behavior or verifying the growth algorithm,
  noisy for normal use.
- **TRACE**: reserved for future per-step geometry/render detail (Phase 2)
  — not used yet.
- **WARN/ERROR**: reserved for genuine problems (e.g. Phase 2 renderer
  failing to acquire a GPU device, Phase 3 wrapper failing its OS
  contract). Not used in Phase 1 since the headless simulation has no
  failure modes beyond programmer error, which should panic loudly in
  debug/test builds rather than log-and-continue.

## Design rule for new events

When adding an event: write the message as a short plain-English sentence
fragment (not `PIPE_SPAWN_EVENT` or similar constant-cased identifiers),
attach identifying fields (`pipe_id`, position, etc.) so a reader can
follow one pipe's story across lines, and add a row to the table above in
the same change. An event with no entry in this table is undocumented and
should be treated as a gap to fix, not a formality to skip.
