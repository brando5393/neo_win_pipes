//! Shared rendering + configuration layer between `pipes-app` (the
//! screensaver) and `pipes-settings` (the settings app with a live
//! preview). See `docs/ARCHITECTURE.md` for how these fit together.

pub mod config;
pub mod geometry;
pub mod instance;
pub mod renderer;

pub use config::{AppConfig, CameraConfig};
pub use instance::{build_instances, InstanceRaw, InstanceSets, PipeVisuals};
pub use renderer::Renderer;
