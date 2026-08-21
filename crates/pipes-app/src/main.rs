//! The screensaver itself. Loads `AppConfig` (shared with `pipes-settings`
//! — see docs/ARCHITECTURE.md) and renders the pipes simulation via
//! pipes-render. Understands the Windows screensaver contract
//! (`screensaver_args.rs`): run `neo_win_pipes.scr /s` for fullscreen,
//! `/c` to open the settings app, `/p <hwnd>` to render a live preview
//! into an existing window (what Windows does for the thumbnail in
//! Settings' screensaver dropdown). With no recognized flag (e.g. plain
//! `cargo run -p pipes-app -- --seed 1`), behaves like `/s` for local
//! testing. Press Escape or close the window to quit in any mode; in `/s`
//! mode, any key press, click, or mouse movement also quits, matching how
//! every real screensaver behaves.

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

use std::sync::Arc;
use std::time::{Duration, Instant};

use pipes_core::{Scene, SceneEvent};
use pipes_render::{build_instances, AppConfig, Renderer};
use screensaver_args::{parse_screensaver_args, ScreensaverMode};
use tracing::info;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::{Fullscreen, WindowBuilder};

fn parse_seed(args: &[String]) -> u64 {
    let mut seed = 1u64;
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        if flag == "--seed" {
            if let Some(v) = iter.next() {
                seed = v.parse().unwrap_or(seed);
            }
        }
    }
    seed
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .compact()
        .init();
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
    init_logging();
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let mode = parse_screensaver_args(&raw_args);
    let seed = parse_seed(&raw_args);

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
    let mut builder = WindowBuilder::new()
        .with_title("neo_win_pipes")
        .with_decorations(false);
    builder = match mode {
        ScreensaverMode::Show => builder.with_fullscreen(Some(Fullscreen::Borderless(None))),
        _ => builder.with_inner_size(LogicalSize::new(320.0, 240.0)),
    };
    let window = Arc::new(builder.build(&event_loop).expect("failed to create window"));

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

    let bounds = (
        app_config.sim.bounds.width,
        app_config.sim.bounds.height,
        app_config.sim.bounds.depth,
    );
    let mut scene = Scene::new(app_config.sim.clone(), seed);

    let mut renderer = pollster::block_on(Renderer::new(window.clone(), bounds));

    info!(seed, "neo_win_pipes window opened");
    let start = Instant::now();
    let mut last_tick = Instant::now();
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
            Event::WindowEvent { event, window_id } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::KeyboardInput { .. } if exit_on_input_now(start) => elwt.exit(),
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    ..
                } if exit_on_input_now(start) => elwt.exit(),
                WindowEvent::CursorMoved { .. } if exit_on_input_now(start) => elwt.exit(),
                WindowEvent::Resized(size) => renderer.resize(size.width, size.height),
                WindowEvent::RedrawRequested => {
                    if last_tick.elapsed() >= tick_interval {
                        last_tick = Instant::now();
                        for event in scene.step() {
                            if event == SceneEvent::SceneReset {
                                info!("scene reset — restarting");
                            }
                        }
                    }

                    let sets = build_instances(&scene, &app_config.visuals);
                    let orbit_seconds = start.elapsed().as_secs_f32();
                    if let Err(err) =
                        renderer.render(orbit_seconds, &app_config.camera, None, &sets)
                    {
                        match err {
                            wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                                let size = window.inner_size();
                                renderer.resize(size.width, size.height);
                            }
                            wgpu::SurfaceError::OutOfMemory => elwt.exit(),
                            wgpu::SurfaceError::Timeout => {}
                        }
                    }
                    window.request_redraw();
                }
                _ => {}
            },
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .expect("event loop error");
}
