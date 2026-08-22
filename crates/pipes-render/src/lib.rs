//! Shared rendering + configuration layer between `pipes-app` (the
//! screensaver) and `pipes-settings` (the settings app with a live
//! preview). See `docs/ARCHITECTURE.md` for how these fit together.

pub mod app_icon;
pub mod config;
pub mod diagnostics;
pub mod geometry;
pub mod instance;
pub mod renderer;
pub mod tile;

pub use config::{AppConfig, CameraConfig, MonitorMode};
pub use instance::{build_instances, InstanceRaw, InstanceSets, PipeVisuals};
pub use renderer::Renderer;
pub use tile::tile_projection;

/// Re-exported so callers building a non-winit window target (e.g.
/// `pipes-xscreensaver`'s raw X11 handle) can implement the traits
/// `Renderer::new` needs without adding their own direct `wgpu` (or
/// `raw-window-handle`) dependency just for these types.
pub use wgpu::rwh;
