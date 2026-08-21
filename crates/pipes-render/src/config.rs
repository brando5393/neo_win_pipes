//! `AppConfig`: everything a "Pipes Settings" session can tune, persisted as
//! TOML in the OS's standard per-user config directory so `pipes-app` (the
//! actual screensaver) and `pipes-settings` (the config app) read/write the
//! exact same file. See `docs/FEATURE_IDEAS.md` for why these particular
//! knobs were chosen — they're the ones validated by looking at what users
//! of prior pipes-screensaver projects actually asked for.

use std::path::{Path, PathBuf};

use pipes_core::SimConfig;
use serde::{Deserialize, Serialize};

use crate::instance::PipeVisuals;

/// `#[serde(default)]` (container-level) makes every field individually
/// forward-compatible with older saved config files — see the matching
/// note on `pipes_core::SimConfig` for why this matters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraConfig {
    pub orbit_enabled: bool,
    /// Radians per second of camera drift around the scene.
    pub orbit_speed: f32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            orbit_enabled: true,
            orbit_speed: 0.15,
        }
    }
}

/// `#[serde(default)]` (container-level) makes every field individually
/// forward-compatible with older saved config files — see the matching
/// note on `pipes_core::SimConfig` for why this matters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub sim: SimConfig,
    pub visuals: PipeVisuals,
    pub camera: CameraConfig,
    /// How often the simulation advances by one tick, independent of frame
    /// rate. Lower = faster-growing pipes.
    pub tick_interval_ms: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sim: SimConfig::default(),
            visuals: PipeVisuals::default(),
            camera: CameraConfig::default(),
            tick_interval_ms: 120,
        }
    }
}

impl AppConfig {
    /// Clamp every field to a safe range. Called after loading (a
    /// hand-edited or stale config file shouldn't be able to produce
    /// degenerate geometry, a divide-by-zero, or an unusably slow/fast
    /// simulation) and is cheap enough to call after every settings-app
    /// edit too.
    pub fn sanitize(&mut self) {
        self.sim.max_pipes = self.sim.max_pipes.clamp(1, 64);
        self.sim.bounds.width = self.sim.bounds.width.clamp(4, 128);
        self.sim.bounds.height = self.sim.bounds.height.clamp(4, 128);
        self.sim.bounds.depth = self.sim.bounds.depth.clamp(4, 128);
        self.sim.reset_occupancy_ratio = self.sim.reset_occupancy_ratio.clamp(0.05, 0.95);
        self.sim.straight_weight = self.sim.straight_weight.clamp(1, 100);
        self.sim.turn_weight = self.sim.turn_weight.clamp(1, 100);
        self.sim.elbow_probability = self.sim.elbow_probability.clamp(0.0, 1.0);
        self.sim.max_pipe_length = self.sim.max_pipe_length.clamp(10, 100_000);
        self.sim.dissolve_duration_ticks = self.sim.dissolve_duration_ticks.clamp(1, 300);
        self.sim.teapot_probability = self.sim.teapot_probability.clamp(0.0, 1.0);
        if self.sim.palette.is_empty() {
            self.sim.palette = pipes_core::default_palette();
        }
        self.visuals.pipe_radius = self.visuals.pipe_radius.clamp(0.02, 0.49);
        self.visuals.ball_joint_scale = self.visuals.ball_joint_scale.clamp(1.0, 3.0);
        self.visuals.elbow_joint_scale = self.visuals.elbow_joint_scale.clamp(1.0, 3.0);
        self.visuals.cap_scale = self.visuals.cap_scale.clamp(1.0, 3.0);
        self.visuals.teapot_scale = self.visuals.teapot_scale.clamp(1.0, 8.0);
        self.camera.orbit_speed = self.camera.orbit_speed.clamp(0.0, 2.0);
        self.tick_interval_ms = self.tick_interval_ms.clamp(10, 2000);
    }

    /// Standard per-user config file path (e.g. `%APPDATA%\neo_win_pipes\config.toml`
    /// on Windows, `~/Library/Application Support/dev.neo-win-pipes.neo_win_pipes/config.toml`
    /// on macOS, `~/.config/neo_win_pipes/config.toml` on Linux). `None` if
    /// the OS's home directory can't be resolved (rare) — callers should
    /// fall back to in-memory defaults.
    pub fn config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("dev", "neo-win-pipes", "neo_win_pipes")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    /// Load from the standard path, or defaults if it's missing, unreadable,
    /// or fails to parse (never panics on a bad config file).
    pub fn load() -> Self {
        match Self::config_path() {
            Some(path) => Self::load_from(&path),
            None => Self::default(),
        }
    }

    pub fn load_from(path: &Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(_) => return Self::default(),
        };
        let mut config: Self = match toml::from_str(&text) {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(?err, path = %path.display(), "failed to parse config file, using defaults");
                return Self::default();
            }
        };
        config.sanitize();
        config
    }

    /// Save to the standard path, creating the parent directory if needed.
    /// Silently does nothing if the config directory can't be resolved.
    pub fn save(&self) -> std::io::Result<()> {
        match Self::config_path() {
            Some(path) => self.save_to(&path),
            None => Ok(()),
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self).expect("AppConfig always serializes to TOML");
        std::fs::write(path, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "neo_win_pipes_config_test_{name}_{}.toml",
            std::process::id()
        ))
    }

    #[test]
    fn default_config_is_already_sane() {
        let mut config = AppConfig::default();
        let before = format!("{config:?}");
        config.sanitize();
        assert_eq!(
            format!("{config:?}"),
            before,
            "defaults should need no clamping"
        );
    }

    #[test]
    fn old_config_missing_newer_fields_keeps_its_other_settings() {
        // Regression test for a real bug caught while adding the dissolve
        // feature: without container-level #[serde(default)], a config
        // file saved before a field existed would fail to parse entirely
        // and silently discard every setting in it, not just fall back
        // for the field that's actually missing. This file has no
        // `dissolve_on_reset`/`dissolve_duration_ticks` at all (as if
        // saved by an older version) but customizes `max_pipes` and
        // `tick_interval_ms` — both must survive.
        let path = test_path("old_format");
        std::fs::write(
            &path,
            r#"
tick_interval_ms = 250

[sim]
max_pipes = 12
straight_weight = 10
turn_weight = 1
elbow_probability = 0.75
reset_occupancy_ratio = 0.35
max_pipe_length = 400
spawn_attempts = 64
style_mode = "Round"

[sim.bounds]
width = 24
height = 16
depth = 24

[visuals]
pipe_radius = 0.18
ball_joint_scale = 1.4
elbow_joint_scale = 1.05
cap_scale = 1.1

[camera]
orbit_enabled = true
orbit_speed = 0.15
"#,
        )
        .unwrap();

        let config = AppConfig::load_from(&path);
        assert_eq!(
            config.sim.max_pipes, 12,
            "customized fields present in the file must survive"
        );
        assert_eq!(config.tick_interval_ms, 250);
        assert!(
            config.sim.dissolve_on_reset,
            "missing fields must fall back to their own default, not wipe the file"
        );
        assert_eq!(config.sim.dissolve_duration_ticks, 15);
        assert!(
            !config.sim.palette.is_empty(),
            "missing palette (also absent from this fixture) must fall back too"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_loads_defaults() {
        let path = test_path("missing");
        let _ = std::fs::remove_file(&path);
        let config = AppConfig::load_from(&path);
        assert_eq!(config.sim.max_pipes, AppConfig::default().sim.max_pipes);
    }

    #[test]
    fn save_then_load_round_trips() {
        let path = test_path("roundtrip");
        let mut config = AppConfig::default();
        config.sim.max_pipes = 12;
        config.tick_interval_ms = 250;
        config.camera.orbit_enabled = false;
        config.save_to(&path).expect("save should succeed");

        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.sim.max_pipes, 12);
        assert_eq!(loaded.tick_interval_ms, 250);
        assert!(!loaded.camera.orbit_enabled);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults_without_panicking() {
        let path = test_path("corrupt");
        std::fs::write(&path, "this is not { valid toml").unwrap();
        let config = AppConfig::load_from(&path);
        assert_eq!(config.sim.max_pipes, AppConfig::default().sim.max_pipes);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sanitize_clamps_out_of_range_values() {
        let mut config = AppConfig::default();
        config.sim.max_pipes = 9999;
        config.sim.elbow_probability = 5.0;
        config.visuals.pipe_radius = -1.0;
        config.tick_interval_ms = 0;
        config.sim.palette.clear();

        config.sanitize();

        assert!(config.sim.max_pipes <= 64);
        assert!(config.sim.elbow_probability <= 1.0);
        assert!(config.visuals.pipe_radius >= 0.02);
        assert!(config.tick_interval_ms >= 10);
        assert!(
            !config.sim.palette.is_empty(),
            "empty palette must fall back to default_palette()"
        );
    }
}
