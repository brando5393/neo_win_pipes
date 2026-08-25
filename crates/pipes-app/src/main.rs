//! The screensaver itself. Loads `AppConfig` (shared with `pipes-settings`
//! — see docs/ARCHITECTURE.md) and renders the pipes simulation via
//! pipes-render. Understands the Windows screensaver contract
//! (`screensaver_args.rs`): run `neo_win_pipes.scr /s` for fullscreen,
//! `/c` to open the settings app, `/p <hwnd>` to render a live preview
//! into an existing window (what Windows does for the thumbnail in
//! Settings' screensaver dropdown). With no recognized flag (e.g. plain
//! `cargo run -p pipes-app`), behaves like `/s` for local testing. Press
//! Escape or close the window to quit in any mode; in `/s` mode, any key
//! press, click, or mouse movement also quits, matching how every real
//! screensaver behaves.
//!
//! The RNG seed defaults to a time-based value each run (see
//! `rand_seed`), since that's what every real activation actually gets —
//! Windows never passes `--seed` — so it's what local testing should see
//! too unless a reproducible run is specifically what's being tested. Pass
//! `--seed <n>` explicitly for that (e.g. `cargo run -p pipes-app --
//! --seed 1`).

// Debug builds keep the console (so `cargo run`/`tracing` output is visible
// in the terminal); release builds drop it. Without this, Rust defaults to
// the console subsystem on Windows for every binary, so both `/s` and `/c`
// pop up a visible console window that comes to the foreground — and
// closing that console window sends its default control handler a close
// event that kills this whole process, not just the console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod screensaver_args;
#[cfg(windows)]
mod winsaver;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::Mat4;
use pipes_core::{Scene, SceneEvent};
use pipes_render::{build_instances, tile_projection, AppConfig, MonitorMode, Renderer};
use screensaver_args::{parse_screensaver_args, ScreensaverMode};
use tracing::info;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowBuilder, WindowId};

// winit's cross-platform `with_window_icon` only sets the small (title
// bar) icon on Windows — the taskbar button uses a separate "big" icon
// that has no cross-platform builder method, only this Windows-specific
// extension trait. Without it, the taskbar button shows Windows' generic
// default icon even though the title bar shows the right one.
#[cfg(windows)]
use winit::platform::windows::WindowBuilderExtWindows;

/// Derives a distinct-but-deterministic RNG seed for the `index`-th
/// monitor's independent instance, so multiple displays don't render
/// identical mirrored scenes — while a given `--seed` still reproduces the
/// exact same multi-monitor run every time (determinism is load-bearing
/// project-wide, see `docs/ARCHITECTURE.md`). The multiplier is an
/// arbitrary large prime purely to spread indices apart in the seed space;
/// it carries no other significance.
fn seed_for_monitor(base_seed: u64, index: usize) -> u64 {
    base_seed.wrapping_add(index as u64 * 104_729)
}

/// `(x, y, width, height)` in physical pixels, winit's `MonitorHandle`
/// convention — Y grows downward, position can be negative for a monitor
/// arranged left of or above the primary display.
type PixelRect = (i32, i32, u32, u32);
/// `(x, y, width, height)`, same shape as [`PixelRect`] but `f32` — the
/// units a computed tile projection expects (see
/// `pipes_render::tile::tile_projection`).
type Rect = (f32, f32, f32, f32);

/// The union bounding box of every monitor's rect — the `MonitorMode::Span`
/// virtual canvas — plus each monitor's own rect translated into that box's
/// local coordinate space (so the box's own origin is always `(0, 0)`, and
/// tiles never need negative coordinates even when a monitor sits to the
/// left of or above the primary display in the OS's arrangement). Returns
/// `(canvas_width, canvas_height)` and one `(x, y, width, height)` tile per
/// input rect, same order in, same order out.
///
/// Takes plain `(x, y, width, height)` rects rather than
/// `winit::monitor::MonitorHandle` directly — `MonitorHandle` has no public
/// constructor and can't be built in a unit test without a live windowing
/// system, so the actual layout math is kept independent of it and callers
/// translate real monitors into rects at the call site.
fn virtual_canvas(rects: &[PixelRect]) -> ((f32, f32), Vec<Rect>) {
    let min_x = rects.iter().map(|&(x, _, _, _)| x).min().unwrap_or(0);
    let min_y = rects.iter().map(|&(_, y, _, _)| y).min().unwrap_or(0);
    let max_x = rects
        .iter()
        .map(|&(x, _, w, _)| x + w as i32)
        .max()
        .unwrap_or(0);
    let max_y = rects
        .iter()
        .map(|&(_, y, _, h)| y + h as i32)
        .max()
        .unwrap_or(0);
    let canvas = ((max_x - min_x).max(1) as f32, (max_y - min_y).max(1) as f32);
    let tiles = rects
        .iter()
        .map(|&(x, y, w, h)| ((x - min_x) as f32, (y - min_y) as f32, w as f32, h as f32))
        .collect();
    (canvas, tiles)
}

/// Builds one borderless window: fullscreen on `monitor` for
/// `ScreensaverMode::Show` (`None` lets the OS pick, used for the
/// single-instance fallback path), or a small fixed-size window otherwise
/// (`/c` preview thumbnail, or local dev testing).
fn build_window(
    event_loop: &EventLoop<()>,
    mode: ScreensaverMode,
    monitor: Option<MonitorHandle>,
) -> Arc<Window> {
    let mut builder = WindowBuilder::new()
        .with_title("neo_win_pipes")
        .with_window_icon(pipes_render::app_icon::window_icon())
        .with_decorations(false);
    #[cfg(windows)]
    {
        builder = builder.with_taskbar_icon(pipes_render::app_icon::window_icon());
    }
    builder = match mode {
        ScreensaverMode::Show => builder.with_fullscreen(Some(Fullscreen::Borderless(monitor))),
        _ => builder.with_inner_size(LogicalSize::new(320.0, 240.0)),
    };
    Arc::new(builder.build(event_loop).expect("failed to create window"))
}

/// One monitor's worth of screensaver state under `MonitorMode::AllMonitors`:
/// its own window, GPU renderer, and independent simulation.
struct Instance {
    window: Arc<Window>,
    renderer: Renderer,
    scene: Scene,
    last_tick: Instant,
}

/// One monitor's worth of screensaver state under `MonitorMode::Span`: its
/// own window and GPU renderer, but no `Scene` of its own — every window
/// shares one `Scene` (see [`Rendering::Span`]) and only differs in
/// `tile_projection`, its slice of that shared scene's virtual canvas.
struct SpanWindow {
    window: Arc<Window>,
    renderer: Renderer,
    tile_projection: Mat4,
}

/// The two shapes multi-monitor rendering can take, dispatched on once at
/// startup from `AppConfig::monitor_mode` — see
/// `docs/ARCHITECTURE.md#multi-monitor-behavior` for why these differ
/// structurally (independent scenes vs. one shared scene) rather than just
/// being a rendering-only switch.
enum Rendering {
    Independent(Vec<Instance>),
    Span {
        windows: Vec<SpanWindow>,
        // Boxed so this variant isn't dramatically larger than
        // `Independent`'s (a bare `Scene` would make every `Rendering`
        // value pay for the biggest variant's size, per clippy's
        // large_enum_variant lint) — otherwise no different from an
        // unboxed field.
        scene: Box<Scene>,
        last_tick: Instant,
    },
}

impl Rendering {
    fn window_ids(&self) -> HashMap<WindowId, usize> {
        match self {
            Rendering::Independent(instances) => instances
                .iter()
                .enumerate()
                .map(|(i, inst)| (inst.window.id(), i))
                .collect(),
            Rendering::Span { windows, .. } => windows
                .iter()
                .enumerate()
                .map(|(i, w)| (w.window.id(), i))
                .collect(),
        }
    }

    fn request_redraw_all(&self) {
        match self {
            Rendering::Independent(instances) => {
                for inst in instances {
                    inst.window.request_redraw();
                }
            }
            Rendering::Span { windows, .. } => {
                for w in windows {
                    w.window.request_redraw();
                }
            }
        }
    }

    fn len(&self) -> usize {
        match self {
            Rendering::Independent(instances) => instances.len(),
            Rendering::Span { windows, .. } => windows.len(),
        }
    }
}

/// The RNG seed explicitly requested via `--seed <n>`, if any and if it
/// parses — `None` otherwise (including a malformed value, which is
/// treated the same as not passing `--seed` at all rather than silently
/// falling back to some other number). If `--seed` appears more than
/// once, the last one that actually parses wins, matching how repeated
/// flags are conventionally resolved.
fn parse_seed_arg(args: &[String]) -> Option<u64> {
    let mut seed = None;
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        if flag == "--seed" {
            if let Some(v) = iter.next() {
                if let Ok(parsed) = v.parse() {
                    seed = Some(parsed);
                }
            }
        }
    }
    seed
}

/// Time-based fallback seed for a real run with no explicit `--seed`.
/// Windows never passes one when it launches the actual `/s` screensaver
/// — without this, every single activation would replay the exact same
/// bit-for-bit pipe-growth pattern forever, which is a real bug rather
/// than a missing feature: a screensaver's whole appeal is a fresh
/// pattern each time, not determinism. Mirrors `pipes-settings`'
/// `rand_seed()` and `pipes-xscreensaver`'s equivalent; `--seed` stays
/// available as an explicit override for reproducible manual testing.
fn rand_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
}

/// Candidate locations for `pipes-settings`, tried in order: (1) next to
/// this executable, for dev/testing convenience when both binaries sit
/// in the same folder (`cargo run`/`target/debug`); (2) the installed
/// location once packaged (Phase 4) — the real screensaver lives in
/// `System32` as a renamed `.scr` per Windows' own convention, but the
/// settings app installs like a normal app in
/// `%ProgramFiles%\neo_win_pipes\`, so this fallback finds it there even
/// when this exe is itself running from `System32`.
fn settings_app_candidates() -> Vec<std::path::PathBuf> {
    let exe_name = if cfg!(windows) {
        "pipes-settings.exe"
    } else {
        "pipes-settings"
    };
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.to_path_buf()))
    {
        candidates.push(dir.join(exe_name));
    }
    // Check every "Program Files" variant, not just %ProgramFiles%: which
    // one that resolves to depends on both this process's bitness and the
    // installer's declared platform, and those two have disagreed before
    // (the release MSI was built without `-arch x64`, so WiX defaulted to
    // x86 and installed into "Program Files (x86)" while this native
    // 64-bit process's %ProgramFiles% pointed at plain "Program Files" —
    // the real cause of a real "Settings doesn't open" bug). Checking all
    // three means a future build/installer mismatch degrades to "slightly
    // redundant lookup" instead of "silently broken".
    for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Ok(program_files) = std::env::var(var) {
            candidates.push(
                std::path::Path::new(&program_files)
                    .join("neo_win_pipes")
                    .join(exe_name),
            );
        }
    }
    candidates
}

fn run_configure() {
    let candidates = settings_app_candidates();
    match candidates.iter().find(|p| p.exists()) {
        Some(path) => match std::process::Command::new(path).spawn() {
            Ok(_) => info!(path = %path.display(), "launched pipes-settings"),
            Err(err) => {
                tracing::error!(?err, path = %path.display(), "failed to launch pipes-settings")
            }
        },
        None => tracing::error!(
            ?candidates,
            "pipes-settings not found in any known location"
        ),
    }
}

fn main() {
    // Held for the whole process lifetime: dropping it stops the file
    // logger's background writer from flushing further lines.
    let _log_guard = pipes_render::diagnostics::init_logging("pipes-app");
    pipes_render::diagnostics::install_panic_hook("neo_win_pipes");

    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let mode = parse_screensaver_args(&raw_args);
    let seed = parse_seed_arg(&raw_args).unwrap_or_else(rand_seed);

    info!(?mode, "neo_win_pipes starting");

    match mode {
        ScreensaverMode::Configure => return run_configure(),
        ScreensaverMode::ChangePassword => {
            info!("change-password invocation not applicable on modern Windows; no-op");
            return;
        }
        ScreensaverMode::Show | ScreensaverMode::Preview(_) => {}
    }

    let mut app_config = AppConfig::load();
    app_config.sanitize();
    info!(config_path = ?AppConfig::config_path(), "loaded AppConfig (or defaults if missing)");

    let event_loop = EventLoop::new().expect("failed to create event loop");
    let bounds = (
        app_config.sim.bounds.width,
        app_config.sim.bounds.height,
        app_config.sim.bounds.depth,
    );

    // Multi-monitor only ever applies to the real fullscreen screensaver —
    // the /c settings-launch path already returned above, and /p's preview
    // thumbnail and configure-mode both render into one caller-provided or
    // small dev window regardless of how many displays exist.
    let monitors: Vec<MonitorHandle> = if mode == ScreensaverMode::Show
        && matches!(
            app_config.monitor_mode,
            MonitorMode::AllMonitors | MonitorMode::Span
        ) {
        event_loop.available_monitors().collect()
    } else {
        Vec::new()
    };

    let mut rendering = if monitors.is_empty() {
        // PrimaryOnly, or /p's preview thumbnail, or /c-adjacent dev
        // testing — a single window regardless of how many displays exist.
        let window = build_window(&event_loop, mode, None);
        if mode == ScreensaverMode::Show {
            window.set_cursor_visible(false);
        }
        if let ScreensaverMode::Preview(hwnd) = mode {
            #[cfg(windows)]
            winsaver::embed_in_preview(&window, hwnd);
            #[cfg(not(windows))]
            {
                let _ = hwnd;
                tracing::warn!("/p preview embedding is only implemented on Windows");
            }
        }
        let window_size = window.inner_size();
        let renderer = pollster::block_on(Renderer::new(
            window.clone(),
            (window_size.width, window_size.height),
            bounds,
        ));
        let scene = Scene::new(app_config.sim.clone(), seed);
        Rendering::Independent(vec![Instance {
            window,
            renderer,
            scene,
            last_tick: Instant::now(),
        }])
    } else if app_config.monitor_mode == MonitorMode::Span {
        info!(
            count = monitors.len(),
            "multi-monitor: spanning one shared scene across every display"
        );
        let rects: Vec<(i32, i32, u32, u32)> = monitors
            .iter()
            .map(|m| {
                let pos = m.position();
                let size = m.size();
                (pos.x, pos.y, size.width, size.height)
            })
            .collect();
        let (canvas_wh, tiles) = virtual_canvas(&rects);
        let mut windows = Vec::new();
        // Built once, from the first window: every Renderer shares the
        // same sim bounds, so `frustum_params()` (derived purely from
        // those bounds) is identical no matter which one supplies it.
        let mut frustum_params = None;
        for (monitor, tile_rect) in monitors.into_iter().zip(tiles) {
            let window = build_window(&event_loop, mode, Some(monitor));
            window.set_cursor_visible(false);
            let window_size = window.inner_size();
            let renderer = pollster::block_on(Renderer::new(
                window.clone(),
                (window_size.width, window_size.height),
                bounds,
            ));
            let (fov_y, near, far) =
                *frustum_params.get_or_insert_with(|| renderer.frustum_params());
            let projection = tile_projection(fov_y, near, far, canvas_wh, tile_rect);
            windows.push(SpanWindow {
                window,
                renderer,
                tile_projection: projection,
            });
        }
        let scene = Box::new(Scene::new(app_config.sim.clone(), seed));
        Rendering::Span {
            windows,
            scene,
            last_tick: Instant::now(),
        }
    } else {
        info!(
            count = monitors.len(),
            "multi-monitor: spawning one independent instance per display"
        );
        let mut instances = Vec::new();
        for (i, monitor) in monitors.into_iter().enumerate() {
            let window = build_window(&event_loop, mode, Some(monitor));
            window.set_cursor_visible(false);
            let window_size = window.inner_size();
            let renderer = pollster::block_on(Renderer::new(
                window.clone(),
                (window_size.width, window_size.height),
                bounds,
            ));
            let scene = Scene::new(app_config.sim.clone(), seed_for_monitor(seed, i));
            instances.push(Instance {
                window,
                renderer,
                scene,
                last_tick: Instant::now(),
            });
        }
        Rendering::Independent(instances)
    };
    let window_ids = rendering.window_ids();

    info!(
        seed,
        instances = rendering.len(),
        "neo_win_pipes window(s) opened"
    );
    let start = Instant::now();
    let tick_interval = Duration::from_millis(app_config.tick_interval_ms as u64);
    let exit_on_any_input = mode == ScreensaverMode::Show;
    // Window creation itself generates a synthetic CursorMoved (the OS
    // reporting where the cursor already was) and can replay a stray
    // KeyboardInput/MouseInput too — without this grace period, /s mode
    // would exit almost instantly on its own startup events rather than
    // real user input. Verified by hitting exactly this bug: the window
    // closed within milliseconds, before a single frame rendered.
    const INPUT_GRACE: Duration = Duration::from_millis(750);
    let exit_on_input_now =
        move |start: Instant| exit_on_any_input && start.elapsed() > INPUT_GRACE;

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { event, window_id } => {
                let Some(&idx) = window_ids.get(&window_id) else {
                    return;
                };
                // Exit conditions don't depend on which Rendering variant is
                // active, so they're checked once up front rather than
                // duplicated in both branches below.
                match &event {
                    WindowEvent::CloseRequested => return elwt.exit(),
                    WindowEvent::KeyboardInput { .. } if exit_on_input_now(start) => {
                        return elwt.exit()
                    }
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        ..
                    } if exit_on_input_now(start) => return elwt.exit(),
                    WindowEvent::CursorMoved { .. } if exit_on_input_now(start) => {
                        return elwt.exit()
                    }
                    _ => {}
                }
                match &mut rendering {
                    Rendering::Independent(instances) => {
                        let inst = &mut instances[idx];
                        match event {
                            WindowEvent::Resized(size) => {
                                inst.renderer.resize(size.width, size.height)
                            }
                            WindowEvent::RedrawRequested => {
                                if inst.last_tick.elapsed() >= tick_interval {
                                    inst.last_tick = Instant::now();
                                    for event in inst.scene.step() {
                                        if event == SceneEvent::SceneReset {
                                            info!("scene reset — restarting");
                                        }
                                    }
                                }

                                // Checked first, separately from the ordinary
                                // SurfaceError match below: once the GPU
                                // device is actually gone (driver reset,
                                // sleep/wake, etc. - see
                                // Renderer::is_device_lost's doc), calling
                                // `resize` here would itself call into the
                                // same dead device and hit the same panic
                                // this whole check exists to avoid.
                                // `recover_if_needed` attempts a real hot
                                // recovery (new Device/Queue/Surface) once
                                // per loss episode before giving up and
                                // just freezing on the last good frame.
                                if inst.renderer.recover_if_needed() {
                                    let sets = build_instances(&inst.scene, &app_config.visuals);
                                    let orbit_seconds = start.elapsed().as_secs_f32();
                                    if let Err(err) = inst.renderer.render(
                                        orbit_seconds,
                                        &app_config.camera,
                                        None,
                                        &sets,
                                    ) {
                                        match err {
                                            wgpu::SurfaceError::Lost
                                            | wgpu::SurfaceError::Outdated => {
                                                let size = inst.window.inner_size();
                                                inst.renderer.resize(size.width, size.height);
                                            }
                                            wgpu::SurfaceError::OutOfMemory => elwt.exit(),
                                            wgpu::SurfaceError::Timeout => {}
                                        }
                                    }
                                }
                                inst.window.request_redraw();
                            }
                            _ => {}
                        }
                    }
                    Rendering::Span {
                        windows,
                        scene,
                        last_tick,
                    } => {
                        let win = &mut windows[idx];
                        match event {
                            WindowEvent::Resized(size) => {
                                win.renderer.resize(size.width, size.height)
                            }
                            WindowEvent::RedrawRequested => {
                                // Gated on one shared `last_tick`, not a
                                // per-window one: there's only one Scene, so
                                // it must step once per tick_interval total,
                                // not once per window's own redraw cadence.
                                if last_tick.elapsed() >= tick_interval {
                                    *last_tick = Instant::now();
                                    for event in scene.step() {
                                        if event == SceneEvent::SceneReset {
                                            info!("scene reset — restarting");
                                        }
                                    }
                                }

                                // See the matching comment in the
                                // AllMonitors/PrimaryOnly branch above: a
                                // real device loss must never reach
                                // `resize` (which would call into the same
                                // dead device), so it's checked (and hot
                                // recovery attempted) separately and first.
                                if win.renderer.recover_if_needed() {
                                    let sets = build_instances(scene, &app_config.visuals);
                                    let orbit_seconds = start.elapsed().as_secs_f32();
                                    if let Err(err) = win.renderer.render_tile(
                                        orbit_seconds,
                                        &app_config.camera,
                                        win.tile_projection,
                                        &sets,
                                    ) {
                                        match err {
                                            wgpu::SurfaceError::Lost
                                            | wgpu::SurfaceError::Outdated => {
                                                let size = win.window.inner_size();
                                                win.renderer.resize(size.width, size.height);
                                            }
                                            wgpu::SurfaceError::OutOfMemory => elwt.exit(),
                                            wgpu::SurfaceError::Timeout => {}
                                        }
                                    }
                                }
                                win.window.request_redraw();
                            }
                            _ => {}
                        }
                    }
                }
            }
            Event::AboutToWait => rendering.request_redraw_all(),
            _ => {}
        })
        .expect("event loop error");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_seed_arg_is_none_when_not_passed() {
        assert_eq!(parse_seed_arg(&args(&["/s"])), None);
        assert_eq!(parse_seed_arg(&args(&[])), None);
    }

    #[test]
    fn parse_seed_arg_reads_an_explicit_value() {
        assert_eq!(parse_seed_arg(&args(&["--seed", "42"])), Some(42));
    }

    #[test]
    fn parse_seed_arg_ignores_a_malformed_value_rather_than_defaulting() {
        // A garbage --seed value must not silently claim some other
        // number is what was requested — it should read the same as not
        // passing --seed at all, so the real-run random fallback kicks in
        // rather than a fake "deterministic" run nobody actually asked for.
        assert_eq!(parse_seed_arg(&args(&["--seed", "not-a-number"])), None);
    }

    #[test]
    fn parse_seed_arg_with_nothing_after_the_flag_is_none() {
        assert_eq!(parse_seed_arg(&args(&["--seed"])), None);
    }

    #[test]
    fn parse_seed_arg_keeps_the_last_flag_that_actually_parses() {
        assert_eq!(
            parse_seed_arg(&args(&["--seed", "5", "--seed", "bogus"])),
            Some(5),
            "a later malformed --seed shouldn't erase an earlier valid one"
        );
        assert_eq!(
            parse_seed_arg(&args(&["--seed", "5", "--seed", "9"])),
            Some(9),
            "a later valid --seed should override an earlier one"
        );
    }

    #[test]
    fn seed_for_monitor_index_zero_is_the_base_seed() {
        // The first monitor must reproduce today's single-monitor behavior
        // exactly, so existing `--seed` reproductions don't change.
        assert_eq!(seed_for_monitor(1, 0), 1);
        assert_eq!(seed_for_monitor(9999, 0), 9999);
    }

    #[test]
    fn seed_for_monitor_gives_each_display_a_distinct_seed() {
        let seeds: Vec<u64> = (0..4).map(|i| seed_for_monitor(1, i)).collect();
        let mut unique = seeds.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            seeds.len(),
            "every monitor index must get a distinct seed"
        );
    }

    #[test]
    fn seed_for_monitor_is_deterministic() {
        assert_eq!(seed_for_monitor(42, 2), seed_for_monitor(42, 2));
    }

    #[test]
    fn virtual_canvas_places_two_side_by_side_monitors_edge_to_edge() {
        let rects = [(0, 0, 1920, 1080), (1920, 0, 1920, 1080)];
        let (canvas, tiles) = virtual_canvas(&rects);
        assert_eq!(canvas, (3840.0, 1080.0));
        assert_eq!(
            tiles,
            vec![(0.0, 0.0, 1920.0, 1080.0), (1920.0, 0.0, 1920.0, 1080.0)]
        );
    }

    #[test]
    fn virtual_canvas_handles_a_monitor_arranged_to_the_left_of_the_primary() {
        // A secondary display placed to the left of (0,0) in the OS's
        // arrangement has negative x — tiles must still come out
        // non-negative, translated into the canvas's own local origin.
        let rects = [(0, 0, 1920, 1080), (-1280, 0, 1280, 1080)];
        let (canvas, tiles) = virtual_canvas(&rects);
        assert_eq!(canvas, (3200.0, 1080.0));
        assert_eq!(tiles[0], (1280.0, 0.0, 1920.0, 1080.0));
        assert_eq!(tiles[1], (0.0, 0.0, 1280.0, 1080.0));
    }

    #[test]
    fn virtual_canvas_of_a_single_monitor_is_just_that_monitor() {
        let rects = [(0, 0, 2560, 1440)];
        let (canvas, tiles) = virtual_canvas(&rects);
        assert_eq!(canvas, (2560.0, 1440.0));
        assert_eq!(tiles, vec![(0.0, 0.0, 2560.0, 1440.0)]);
    }
}
