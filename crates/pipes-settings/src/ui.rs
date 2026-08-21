//! The settings drawer + live-preview split layout. Pure UI: reads/writes
//! `AppConfig` fields directly and reports back what category of change
//! happened (so `main.rs` knows whether the `Scene` needs rebuilding) and
//! where the preview panel ended up (so `main.rs` knows what rect to render
//! the 3D scene into).

use egui::{Context, RichText, Slider};
use pipes_core::{Color, PipeStyleMode};
use pipes_render::AppConfig;

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
    pub preview_rect: egui::Rect,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            sim_changed: false,
            other_changed: false,
            reset_to_defaults: false,
            preview_rect: egui::Rect::NOTHING,
        }
    }
}

impl Outcome {
    pub fn changed(&self) -> bool {
        self.sim_changed || self.other_changed || self.reset_to_defaults
    }
}

pub fn draw(ctx: &Context, config: &mut AppConfig) -> Outcome {
    let mut outcome = Outcome::default();

    egui::SidePanel::right("settings_drawer")
        .resizable(true)
        .default_width(360.0)
        .min_width(300.0)
        .show(ctx, |ui| {
            ui.heading("Pipes Settings");
            ui.label(RichText::new("Live preview updates as you adjust these.").weak());
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
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Reset to defaults").clicked() {
                    outcome.reset_to_defaults = true;
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
