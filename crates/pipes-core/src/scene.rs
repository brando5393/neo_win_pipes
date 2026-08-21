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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    PipeSpawned { id: u32 },
    PipeTerminated { id: u32 },
    SceneReset,
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

    /// Advance the whole scene by one tick: step every live pipe, reap dead
    /// ones, spawn replacements up to `max_pipes`, and reset if the grid has
    /// filled past the configured threshold. Returns the events that
    /// occurred, in order, for callers that want to log or react to them.
    pub fn step(&mut self) -> Vec<SceneEvent> {
        let mut events = Vec::new();
        self.tick += 1;

        for pipe in &mut self.pipes {
            if !pipe.is_alive() {
                continue;
            }
            let outcome = pipe.step(
                &mut self.grid,
                &mut self.rng,
                self.config.straight_weight,
                self.config.turn_weight,
                self.config.elbow_probability,
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
            info!(
                tick = self.tick,
                ratio = self.grid.occupancy_ratio(),
                "scene reset (grid filled)"
            );
            self.reset();
            events.push(SceneEvent::SceneReset);
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
            let style = match self.config.style_mode {
                PipeStyleMode::Round => PipeStyle::Round,
                PipeStyleMode::Square => PipeStyle::Square,
                PipeStyleMode::Mixed => {
                    if self.rng.gen_bool(0.5) {
                        PipeStyle::Round
                    } else {
                        PipeStyle::Square
                    }
                }
            };
            let color = if self.config.palette.is_empty() {
                let fallback = default_palette();
                fallback[self.rng.gen_range(0..fallback.len())]
            } else {
                let palette = &self.config.palette;
                palette[self.rng.gen_range(0..palette.len())]
            };
            let id = self.next_id;
            self.next_id += 1;
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
}
