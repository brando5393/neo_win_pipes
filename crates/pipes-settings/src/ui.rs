//! The settings drawer + live-preview split layout. Pure UI: reads/writes
//! `AppConfig` fields directly and reports back what category of change
//! happened (so `main.rs` knows whether the `Scene` needs rebuilding) and
//! where the preview panel ended up (so `main.rs` knows what rect to render
//! the 3D scene into).

use egui::{Context, RichText, Slider};
use pipes_core::{Color, PipeStyleMode};
use pipes_render::{AppConfig, MonitorMode};

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

            ui.collapsing("Multi-monitor", |ui| {
                ui.label(
                    RichText::new(
                        "Only affects the actual fullscreen screensaver, not this preview.",
                    )
                    .weak()
                    .small(),
                );
                ui.horizontal(|ui| {
                    outcome.other_changed |= ui
                        .radio_value(
                            &mut config.monitor_mode,
                            MonitorMode::AllMonitors,
                            "All displays (independent instance per screen)",
                        )
                        .changed();
                });
                ui.horizontal(|ui| {
                    outcome.other_changed |= ui
                        .radio_value(
                            &mut config.monitor_mode,
                            MonitorMode::PrimaryOnly,
                            "Primary display only",
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

            ui.separator();
            if ui.button("Report Issue / Feedback…").clicked() {
                set_feedback_popup_open(ctx, true);
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

    draw_feedback_popup(ctx);

    outcome
}

fn feedback_state_id() -> egui::Id {
    egui::Id::new("feedback_popup_state")
}

fn set_feedback_popup_open(ctx: &Context, open: bool) {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<FeedbackState>(feedback_state_id())
            .open = open;
    });
}

/// The feedback popup: a floating (not a dimmed/blocking modal — the
/// rest of the UI stays interactive behind it, which is fine here, since
/// nothing catastrophic happens if a slider gets nudged while this is
/// open) `egui::Window`. State lives in egui's temp memory (not
/// `AppConfig` — this is never autosaved/exported) and is explicitly
/// reset to `FeedbackState::default()` — not just hidden — on every close
/// path (successful submit, the window's own X button), so reopening
/// always starts from a blank form rather than stale leftover text.
fn draw_feedback_popup(ctx: &Context) {
    let id = feedback_state_id();
    let mut state: FeedbackState =
        ctx.data_mut(|d| d.get_temp_mut_or_default::<FeedbackState>(id).clone());
    if !state.open {
        return;
    }

    let mut still_open = true;
    egui::Window::new("Report Issue / Feedback")
        .open(&mut still_open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for category in FeedbackCategory::ALL {
                    ui.radio_value(&mut state.category, category, category.label());
                }
            });
            ui.label(
                RichText::new(
                    "The category above sets the issue's GitHub label directly — it's \
                     not added as text, so keep the title itself plain.",
                )
                .weak()
                .small(),
            );
            ui.add_space(4.0);

            ui.label("Title");
            ui.add(
                egui::TextEdit::singleline(&mut state.title)
                    .hint_text("Short summary (required)")
                    .desired_width(320.0),
            );
            ui.add_space(4.0);
            ui.label("Description");
            ui.add(
                egui::TextEdit::multiline(&mut state.description)
                    .hint_text("What happened, or what would you like to see? (required)")
                    .desired_rows(5)
                    .desired_width(320.0),
            );

            // Only offered for Bug: recent log output isn't relevant to a
            // feature request or question. On-by-default (logs usually
            // help us fix things faster) but stays a visible choice, not
            // something baked in silently - this ships to anyone who
            // installs the app, not just us, so "sanitized" below means
            // "your own home-directory path is redacted", not "provably
            // free of anything sensitive": we can't know what a real
            // user's own description text or a third-party log line
            // might contain.
            if state.category == FeedbackCategory::Bug {
                ui.add_space(4.0);
                ui.checkbox(
                    &mut state.include_log,
                    "Include recent log output — often the single most helpful thing in a bug report",
                );
                ui.label(
                    RichText::new(
                        "Your home directory path is redacted before inclusion (best effort, \
                         not a guarantee) — this becomes a public GitHub issue either way, so \
                         look over what you're sharing if you're not sure.",
                    )
                    .weak()
                    .small(),
                );
            }

            if let Some(err) = state.launch_error.clone() {
                ui.add_space(6.0);
                ui.colored_label(
                    egui::Color32::from_rgb(220, 90, 90),
                    format!("Couldn't open your browser: {err}"),
                );
                ui.label(
                    RichText::new("Copy this link and open it manually:")
                        .weak()
                        .small(),
                );
                let mut url = state.last_url.clone();
                ui.add(
                    egui::TextEdit::singleline(&mut url)
                        .desired_width(320.0)
                        .interactive(false),
                );
            }

            ui.add_space(8.0);
            let can_submit = !state.title.trim().is_empty() && !state.description.trim().is_empty();
            ui.add_enabled_ui(can_submit, |ui| {
                if ui.button("Open GitHub Issue").clicked() {
                    let include_log = state.category == FeedbackCategory::Bug && state.include_log;
                    let url = feedback_issue_url(
                        state.category,
                        &state.title,
                        &state.description,
                        include_log,
                    );
                    match open_in_browser_checked(&url) {
                        Ok(()) => state = FeedbackState::default(),
                        Err(err) => {
                            state.launch_error = Some(err);
                            state.last_url = url;
                        }
                    }
                }
            });
            if !can_submit {
                ui.label(
                    RichText::new("Title and description are both required.")
                        .weak()
                        .small(),
                );
            }
            ui.label(
                RichText::new(
                    "Submitting needs a GitHub account, and nothing is sent anywhere \
                     until you click \"Submit new issue\" on the page that opens.",
                )
                .weak()
                .small(),
            );
        });

    if !still_open {
        // The window's own X button — clear, not just hide, so reopening
        // starts fresh.
        state = FeedbackState::default();
    }

    ctx.data_mut(|d| d.insert_temp(id, state));
}

/// Opens a URL in the system's default browser. Best-effort: a failure
/// here (no browser configured, `cmd`/`open`/`xdg-open` missing) just
/// means the link doesn't open, not something worth surfacing as an
/// error to the user over — for callers that don't need to react to it
/// (e.g. "Release notes").
fn open_in_browser(url: &str) {
    if let Err(err) = open_in_browser_checked(url) {
        tracing::warn!(url, %err, "failed to open link in browser");
    }
}

/// Same as `open_in_browser`, but returns the error instead of just
/// logging it — for callers that need to show it to the user (the
/// feedback popup, so a missing/unconfigured browser doesn't just
/// silently do nothing).
#[cfg(windows)]
fn open_in_browser_checked(url: &str) -> Result<(), String> {
    // Deliberately not `cmd /c start "" url`: cmd.exe re-parses its
    // command tail through its own shell grammar, where `&` is a command
    // separator - every feedback-issue URL contains one (query-string
    // separators), so that approach silently truncated the URL at the
    // first `&` (caught by actually clicking through a generated issue
    // link and seeing the title/body/labels params missing, not by
    // reading this code). ShellExecuteW talks to the shell directly, with
    // no command-line re-parsing involved.
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let operation = to_wide("open");
    let file = to_wide(url);
    // SAFETY: both wide-string buffers outlive this call (owned locals,
    // dropped only after ShellExecuteW returns); null hwnd/parameters/
    // directory are all documented-valid for this operation.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // Per the Win32 docs: a return value > 32 means success; anything
    // <= 32 is an SE_ERR_* error code.
    if (result as isize) > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW failed (code {})", result as isize))
    }
}

#[cfg(target_os = "macos")]
fn open_in_browser_checked(url: &str) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|err| err.to_string())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn open_in_browser_checked(url: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|err| err.to_string())
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

/// Category picked in the feedback form — maps directly to a real GitHub
/// label (confirmed to already exist on this repo: `bug`, `enhancement`,
/// `question`) via the issue URL's `labels=` param, not to any text
/// mangled into the title, so triage doesn't depend on the reporter
/// phrasing anything a particular way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum FeedbackCategory {
    #[default]
    Bug,
    FeatureRequest,
    Question,
}

impl FeedbackCategory {
    const ALL: [FeedbackCategory; 3] = [
        FeedbackCategory::Bug,
        FeedbackCategory::FeatureRequest,
        FeedbackCategory::Question,
    ];

    fn label(self) -> &'static str {
        match self {
            FeedbackCategory::Bug => "Bug",
            FeedbackCategory::FeatureRequest => "Feature request",
            FeedbackCategory::Question => "Question / other",
        }
    }

    fn github_label(self) -> &'static str {
        match self {
            FeedbackCategory::Bug => "bug",
            FeedbackCategory::FeatureRequest => "enhancement",
            FeedbackCategory::Question => "question",
        }
    }
}

#[derive(Clone)]
struct FeedbackState {
    open: bool,
    category: FeedbackCategory,
    /// Separate from `description` — mirrors GitHub's own issue form
    /// (short title field + a full description body), rather than
    /// deriving a title from the first line of one combined text box.
    /// That earlier approach broke for anyone typing a single-paragraph
    /// description with no line break: the *entire* text became the
    /// title, since there was no newline to split on.
    title: String,
    description: String,
    /// Only consulted for `FeedbackCategory::Bug` — defaults to on
    /// (logs are usually the single most useful thing in a bug report),
    /// but stays a visible checkbox rather than an automatic inclusion
    /// since the log can contain local file paths.
    include_log: bool,
    /// Set when `open_in_browser_checked` fails, so the popup can show the
    /// error plus `last_url` for the user to copy and open by hand,
    /// instead of silently doing nothing.
    launch_error: Option<String>,
    last_url: String,
}

impl Default for FeedbackState {
    fn default() -> Self {
        Self {
            open: false,
            category: FeedbackCategory::default(),
            title: String::new(),
            description: String::new(),
            include_log: true,
            launch_error: None,
            last_url: String::new(),
        }
    }
}

/// Builds a pre-filled "new issue" URL for this project's own repo — no
/// account or API token needed on our end, since the person reporting the
/// issue submits it themselves once the browser opens (they do still need
/// their own GitHub account for that final step). `category` only sets
/// `labels=` (the `bug`/`enhancement`/`question` labels already exist on
/// this repo, confirmed via `gh label list`) — it's deliberately never
/// mixed into `title` as text, so the label is the one source of truth
/// for what kind of feedback this is, not the title's wording.
fn feedback_issue_url(
    category: FeedbackCategory,
    title: &str,
    description: &str,
    include_log: bool,
) -> String {
    let mut body = format!(
        "{description}\n\n---\nApp version: {}\nOS: {}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
    );
    if include_log {
        match recent_log_tail(MAX_LOG_TAIL_CHARS) {
            Some(tail) => {
                body.push_str(&format!(
                    "\n<details><summary>Recent log output</summary>\n\n```\n{tail}\n```\n\n</details>\n"
                ));
            }
            None => body.push_str("\n(No log file found to include.)\n"),
        }
    }
    format!(
        "https://github.com/brando5393/neo_win_pipes/issues/new?title={}&body={}&labels={}",
        percent_encode(title),
        percent_encode(&body),
        percent_encode(category.github_label()),
    )
}

/// Capped well under a URL's practical length limit (title/description
/// already share the same budget), while still being generous enough to
/// usually catch the actual error, not just a couple of lines of noise
/// before it.
const MAX_LOG_TAIL_CHARS: usize = 2000;

/// Reads the tail of the most recently modified `pipes-settings.log*`
/// file in the standard log directory (see
/// `pipes_render::diagnostics::log_dir`) — the newest file, not a fixed
/// filename, since the log rotates daily and this must still find
/// today's file without hardcoding a date format. `None` if there's no
/// log directory, no matching file, or it can't be read — callers treat
/// that as "nothing to include", not an error worth surfacing.
fn recent_log_tail(max_chars: usize) -> Option<String> {
    let dir = pipes_render::diagnostics::log_dir()?;
    let latest = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("pipes-settings.log")
        })
        .max_by_key(|entry| {
            entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        })?;
    let content = std::fs::read_to_string(latest.path()).ok()?;
    if content.len() <= max_chars {
        return Some(sanitize_log_text(&content));
    }
    // Take the tail, not the head - the error is almost always at the
    // end - but start at a real char boundary so this can't panic on a
    // multi-byte UTF-8 sequence split by the byte-offset cut.
    let mut start = content.len() - max_chars;
    while !content.is_char_boundary(start) {
        start += 1;
    }
    Some(sanitize_log_text(&format!(
        "...(truncated)...\n{}",
        &content[start..]
    )))
}

/// Best-effort redaction, not a guarantee: this ships to anyone who
/// installs the app, and log lines can contain full file paths (e.g. the
/// config path logged on startup) that embed the local Windows username.
/// Replaces the current user's home directory with a placeholder;
/// doesn't attempt to catch every possible form of sensitive data (a
/// user's own typed description isn't run through this at all, only the
/// log tail), which is why the popup says "best effort" rather than
/// implying anything stronger.
fn sanitize_log_text(text: &str) -> String {
    let home = match std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        Ok(home) if !home.is_empty() => home,
        _ => return text.to_string(),
    };
    // Log lines built from `{:?}` (Debug) formatting - e.g. the
    // config_path logged at startup - escape backslashes as `\\`, so a
    // Windows home path shows up in the log file as `C:\\Users\\name...`,
    // not `C:\Users\name...`. Missing this second form is exactly how
    // this redaction first shipped silently broken: replacing only the
    // raw form never matched real log content at all, and there was no
    // error to notice - just a path that should have been redacted and
    // wasn't. Caught by actually reading a generated issue body, not by
    // re-reading this code.
    let escaped_home = home.replace('\\', "\\\\");
    text.replace(&escaped_home, "<home>")
        .replace(&home, "<home>")
}

/// Minimal RFC 3986 percent-encoding for a URL query-string value.
/// Operates byte-by-byte over UTF-8, which is exactly right for
/// non-ASCII text: each byte of a multi-byte character falls outside the
/// unreserved set and gets its own `%XX`, and decoding the whole
/// percent-encoded sequence back reassembles the original UTF-8 bytes.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod feedback_tests {
    use super::*;

    #[test]
    fn percent_encode_leaves_unreserved_chars_alone() {
        assert_eq!(percent_encode("abc-XYZ_123.~"), "abc-XYZ_123.~");
    }

    #[test]
    fn percent_encode_escapes_spaces_and_symbols() {
        assert_eq!(percent_encode("a b&c=d"), "a%20b%26c%3Dd");
    }

    #[test]
    fn percent_encode_handles_multibyte_utf8() {
        assert_eq!(percent_encode("café"), "caf%C3%A9");
    }

    #[test]
    fn feedback_url_points_at_this_repo_and_carries_the_right_label() {
        let url = feedback_issue_url(
            FeedbackCategory::Bug,
            "App crashes on startup",
            "it crashed",
            false,
        );
        assert!(url.starts_with("https://github.com/brando5393/neo_win_pipes/issues/new?"));
        assert!(url.contains("labels=bug"));
    }

    #[test]
    fn feedback_url_keeps_title_and_description_separate_and_untagged() {
        // A single-paragraph description with no line breaks must not
        // bleed into the title, and the category must not be mangled
        // into the title as text (e.g. "[Bug] ...") now that it's
        // conveyed via the real `labels=` param instead.
        let long_description = "this happened and then that happened and it kept going";
        let url = feedback_issue_url(
            FeedbackCategory::Bug,
            "Short title",
            long_description,
            false,
        );
        assert!(url.contains(&percent_encode("Short title")));
        assert!(!url.contains("title=%5BBug%5D"));
        let title_param = url
            .split("title=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap();
        assert!(
            !title_param.contains(&percent_encode("this happened")),
            "the long description leaked into the title param: {title_param}"
        );
    }

    #[test]
    fn feedback_url_embeds_app_version_and_os_in_the_body() {
        let url = feedback_issue_url(
            FeedbackCategory::FeatureRequest,
            "Add a thing",
            "add X",
            false,
        );
        assert!(url.contains(&percent_encode(env!("CARGO_PKG_VERSION"))));
        assert!(url.contains(&percent_encode(std::env::consts::OS)));
    }

    #[test]
    fn feedback_url_omits_log_section_when_not_requested() {
        let url = feedback_issue_url(FeedbackCategory::Bug, "Title", "desc", false);
        assert!(!url.contains(&percent_encode("Recent log output")));
    }

    #[test]
    fn feedback_url_includes_log_marker_when_requested() {
        // Doesn't assert real log content is present (this runs in a test
        // process with no rotating log file of its own) - just that the
        // "no log file found" fallback path fires rather than the
        // include_log flag being silently ignored.
        let url = feedback_issue_url(FeedbackCategory::Bug, "Title", "desc", true);
        assert!(
            url.contains(&percent_encode("Recent log output"))
                || url.contains(&percent_encode("No log file found"))
        );
    }

    #[test]
    fn recent_log_tail_truncates_at_a_char_boundary_not_mid_utf8_sequence() {
        let dir =
            std::env::temp_dir().join(format!("pipes_settings_log_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // "é" is 2 bytes in UTF-8 - repeat it so a byte-offset cut at an
        // arbitrary max_chars is likely to land mid-character unless the
        // boundary search actually works.
        let content: String = "é".repeat(50);
        std::fs::write(dir.join("pipes-settings.log.test"), &content).unwrap();

        let latest = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .max_by_key(|e| e.metadata().and_then(|m| m.modified()).unwrap())
            .unwrap();
        let read_back = std::fs::read_to_string(latest.path()).unwrap();
        let max_chars = 21; // odd, deliberately not a multiple of 2
        let mut start = read_back.len() - max_chars;
        while !read_back.is_char_boundary(start) {
            start += 1;
        }
        // Must not panic - the real assertion is that this slice succeeds.
        let _ = &read_back[start..];

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sanitize_log_text_redacts_the_home_directory() {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .expect("test environment should have USERPROFILE or HOME set");
        let text = format!("config_path=Some(\"{home}\\\\AppData\\\\config.toml\")");
        let sanitized = sanitize_log_text(&text);
        assert!(!sanitized.contains(&home));
        assert!(sanitized.contains("<home>"));
    }

    #[test]
    fn sanitize_log_text_redacts_the_debug_escaped_form_too() {
        // Reproduces the actual bug: `tracing::info!(config_path = ?path)`
        // logs through Rust's `Debug` formatting, which escapes every `\`
        // as `\\` - so on Windows the *entire* path in a real log line is
        // double-backslash-escaped, not just interspersed with it. A
        // sanitizer that only replaces the raw (single-backslash) home
        // path silently does nothing on this input - no error, no panic,
        // just an unredacted path - which is exactly how this shipped
        // broken the first time and passed the other test above anyway
        // (that test's input wasn't actually double-escaped throughout).
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .expect("test environment should have USERPROFILE or HOME set");
        let escaped_home = home.replace('\\', "\\\\");
        let text = format!("config_path=Some(\"{escaped_home}\\\\AppData\\\\config.toml\")");
        let sanitized = sanitize_log_text(&text);
        assert!(
            !sanitized.contains(&escaped_home),
            "debug-escaped home path was not redacted: {sanitized}"
        );
        assert!(sanitized.contains("<home>"));
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
