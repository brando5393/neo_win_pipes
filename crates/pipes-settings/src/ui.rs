//! The settings drawer + live-preview split layout. Pure UI: reads/writes
//! `AppConfig` fields directly and reports back what category of change
//! happened (so `main.rs` knows whether the `Scene` needs rebuilding) and
//! where the preview panel ended up (so `main.rs` knows what rect to render
//! the 3D scene into).

use egui::{Context, RichText, Slider};
use pipes_core::{Color, PipeStyleMode};
use pipes_render::AppConfig;

use crate::update::AvailableUpdate;

pub struct Outcome {
    /// A change that affects the simulation itself (style, count, palette,
    /// grid size, reset threshold) — the live-preview `Scene` must be
    /// rebuilt for it to take effect, since `SimConfig` is baked in at
    /// `Scene::new`.
    pub sim_changed: bool,
    /// A change that only affects rendering (pipe thickness, camera,
    /// speed) — takes effect on the very next frame with no rebuild.
    pub other_changed: bool,
    pub reset_to_defaults: bool,
    /// "Update Now" clicked on the update banner — `main.rs` downloads and
    /// launches the installer for the `AvailableUpdate` it already has.
    pub update_now_clicked: bool,
    /// "Dismiss" clicked on the update banner — hides it for the rest of
    /// this session (doesn't disable future checks/relaunches).
    pub update_dismissed: bool,
    pub preview_rect: egui::Rect,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            sim_changed: false,
            other_changed: false,
            reset_to_defaults: false,
            update_now_clicked: false,
            update_dismissed: false,
            preview_rect: egui::Rect::NOTHING,
        }
    }
}

impl Outcome {
    pub fn changed(&self) -> bool {
        self.sim_changed || self.other_changed || self.reset_to_defaults
    }
}

pub fn draw(
    ctx: &Context,
    config: &mut AppConfig,
    update: Option<&AvailableUpdate>,
    downloading: bool,
) -> Outcome {
    let mut outcome = Outcome::default();

    if let Some(update) = update {
        egui::TopBottomPanel::top("update_banner").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(format!("A new version (v{}) is available.", update.version));
                if downloading {
                    ui.add(egui::Spinner::new());
                    ui.label(RichText::new("Downloading and launching installer…").weak());
                } else {
                    if ui.button("Update Now").clicked() {
                        outcome.update_now_clicked = true;
                    }
                    if ui.link("Release notes").clicked() {
                        open_in_browser(&update.release_page_url);
                    }
                    if ui.small_button("Dismiss").clicked() {
                        outcome.update_dismissed = true;
                    }
                }
            });
            ui.add_space(4.0);
        });
    }

    egui::SidePanel::right("settings_drawer")
        .resizable(true)
        .default_width(360.0)
        .min_width(300.0)
        .show(ctx, |ui| {
            // The settings drawer has grown past a single 760px-tall window
            // (dissolve, teapot, and future sections push later content off
            // the bottom) — without this, overflow is silently clipped with
            // no scrollbar and no error, not just visually cramped.
            egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Pipes Settings");
            ui.label(RichText::new("Live preview updates as you adjust these.").weak());
            ui.separator();

            ui.label(RichText::new("Themes").strong());
            ui.horizontal(|ui| {
                for theme in Theme::ALL {
                    if ui.button(theme.label()).clicked() {
                        theme.apply(config);
                        outcome.sim_changed = true;
                        outcome.other_changed = true;
                    }
                }
            });
            ui.label(RichText::new("Bundles palette + style + speed together; tweak anything below afterward.").weak().small());
            ui.separator();

            ui.collapsing("Pipe style & count", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Style:");
                    outcome.sim_changed |= ui
                        .radio_value(&mut config.sim.style_mode, PipeStyleMode::Mixed, "Mixed")
                        .changed();
                    outcome.sim_changed |= ui
                        .radio_value(&mut config.sim.style_mode, PipeStyleMode::Round, "Round")
                        .changed();
                    outcome.sim_changed |= ui
                        .radio_value(&mut config.sim.style_mode, PipeStyleMode::Square, "Square")
                        .changed();
                });
                outcome.sim_changed |= ui
                    .add(Slider::new(&mut config.sim.max_pipes, 1..=32).text("Number of pipes"))
                    .changed();
                outcome.other_changed |= ui
                    .add(
                        Slider::new(&mut config.visuals.pipe_radius, 0.05..=0.4)
                            .text("Pipe thickness"),
                    )
                    .changed();
            });

            ui.collapsing("Pipe behavior", |ui| {
                outcome.sim_changed |= ui
                    .add(
                        Slider::new(&mut config.sim.straight_weight, 1..=50)
                            .text("Straightness (vs. turning)"),
                    )
                    .changed();
                outcome.sim_changed |= ui
                    .add(Slider::new(&mut config.sim.turn_weight, 1..=50).text("Turn eagerness"))
                    .changed();
                outcome.sim_changed |= ui
                    .add(
                        Slider::new(&mut config.sim.elbow_probability, 0.0..=1.0)
                            .text("Elbow joint chance (vs. ball joint)"),
                    )
                    .changed();
                outcome.sim_changed |= ui
                    .checkbox(
                        &mut config.sim.teapot_easter_egg_enabled,
                        "Teapot easter egg (classic Utah teapot, very rare)",
                    )
                    .changed();
                ui.add_enabled_ui(config.sim.teapot_easter_egg_enabled, |ui| {
                    outcome.sim_changed |= ui
                        .add(
                            Slider::new(&mut config.sim.teapot_probability, 0.0..=0.25)
                                .text("Teapot chance"),
                        )
                        .changed();
                });
            });

            ui.collapsing("Speed & camera", |ui| {
                outcome.other_changed |= ui
                    .add(
                        Slider::new(&mut config.tick_interval_ms, 20..=800)
                            .text("Tick interval (ms, lower = faster)"),
                    )
                    .changed();
                outcome.other_changed |= ui
                    .checkbox(&mut config.camera.orbit_enabled, "Camera orbits the scene")
                    .changed();
                ui.add_enabled_ui(config.camera.orbit_enabled, |ui| {
                    outcome.other_changed |= ui
                        .add(
                            Slider::new(&mut config.camera.orbit_speed, 0.0..=1.0)
                                .text("Orbit speed"),
                        )
                        .changed();
                });
            });

            ui.collapsing("Color palette", |ui| {
                outcome.sim_changed |= ui
                    .checkbox(
                        &mut config.sim.lock_colors_across_resets,
                        "Keep the same color/style pattern across resets",
                    )
                    .changed();
                ui.horizontal(|ui| {
                    if ui.button("Classic").clicked() {
                        config.sim.palette = pipes_core::default_palette();
                        outcome.sim_changed = true;
                    }
                    if ui.button("Neon").clicked() {
                        config.sim.palette = neon_palette();
                        outcome.sim_changed = true;
                    }
                    if ui.button("Monochrome").clicked() {
                        config.sim.palette = monochrome_palette();
                        outcome.sim_changed = true;
                    }
                });
                ui.label(RichText::new("Custom colors:").weak());
                let mut remove_index = None;
                let palette_len = config.sim.palette.len();
                for (i, color) in config.sim.palette.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        let mut rgb = [color.r, color.g, color.b];
                        if ui.color_edit_button_rgb(&mut rgb).changed() {
                            color.r = rgb[0];
                            color.g = rgb[1];
                            color.b = rgb[2];
                            outcome.sim_changed = true;
                        }
                        if palette_len > 1 && ui.small_button("remove").clicked() {
                            remove_index = Some(i);
                        }
                    });
                }
                if let Some(i) = remove_index {
                    config.sim.palette.remove(i);
                    outcome.sim_changed = true;
                }
                if config.sim.palette.len() < 8 && ui.button("+ add color").clicked() {
                    config.sim.palette.push(Color::new(0.8, 0.8, 0.8));
                    outcome.sim_changed = true;
                }
            });

            ui.collapsing("Grid size & reset", |ui| {
                outcome.sim_changed |= ui
                    .add(Slider::new(&mut config.sim.bounds.width, 8..=64).text("Grid width"))
                    .changed();
                outcome.sim_changed |= ui
                    .add(Slider::new(&mut config.sim.bounds.height, 8..=64).text("Grid height"))
                    .changed();
                outcome.sim_changed |= ui
                    .add(Slider::new(&mut config.sim.bounds.depth, 8..=64).text("Grid depth"))
                    .changed();
                outcome.sim_changed |= ui
                    .add(
                        Slider::new(&mut config.sim.reset_occupancy_ratio, 0.05..=0.9)
                            .text("Reset once this full"),
                    )
                    .changed();
                outcome.sim_changed |= ui
                    .checkbox(
                        &mut config.sim.dissolve_on_reset,
                        "Dissolve pipes away on reset (classic effect)",
                    )
                    .changed();
                ui.add_enabled_ui(config.sim.dissolve_on_reset, |ui| {
                    outcome.sim_changed |= ui
                        .add(
                            Slider::new(&mut config.sim.dissolve_duration_ticks, 1..=90)
                                .text("Dissolve duration (ticks)"),
                        )
                        .changed();
                });
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Reset to defaults").clicked() {
                    outcome.reset_to_defaults = true;
                }
                if ui.button("Export…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("neo_win_pipes_config.toml")
                        .add_filter("TOML", &["toml"])
                        .save_file()
                    {
                        if let Err(err) = config.save_to(&path) {
                            tracing::error!(?err, path = %path.display(), "failed to export config");
                        } else {
                            tracing::info!(path = %path.display(), "exported config");
                        }
                    }
                }
                if ui.button("Import…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("TOML", &["toml"]).pick_file() {
                        *config = AppConfig::load_from(&path);
                        tracing::info!(path = %path.display(), "imported config");
                        outcome.sim_changed = true;
                        outcome.other_changed = true;
                    }
                }
            });
            if let Some(path) = AppConfig::config_path() {
                ui.label(
                    RichText::new(format!("Autosaves to {}", path.display()))
                        .weak()
                        .small(),
                );
            }
            });
        });

    // Frame::none() is essential here: CentralPanel's default frame paints
    // an opaque background fill over its whole area, which would otherwise
    // completely hide the 3D pipes render drawn underneath it in the same
    // frame (see pipes-settings main.rs's render_with `extra` closure).
    outcome.preview_rect = egui::CentralPanel::default()
        .frame(egui::Frame::none())
        .show(ctx, |ui| ui.available_rect_before_wrap())
        .inner;

    outcome
}

/// Opens a URL in the system's default browser. Best-effort: a failure
/// here (no browser configured, `cmd`/`open`/`xdg-open` missing) just
/// means the link doesn't open, not something worth surfacing as an
/// error to the user over.
fn open_in_browser(url: &str) {
    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    if let Err(err) = result {
        tracing::warn!(?err, url, "failed to open release notes link");
    }
}

fn neon_palette() -> Vec<Color> {
    vec![
        Color::new(1.0, 0.05, 0.6),
        Color::new(0.05, 1.0, 0.85),
        Color::new(0.65, 0.05, 1.0),
        Color::new(1.0, 0.85, 0.05),
        Color::new(0.05, 0.6, 1.0),
    ]
}

fn monochrome_palette() -> Vec<Color> {
    vec![
        Color::new(0.95, 0.95, 0.97),
        Color::new(0.7, 0.7, 0.75),
        Color::new(0.45, 0.45, 0.5),
        Color::new(0.85, 0.85, 0.9),
    ]
}

/// One-click bundles of palette + style + speed, requested as a
/// lower-confidence "ours" idea in `docs/FEATURE_IDEAS.md` — cheap to
/// build on top of the palette presets and per-field sliders that already
/// existed. Each theme sets several `AppConfig` fields at once; anything
/// it sets can still be tweaked individually afterward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Theme {
    Classic96,
    Neon,
    Monochrome,
}

impl Theme {
    const ALL: [Theme; 3] = [Theme::Classic96, Theme::Neon, Theme::Monochrome];

    fn label(self) -> &'static str {
        match self {
            Theme::Classic96 => "Classic '96",
            Theme::Neon => "Neon",
            Theme::Monochrome => "Monochrome",
        }
    }

    fn apply(self, config: &mut AppConfig) {
        match self {
            Theme::Classic96 => {
                config.sim.palette = pipes_core::default_palette();
                config.sim.style_mode = PipeStyleMode::Mixed;
                config.tick_interval_ms = 120;
                config.camera.orbit_enabled = true;
                config.camera.orbit_speed = 0.15;
            }
            Theme::Neon => {
                config.sim.palette = neon_palette();
                config.sim.style_mode = PipeStyleMode::Mixed;
                config.tick_interval_ms = 60;
                config.camera.orbit_enabled = true;
                config.camera.orbit_speed = 0.35;
            }
            Theme::Monochrome => {
                config.sim.palette = monochrome_palette();
                config.sim.style_mode = PipeStyleMode::Round;
                config.tick_interval_ms = 200;
                config.camera.orbit_enabled = true;
                config.camera.orbit_speed = 0.08;
            }
        }
    }
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn every_theme_produces_a_sanitize_stable_config() {
        // Each theme's values must already be within AppConfig::sanitize's
        // clamped ranges — a theme that got clamped on apply would be
        // silently different from what the button claimed to set.
        for theme in Theme::ALL {
            let mut config = AppConfig::default();
            theme.apply(&mut config);
            let before = config.clone();
            config.sanitize();
            assert_eq!(
                config,
                before,
                "{:?} should need no clamping",
                theme.label()
            );
            assert!(!config.sim.palette.is_empty());
        }
    }
}
