//! Platform-agnostic simulation engine for neo_win_pipes.
//!
//! This crate has no rendering or windowing dependencies on purpose: it only
//! knows about a 3D grid, pipes growing through it, and when the scene
//! should reset. That keeps the entire simulation deterministically
//! unit-testable (see `same_seed_produces_identical_*` tests in each module)
//! without needing a GPU, a window, or a display at all — CI can run the
//! full test suite headless on every platform.
//!
//! See `docs/ARCHITECTURE.md` in the repo root for how this crate fits into
//! `pipes-app` and the future native screensaver wrappers, and
//! `docs/RESEARCH.md` for the original screensaver behavior this models.

mod direction;
mod grid;
mod pipe;
mod scene;

pub use direction::Direction;
pub use grid::{GridBounds, GridPos, OccupancyGrid};
pub use pipe::{Color, JointKind, Pipe, PipeStyle, StepOutcome, TerminationReason};
pub use scene::{Scene, SceneEvent, SimConfig};
