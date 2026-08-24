//! Linux xscreensaver "hack" wrapper: renders into whatever X11 window
//! xscreensaver's driver hands us (`-window-id <id>`), or the root window
//! for standalone/manual testing (`-root`, or no recognized flag at all).
//! Reuses the exact same `pipes-core`/`pipes-render` pipeline `pipes-app`
//! does on Windows — only the windowing/surface-creation layer differs
//! (see `x11_target.rs`).
//!
//! Unlike `pipes-app`'s `/s` mode, a hack doesn't exit on its own input:
//! `xscreensaver`'s driver owns the window's lifecycle (it decides when to
//! start/stop hacks, e.g. on real user activity) and just sends this
//! process `SIGTERM` when it's done with it — the OS default handler for
//! that already exits the process, so there's nothing extra to wire up
//! here.
//!
//! **Verification caveat**, stated plainly rather than implied away: this
//! is real, complete code, not a stub — but it's only been type-checked
//! and clippy-checked against the `x86_64-unknown-linux-gnu` target from
//! a Windows machine with no X server to actually run it against. See
//! `x11_target.rs` and `docs/ROADMAP.md` for what that does and doesn't
//! prove.

mod args;
#[cfg(target_os = "linux")]
mod x11_target;

use args::parse_xscreensaver_args;

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .compact()
        .init();
}

#[cfg(target_os = "linux")]
fn main() {
    use std::time::{Duration, Instant};

    use pipes_core::{Scene, SceneEvent};
    use pipes_render::{build_instances, AppConfig, Renderer};
    use x11_target::X11Target;

    init_logging();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let target_window = parse_xscreensaver_args(&args);
    tracing::info!(?target_window, "pipes-xscreensaver starting");

    let target = std::sync::Arc::new(X11Target::open(target_window));

    let mut app_config = AppConfig::load();
    app_config.sanitize();
    tracing::info!(config_path = ?AppConfig::config_path(), "loaded AppConfig (or defaults if missing)");

    let bounds = (
        app_config.sim.bounds.width,
        app_config.sim.bounds.height,
        app_config.sim.bounds.depth,
    );
    // A hack should look different each run, unlike pipes-app's dev-mode
    // default seed=1 — there's no equivalent to a fixed --seed convention
    // in the xscreensaver hack contract to override this with, so a
    // time-based seed (same approach pipes-settings' live preview uses)
    // is the right default here, not a debugging aid to remove later.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let mut scene = Scene::new(app_config.sim.clone(), seed);

    let mut renderer = pollster::block_on(Renderer::new(target.clone(), target.size(), bounds));

    let start = Instant::now();
    let mut last_tick = Instant::now();
    let tick_interval = Duration::from_millis(app_config.tick_interval_ms as u64);

    tracing::info!(seed, "pipes-xscreensaver render loop starting");
    loop {
        if let Some((width, height)) = target.poll_resize() {
            renderer.resize(width, height);
        }

        if last_tick.elapsed() >= tick_interval {
            last_tick = Instant::now();
            for event in scene.step() {
                if event == SceneEvent::SceneReset {
                    tracing::info!("scene reset — restarting");
                }
            }
        }

        // See the matching comment in pipes-app/src/main.rs: a real device
        // loss must never reach `resize` (which would call into the same
        // dead device), so it's checked separately and first.
        if !renderer.is_device_lost() {
            let sets = build_instances(&scene, &app_config.visuals);
            let orbit_seconds = start.elapsed().as_secs_f32();
            if let Err(err) = renderer.render(orbit_seconds, &app_config.camera, None, &sets) {
                match err {
                    wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                        let (w, h) = target.size();
                        renderer.resize(w, h);
                    }
                    wgpu::SurfaceError::OutOfMemory => {
                        tracing::error!("GPU out of memory, exiting");
                        return;
                    }
                    wgpu::SurfaceError::Timeout => {}
                }
            }
        }

        // Not a fixed frame-rate sleep: tick_interval already governs
        // simulation speed above, so this just caps how often we redraw
        // between ticks — matching pipes-app's per-frame redraw cadence
        // without needing a winit event loop to drive it.
        std::thread::sleep(Duration::from_millis(16));
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    init_logging();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let target = parse_xscreensaver_args(&args);
    tracing::info!(?target, "resolved target (parsing only works here)");

    tracing::error!(
        "pipes-xscreensaver only renders on Linux (it's an xscreensaver 'hack' wrapper, \
         a Linux/BSD-specific concept) — argument parsing is cross-platform and tested, \
         but this OS has no X11 rendering path. See docs/ROADMAP.md."
    );
    std::process::exit(1);
}
