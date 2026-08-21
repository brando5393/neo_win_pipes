use rand::Rng;
use rand_pcg::Pcg32;
use tracing::{debug, info};

use crate::direction::Direction;
use crate::grid::{GridBounds, GridPos, OccupancyGrid};
use crate::pipe::{Color, Pipe, PipeStyle, PipeStyleMode, StepOutcome};

/// Tunable knobs for one simulation. Defaults aim for a "faithful classic"
/// look; renderer front-ends may expose these as user-facing settings — see
/// `docs/FEATURE_IDEAS.md` for which of these were validated by looking at
/// what users of prior pipes-screensaver projects actually asked for.
/// `#[serde(default)]` here is load-bearing, not decoration: it makes
/// every field individually forward-compatible — a config file saved by
/// an older version (missing whatever field got added since) still loads
/// with its other settings intact, falling back to `SimConfig::default()`
/// only for the fields that are actually absent, rather than the whole
/// struct failing to parse and silently discarding everything (which is
/// what would happen without this, and did — caught while testing the
/// dissolve feature against a config file saved before it existed).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SimConfig {
    pub bounds: GridBounds,
    pub max_pipes: usize,
    /// Relative weight of continuing straight vs. turning at each step.
    /// The original favors long straight runs, so straight >> turn.
    pub straight_weight: u32,
    pub turn_weight: u32,
    /// Probability a turn renders as a smooth elbow rather than a ball
    /// joint. ~0.75 matches observed behavior of the original.
    pub elbow_probability: f32,
    /// Once the grid is this full, the scene clears and starts over.
    pub reset_occupancy_ratio: f32,
    pub max_pipe_length: usize,
    /// How many random free cells to try before giving up on spawning a
    /// pipe this tick (the grid may be nearly full).
    pub spawn_attempts: u32,
    /// Which pipe style(s) get spawned.
    pub style_mode: PipeStyleMode,
    /// Colors newly-spawned pipes are randomly drawn from. Must be
    /// non-empty; `Scene::new` falls back to `default_palette()` if given
    /// an empty list (e.g. from a hand-edited config file).
    pub palette: Vec<Color>,
    /// Validated by `pipes.sh`'s `-K` flag (see `docs/FEATURE_IDEAS.md`):
    /// when `false` (default, matching the original's — and `pipes.sh`'s
    /// own default — fully-random look), each spawn independently rolls a
    /// random color/style. When `true`, both cycle deterministically by
    /// spawn order instead, so every generation reproduces the *same*
    /// color/style pattern rather than a fresh random one each reset.
    pub lock_colors_across_resets: bool,
    /// When the grid fills up, pipes shrink away over
    /// `dissolve_duration_ticks` before the scene clears and restarts —
    /// matching the original screensaver's transition — rather than
    /// vanishing instantly. Toggleable per user preference.
    pub dissolve_on_reset: bool,
    /// How many ticks the dissolve-away animation takes. Only meaningful
    /// when `dissolve_on_reset` is true.
    pub dissolve_duration_ticks: u32,
    /// The classic screensaver's teapot easter egg: a rare, separate roll
    /// (see `JointKind::Teapot`) that occasionally renders a Utah teapot
    /// at a joint instead of the normal ball/elbow. Toggleable; when
    /// `false`, a teapot can never occur regardless of `teapot_probability`.
    pub teapot_easter_egg_enabled: bool,
    /// Probability a turn becomes a teapot joint instead of ball/elbow,
    /// checked before the elbow/ball roll. Kept small and rare to match
    /// how sparingly the original used it. Only takes effect when
    /// `teapot_easter_egg_enabled` is true.
    pub teapot_probability: f32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            bounds: GridBounds::new(24, 16, 24),
            max_pipes: 6,
            straight_weight: 10,
            turn_weight: 1,
            elbow_probability: 0.75,
            reset_occupancy_ratio: 0.35,
            max_pipe_length: 400,
            spawn_attempts: 64,
            style_mode: PipeStyleMode::default(),
            palette: default_palette(),
            lock_colors_across_resets: false,
            dissolve_on_reset: true,
            dissolve_duration_ticks: 15,
            teapot_easter_egg_enabled: true,
            teapot_probability: 0.02,
        }
    }
}

/// The bright, saturated palette in the spirit of the original's
/// chrome-and-neon pipe colors, rather than a photorealistic random hue.
/// Public so front-ends (e.g. a settings app) can offer "reset to classic
/// palette" without duplicating the color list.
pub fn default_palette() -> Vec<Color> {
    vec![
        Color {
            r: 0.85,
            g: 0.15,
            b: 0.15,
        }, // red
        Color {
            r: 0.15,
            g: 0.55,
            b: 0.85,
        }, // blue
        Color {
            r: 0.15,
            g: 0.75,
            b: 0.35,
        }, // green
        Color {
            r: 0.95,
            g: 0.75,
            b: 0.10,
        }, // gold
        Color {
            r: 0.70,
            g: 0.20,
            b: 0.85,
        }, // purple
        Color {
            r: 0.85,
            g: 0.85,
            b: 0.90,
        }, // chrome/silver
    ]
}

/// A notable event a caller (renderer, logger, tests) might care about.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneEvent {
    PipeSpawned {
        id: u32,
    },
    PipeTerminated {
        id: u32,
    },
    /// The grid filled up and the dissolve-away animation began (only
    /// fires when `dissolve_on_reset` is enabled; otherwise the scene
    /// clears immediately and only `SceneReset` fires).
    DissolveStarted,
    SceneReset,
}

/// Where the scene is in its fill/clear cycle. Growth pauses during
/// `Dissolving` — nothing spawns or steps further while pipes are
/// shrinking away, since the whole point is a clean visual break before
/// the next cycle starts.
enum ScenePhase {
    Growing,
    Dissolving {
        ticks_remaining: u32,
        total_ticks: u32,
    },
}

/// Owns the occupancy grid and the set of live pipes, and drives the
/// simulation forward one tick at a time. Deterministic given a seed, which
/// is what makes this whole crate unit-testable without a renderer.
pub struct Scene {
    config: SimConfig,
    grid: OccupancyGrid,
    pipes: Vec<Pipe>,
    next_id: u32,
    rng: Pcg32,
    tick: u64,
    phase: ScenePhase,
    /// Resets to 0 each generation (see `reset`); used instead of `rng`
    /// for color/style selection when `lock_colors_across_resets` is on,
    /// so every generation reproduces the same pattern by spawn order.
    spawn_index_this_generation: u32,
}

impl Scene {
    pub fn new(config: SimConfig, seed: u64) -> Self {
        let grid = OccupancyGrid::new(config.bounds);
        info!(seed, ?config.bounds, max_pipes = config.max_pipes, "scene created");
        Self {
            config,
            grid,
            pipes: Vec::new(),
            next_id: 0,
            rng: Pcg32::new(seed, 0x0a02_bdbf_7bb3_c0a7),
            tick: 0,
            phase: ScenePhase::Growing,
            spawn_index_this_generation: 0,
        }
    }

    pub fn pipes(&self) -> &[Pipe] {
        &self.pipes
    }

    pub fn grid(&self) -> &OccupancyGrid {
        &self.grid
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// `Some(progress)` in `[0.0, 1.0]` while the scene is dissolving away
    /// after filling up (0.0 = just started, approaching 1.0 = about to
    /// clear); `None` while pipes are growing normally. Renderers use this
    /// to shrink pipe/joint visuals proportionally — see
    /// `pipes-render::instance`.
    pub fn dissolve_progress(&self) -> Option<f32> {
        match self.phase {
            ScenePhase::Growing => None,
            ScenePhase::Dissolving {
                ticks_remaining,
                total_ticks,
            } => {
                let elapsed = total_ticks.saturating_sub(ticks_remaining);
                Some(elapsed as f32 / total_ticks.max(1) as f32)
            }
        }
    }

    /// Advance the whole scene by one tick. While `Growing`: step every
    /// live pipe, reap dead ones, spawn replacements up to `max_pipes`,
    /// and — once the grid fills past the configured threshold — either
    /// clear immediately (`SceneReset`) or, if `dissolve_on_reset` is
    /// enabled, enter the `Dissolving` phase instead (`DissolveStarted`)
    /// and freeze growth. While `Dissolving`: nothing grows; once the
    /// countdown reaches zero, clear and emit `SceneReset`. Returns the
    /// events that occurred, in order, for callers that want to log or
    /// react to them.
    pub fn step(&mut self) -> Vec<SceneEvent> {
        let mut events = Vec::new();
        self.tick += 1;

        if let ScenePhase::Dissolving {
            ticks_remaining,
            total_ticks,
        } = self.phase
        {
            if ticks_remaining <= 1 {
                info!(tick = self.tick, "scene reset (dissolve complete)");
                self.reset();
                events.push(SceneEvent::SceneReset);
            } else {
                self.phase = ScenePhase::Dissolving {
                    ticks_remaining: ticks_remaining - 1,
                    total_ticks,
                };
            }
            return events;
        }

        for pipe in &mut self.pipes {
            if !pipe.is_alive() {
                continue;
            }
            let teapot_probability = if self.config.teapot_easter_egg_enabled {
                self.config.teapot_probability
            } else {
                0.0
            };
            let outcome = pipe.step(
                &mut self.grid,
                &mut self.rng,
                self.config.straight_weight,
                self.config.turn_weight,
                self.config.elbow_probability,
                teapot_probability,
                self.config.max_pipe_length,
            );
            if let StepOutcome::Terminated(reason) = outcome {
                debug!(
                    pipe_id = pipe.id,
                    ?reason,
                    len = pipe.len(),
                    "pipe terminated"
                );
                events.push(SceneEvent::PipeTerminated { id: pipe.id });
            }
        }

        while self.pipes.iter().filter(|p| p.is_alive()).count() < self.config.max_pipes {
            match self.try_spawn_pipe() {
                Some(id) => events.push(SceneEvent::PipeSpawned { id }),
                None => break,
            }
        }

        if self.grid.occupancy_ratio() >= self.config.reset_occupancy_ratio {
            if self.config.dissolve_on_reset {
                let total_ticks = self.config.dissolve_duration_ticks.max(1);
                info!(
                    tick = self.tick,
                    ratio = self.grid.occupancy_ratio(),
                    total_ticks,
                    "scene dissolving (grid filled)"
                );
                self.phase = ScenePhase::Dissolving {
                    ticks_remaining: total_ticks,
                    total_ticks,
                };
                events.push(SceneEvent::DissolveStarted);
            } else {
                info!(
                    tick = self.tick,
                    ratio = self.grid.occupancy_ratio(),
                    "scene reset (grid filled)"
                );
                self.reset();
                events.push(SceneEvent::SceneReset);
            }
        }

        events
    }

    fn try_spawn_pipe(&mut self) -> Option<u32> {
        let bounds = self.config.bounds;
        for _ in 0..self.config.spawn_attempts {
            let p = GridPos::new(
                self.rng.gen_range(0..bounds.width),
                self.rng.gen_range(0..bounds.height),
                self.rng.gen_range(0..bounds.depth),
            );
            if !self.grid.is_free(p) {
                continue;
            }
            let dir = Direction::ALL[self.rng.gen_range(0..Direction::ALL.len())];
            let spawn_index = self.spawn_index_this_generation;
            let locked = self.config.lock_colors_across_resets;
            let style = match self.config.style_mode {
                PipeStyleMode::Round => PipeStyle::Round,
                PipeStyleMode::Square => PipeStyle::Square,
                PipeStyleMode::Mixed => {
                    let round = if locked {
                        spawn_index.is_multiple_of(2)
                    } else {
                        self.rng.gen_bool(0.5)
                    };
                    if round {
                        PipeStyle::Round
                    } else {
                        PipeStyle::Square
                    }
                }
            };
            let palette_index = |rng: &mut Pcg32, len: usize| {
                if locked {
                    spawn_index as usize % len
                } else {
                    rng.gen_range(0..len)
                }
            };
            let color = if self.config.palette.is_empty() {
                let fallback = default_palette();
                let i = palette_index(&mut self.rng, fallback.len());
                fallback[i]
            } else {
                let i = palette_index(&mut self.rng, self.config.palette.len());
                self.config.palette[i]
            };
            let id = self.next_id;
            self.next_id += 1;
            self.spawn_index_this_generation += 1;
            self.grid.occupy(p);
            debug!(pipe_id = id, ?p, ?dir, ?style, "pipe spawned");
            self.pipes.push(Pipe::new(id, style, color, p, dir));
            return Some(id);
        }
        None
    }

    fn reset(&mut self) {
        self.grid.clear();
        self.pipes.clear();
        self.phase = ScenePhase::Growing;
        self.spawn_index_this_generation = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_config() -> SimConfig {
        SimConfig {
            bounds: GridBounds::new(6, 6, 6),
            max_pipes: 2,
            straight_weight: 10,
            turn_weight: 1,
            elbow_probability: 0.75,
            reset_occupancy_ratio: 0.5,
            max_pipe_length: 20,
            spawn_attempts: 64,
            ..SimConfig::default()
        }
    }

    #[test]
    fn scene_spawns_up_to_max_pipes() {
        let mut scene = Scene::new(tiny_config(), 1);
        scene.step();
        assert!(scene.pipes().iter().filter(|p| p.is_alive()).count() <= 2);
        assert!(!scene.pipes().is_empty());
    }

    #[test]
    fn scene_resets_once_occupancy_threshold_hit() {
        let mut scene = Scene::new(tiny_config(), 2);
        let mut saw_reset = false;
        for _ in 0..500 {
            let events = scene.step();
            if events.contains(&SceneEvent::SceneReset) {
                saw_reset = true;
                break;
            }
        }
        assert!(
            saw_reset,
            "a 6x6x6 grid with 50% threshold must eventually trigger a reset"
        );
        assert_eq!(
            scene.grid().occupied_count(),
            0,
            "reset must clear the grid"
        );
    }

    #[test]
    fn scene_never_exceeds_bounds_cell_count() {
        let mut scene = Scene::new(tiny_config(), 3);
        for _ in 0..1000 {
            scene.step();
            assert!(scene.grid().occupied_count() <= scene.grid().bounds().cell_count());
        }
    }

    #[test]
    fn same_seed_produces_identical_tick_sequence() {
        let run = || {
            let mut scene = Scene::new(tiny_config(), 99);
            let mut all_events = Vec::new();
            for _ in 0..50 {
                all_events.extend(scene.step());
            }
            all_events
        };
        assert_eq!(
            run(),
            run(),
            "identical seed must reproduce identical event sequence"
        );
    }

    #[test]
    fn dissolve_progress_is_none_while_growing() {
        let scene = Scene::new(tiny_config(), 1);
        assert_eq!(scene.dissolve_progress(), None);
    }

    #[test]
    fn dissolve_starts_freezes_growth_then_resets() {
        let config = SimConfig {
            dissolve_on_reset: true,
            dissolve_duration_ticks: 4,
            ..tiny_config()
        };
        let mut scene = Scene::new(config, 2);

        let mut saw_dissolve_start = false;
        let mut occupied_at_dissolve_start = None;
        for _ in 0..500 {
            let events = scene.step();
            if events.contains(&SceneEvent::DissolveStarted) {
                saw_dissolve_start = true;
                occupied_at_dissolve_start = Some(scene.grid().occupied_count());
                assert!(
                    scene.dissolve_progress().is_some(),
                    "dissolve_progress must be Some right after DissolveStarted"
                );
                break;
            }
        }
        assert!(
            saw_dissolve_start,
            "a 6x6x6 grid at 50% threshold must eventually start dissolving"
        );

        // While dissolving, the grid must stay frozen (no more growth) —
        // nothing should spawn or step until the dissolve completes.
        let occupied_before = occupied_at_dissolve_start.unwrap();
        let mut saw_reset = false;
        for _ in 0..10 {
            let events = scene.step();
            assert!(
                !events.iter().any(|e| matches!(
                    e,
                    SceneEvent::PipeSpawned { .. } | SceneEvent::PipeTerminated { .. }
                )),
                "no pipe growth/termination events should occur while dissolving"
            );
            if events.contains(&SceneEvent::SceneReset) {
                saw_reset = true;
                break;
            }
            assert_eq!(
                scene.grid().occupied_count(),
                occupied_before,
                "grid must stay frozen while dissolving"
            );
        }
        assert!(saw_reset, "dissolve must eventually complete and reset");
        assert_eq!(
            scene.dissolve_progress(),
            None,
            "dissolve_progress must be None again after reset"
        );
        assert_eq!(
            scene.grid().occupied_count(),
            0,
            "reset must clear the grid"
        );
    }

    #[test]
    fn dissolve_disabled_resets_immediately_like_before() {
        let config = SimConfig {
            dissolve_on_reset: false,
            ..tiny_config()
        };
        let mut scene = Scene::new(config, 2);

        let mut saw_reset = false;
        for _ in 0..500 {
            let events = scene.step();
            assert!(
                !events.contains(&SceneEvent::DissolveStarted),
                "DissolveStarted must never fire when disabled"
            );
            if events.contains(&SceneEvent::SceneReset) {
                saw_reset = true;
                break;
            }
        }
        assert!(saw_reset);
        assert_eq!(scene.grid().occupied_count(), 0);
        assert_eq!(scene.dissolve_progress(), None);
    }

    #[test]
    fn locked_colors_reproduce_the_same_pattern_every_generation() {
        let config = SimConfig {
            lock_colors_across_resets: true,
            dissolve_on_reset: false,
            style_mode: PipeStyleMode::Mixed,
            ..tiny_config()
        };
        let mut scene = Scene::new(config, 7);

        let mut generations: Vec<Vec<(PipeStyle, Color)>> = vec![Vec::new()];
        for _ in 0..500 {
            let events = scene.step();
            // A spawn and a reset can land in the same tick (a fresh spawn's
            // cell can itself tip occupancy over the threshold); by the
            // time `step()` returns, `reset()` has already cleared
            // `self.pipes`, so that spawn's data isn't recoverable from
            // `scene.pipes()` anymore. Skip such ticks rather than inspect
            // pipes that are already gone — there are plenty of other
            // ticks to gather the pattern from.
            let reset_this_tick = events.contains(&SceneEvent::SceneReset);
            for event in &events {
                if let SceneEvent::PipeSpawned { id } = event {
                    if reset_this_tick {
                        continue;
                    }
                    let pipe = scene
                        .pipes()
                        .iter()
                        .find(|p| p.id == *id)
                        .expect("just-spawned pipe must exist");
                    generations
                        .last_mut()
                        .unwrap()
                        .push((pipe.style, pipe.color));
                }
                if *event == SceneEvent::SceneReset {
                    generations.push(Vec::new());
                }
            }
            if generations.len() >= 3 {
                break;
            }
        }

        // Pipe *paths* still vary by RNG (only color/style assignment is
        // locked), so two generations can legitimately spawn a slightly
        // different number of pipes before hitting the reset threshold —
        // compare their common prefix rather than requiring equal length.
        let common_len = generations[0].len().min(generations[1].len());
        assert!(
            common_len >= 2,
            "need at least a couple of comparable spawns"
        );
        assert_eq!(
            generations[0][..common_len],
            generations[1][..common_len],
            "locked colors/styles must reproduce the identical spawn-order pattern every generation"
        );
    }
}
