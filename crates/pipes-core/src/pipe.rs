use rand::Rng;

use crate::direction::Direction;
use crate::grid::{GridPos, OccupancyGrid};

/// How a turn in the pipe's path is rendered. The original screensaver mixes
/// smooth elbow bends with ball joints; we keep both and pick per-turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointKind {
    Elbow,
    Ball,
}

/// Cross-section shape of a pipe. `Square` pipes are the classic "mixed
/// style" companion to the default round chrome pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeStyle {
    Round,
    Square,
}

/// Which `PipeStyle`(s) a `Scene` spawns. `Mixed` is the original
/// screensaver's default behavior (each new pipe independently randomized);
/// `Round`/`Square` pin every pipe to one style, validated as a real user
/// want by `pipes.sh`'s `-t` style selection — see `docs/FEATURE_IDEAS.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PipeStyleMode {
    Round,
    Square,
    #[default]
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }
}

/// Why a pipe stopped growing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationReason {
    /// Every neighboring cell is occupied or out of bounds.
    Stuck,
    /// The pipe reached its configured maximum path length.
    MaxLengthReached,
}

/// Outcome of advancing a pipe by one simulation tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    AdvancedStraight,
    Turned,
    Terminated(TerminationReason),
}

/// A single growing (or finished) pipe: its full path through the grid plus
/// the joint placed at each turn.
#[derive(Debug, Clone)]
pub struct Pipe {
    pub id: u32,
    pub style: PipeStyle,
    pub color: Color,
    direction: Direction,
    path: Vec<GridPos>,
    /// One entry per turn in `path`, parallel to the index of the cell where
    /// the turn happened (never the first cell, since there's nothing to
    /// turn from yet).
    joints: Vec<(usize, JointKind)>,
    alive: bool,
    /// Set once, on the step that kills the pipe, and never overwritten
    /// afterward — so a stale caller that steps a dead pipe again still gets
    /// back the real reason it died, instead of a misleading default.
    termination_reason: Option<TerminationReason>,
}

impl Pipe {
    pub fn new(
        id: u32,
        style: PipeStyle,
        color: Color,
        start: GridPos,
        direction: Direction,
    ) -> Self {
        Self {
            id,
            style,
            color,
            direction,
            path: vec![start],
            joints: Vec::new(),
            alive: true,
            termination_reason: None,
        }
    }

    pub fn head(&self) -> GridPos {
        *self.path.last().expect("pipe path is never empty")
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    pub fn path(&self) -> &[GridPos] {
        &self.path
    }

    pub fn joints(&self) -> &[(usize, JointKind)] {
        &self.joints
    }

    pub fn is_alive(&self) -> bool {
        self.alive
    }

    pub fn len(&self) -> usize {
        self.path.len()
    }

    /// Always `false` in practice — a pipe's path always has at least its
    /// starting cell — but required by clippy alongside `len`.
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// Advance the pipe by one grid cell, weighting continued straight travel
    /// much higher than turning (matches the original's tendency to run long
    /// straight stretches punctuated by occasional turns), and never
    /// reversing directly into the cell just vacated.
    pub fn step(
        &mut self,
        grid: &mut OccupancyGrid,
        rng: &mut impl Rng,
        straight_weight: u32,
        turn_weight: u32,
        elbow_probability: f32,
        max_len: usize,
    ) -> StepOutcome {
        if !self.alive {
            let reason = self
                .termination_reason
                .expect("dead pipe always has a recorded reason");
            return StepOutcome::Terminated(reason);
        }
        if self.path.len() >= max_len {
            self.alive = false;
            self.termination_reason = Some(TerminationReason::MaxLengthReached);
            return StepOutcome::Terminated(TerminationReason::MaxLengthReached);
        }

        let banned = self.direction.opposite();
        let head = self.head();

        let mut candidates: Vec<(Direction, u32)> = Direction::ALL
            .into_iter()
            .filter(|d| *d != banned)
            .filter(|d| grid.is_free(head.step(*d)))
            .map(|d| {
                (
                    d,
                    if d == self.direction {
                        straight_weight
                    } else {
                        turn_weight
                    },
                )
            })
            .collect();

        if candidates.is_empty() {
            self.alive = false;
            self.termination_reason = Some(TerminationReason::Stuck);
            return StepOutcome::Terminated(TerminationReason::Stuck);
        }

        candidates.sort_by_key(|(d, _)| direction_rank(*d)); // deterministic ordering before weighted pick
        let total_weight: u32 = candidates.iter().map(|(_, w)| w).sum();
        let mut roll = rng.gen_range(0..total_weight);
        let mut chosen = candidates[0].0;
        for (d, w) in candidates {
            if roll < w {
                chosen = d;
                break;
            }
            roll -= w;
        }

        let turned = chosen != self.direction;
        if turned {
            let joint = if rng.gen::<f32>() < elbow_probability {
                JointKind::Elbow
            } else {
                JointKind::Ball
            };
            self.joints.push((self.path.len() - 1, joint));
        }

        self.direction = chosen;
        let next = head.step(chosen);
        grid.occupy(next);
        self.path.push(next);

        if turned {
            StepOutcome::Turned
        } else {
            StepOutcome::AdvancedStraight
        }
    }
}

/// Stable ordering key so weighted selection is deterministic given the same
/// RNG draws, independent of HashSet/iteration order.
fn direction_rank(d: Direction) -> u8 {
    match d {
        Direction::PosX => 0,
        Direction::NegX => 1,
        Direction::PosY => 2,
        Direction::NegY => 3,
        Direction::PosZ => 4,
        Direction::NegZ => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::GridBounds;
    use rand_pcg::Pcg32;

    fn rng() -> Pcg32 {
        Pcg32::new(42, 54)
    }

    fn small_grid() -> OccupancyGrid {
        let mut g = OccupancyGrid::new(GridBounds::new(10, 10, 10));
        g.occupy(GridPos::new(5, 5, 5));
        g
    }

    #[test]
    fn new_pipe_starts_with_single_point_path() {
        let p = Pipe::new(
            0,
            PipeStyle::Round,
            Color::new(1.0, 1.0, 1.0),
            GridPos::new(5, 5, 5),
            Direction::PosX,
        );
        assert_eq!(p.path(), &[GridPos::new(5, 5, 5)]);
        assert!(p.is_alive());
        assert_eq!(p.joints().len(), 0);
    }

    #[test]
    fn step_never_reverses_into_previous_cell() {
        let mut grid = OccupancyGrid::new(GridBounds::new(20, 20, 20));
        let mut p = Pipe::new(
            0,
            PipeStyle::Round,
            Color::new(1.0, 1.0, 1.0),
            GridPos::new(10, 10, 10),
            Direction::PosX,
        );
        grid.occupy(p.head());
        let mut r = rng();
        for _ in 0..200 {
            let before = p.head();
            let dir_before = p.direction();
            let outcome = p.step(&mut grid, &mut r, 10, 1, 0.75, 10_000);
            if let StepOutcome::Terminated(_) = outcome {
                break;
            }
            let after = p.head();
            assert_ne!(after, before, "pipe head must move every non-terminal step");
            // The new head can never equal stepping backward from `before`.
            assert_ne!(after, before.step(dir_before.opposite()));
        }
    }

    #[test]
    fn step_never_enters_occupied_cell() {
        let mut grid = small_grid(); // (5,5,5) pre-occupied
        let mut p = Pipe::new(
            0,
            PipeStyle::Round,
            Color::new(1.0, 1.0, 1.0),
            GridPos::new(4, 5, 5),
            Direction::PosX,
        );
        grid.occupy(p.head());
        let mut r = rng();
        for _ in 0..50 {
            if let StepOutcome::Terminated(_) = p.step(&mut grid, &mut r, 10, 1, 0.75, 10_000) {
                break;
            }
            assert_ne!(p.head(), GridPos::new(5, 5, 5));
        }
    }

    #[test]
    fn step_never_leaves_grid_bounds() {
        let bounds = GridBounds::new(3, 3, 3);
        let mut grid = OccupancyGrid::new(bounds);
        let mut p = Pipe::new(
            0,
            PipeStyle::Round,
            Color::new(1.0, 1.0, 1.0),
            GridPos::new(1, 1, 1),
            Direction::PosX,
        );
        grid.occupy(p.head());
        let mut r = rng();
        for _ in 0..100 {
            let outcome = p.step(&mut grid, &mut r, 10, 1, 0.75, 10_000);
            assert!(bounds.contains(p.head()));
            if let StepOutcome::Terminated(_) = outcome {
                break;
            }
        }
    }

    #[test]
    fn boxed_in_pipe_terminates_as_stuck() {
        // Trap the pipe with every neighbor of its start cell pre-occupied.
        let bounds = GridBounds::new(5, 5, 5);
        let mut grid = OccupancyGrid::new(bounds);
        let start = GridPos::new(2, 2, 2);
        for d in Direction::ALL {
            grid.occupy(start.step(d));
        }
        let mut p = Pipe::new(
            0,
            PipeStyle::Round,
            Color::new(1.0, 1.0, 1.0),
            start,
            Direction::PosX,
        );
        let mut r = rng();
        let outcome = p.step(&mut grid, &mut r, 10, 1, 0.75, 10_000);
        assert_eq!(outcome, StepOutcome::Terminated(TerminationReason::Stuck));
        assert!(!p.is_alive());
    }

    #[test]
    fn max_length_terminates_pipe() {
        let mut grid = OccupancyGrid::new(GridBounds::new(50, 50, 50));
        let mut p = Pipe::new(
            0,
            PipeStyle::Round,
            Color::new(1.0, 1.0, 1.0),
            GridPos::new(25, 25, 25),
            Direction::PosX,
        );
        grid.occupy(p.head());
        let mut r = rng();
        let mut last = StepOutcome::AdvancedStraight;
        for _ in 0..10 {
            last = p.step(&mut grid, &mut r, 10, 1, 0.75, 3);
        }
        assert_eq!(
            last,
            StepOutcome::Terminated(TerminationReason::MaxLengthReached)
        );
        assert!(p.len() <= 3);
    }

    #[test]
    fn same_seed_produces_identical_path() {
        let run = || {
            let mut grid = OccupancyGrid::new(GridBounds::new(30, 30, 30));
            let mut p = Pipe::new(
                0,
                PipeStyle::Round,
                Color::new(1.0, 1.0, 1.0),
                GridPos::new(15, 15, 15),
                Direction::PosX,
            );
            grid.occupy(p.head());
            let mut r = Pcg32::new(7, 13);
            for _ in 0..40 {
                if let StepOutcome::Terminated(_) = p.step(&mut grid, &mut r, 10, 1, 0.75, 10_000) {
                    break;
                }
            }
            p.path().to_vec()
        };
        assert_eq!(
            run(),
            run(),
            "identical seed must produce identical path (determinism for tests/replays)"
        );
    }
}
