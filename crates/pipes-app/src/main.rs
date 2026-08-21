//! Phase 1 entry point: runs the pipes-core simulation headlessly and logs
//! it in human-readable form. There is no window or renderer yet — this
//! exists to exercise and observe the engine end-to-end before Phase 2 adds
//! a wgpu-based window (see docs/ROADMAP.md). Run with `cargo run -p
//! pipes-app -- --ticks 500 --seed 1`.

use std::time::Instant;

use pipes_core::{Scene, SceneEvent, SimConfig};
use tracing::info;

struct Args {
    ticks: u64,
    seed: u64,
}

fn parse_args() -> Args {
    let mut ticks = 200u64;
    let mut seed = 1u64;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--ticks" => {
                if let Some(v) = args.next() {
                    ticks = v.parse().unwrap_or(ticks);
                }
            }
            "--seed" => {
                if let Some(v) = args.next() {
                    seed = v.parse().unwrap_or(seed);
                }
            }
            other => {
                eprintln!("warning: ignoring unknown argument '{other}'");
            }
        }
    }
    Args { ticks, seed }
}

fn init_logging() {
    // Human-readable, single-line-per-event output with target + level, no
    // JSON. See docs/LOGGING.md for the full field/event catalog and the
    // rationale for keeping this readable-first rather than machine-first.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .compact()
        .init();
}

fn main() {
    init_logging();
    let args = parse_args();

    info!(
        ticks = args.ticks,
        seed = args.seed,
        "neo_win_pipes starting (headless simulation)"
    );
    let start = Instant::now();

    let mut scene = Scene::new(SimConfig::default(), args.seed);
    let mut spawned = 0u64;
    let mut terminated = 0u64;
    let mut resets = 0u64;

    for t in 0..args.ticks {
        for event in scene.step() {
            match event {
                SceneEvent::PipeSpawned { .. } => spawned += 1,
                SceneEvent::PipeTerminated { .. } => terminated += 1,
                SceneEvent::SceneReset => resets += 1,
            }
        }
        if t % 50 == 0 {
            info!(
                tick = t,
                live_pipes = scene.pipes().iter().filter(|p| p.is_alive()).count(),
                occupancy = scene.grid().occupancy_ratio(),
                "tick summary"
            );
        }
    }

    info!(
        elapsed_ms = start.elapsed().as_millis(),
        total_pipes_spawned = spawned,
        total_pipes_terminated = terminated,
        total_resets = resets,
        "neo_win_pipes finished"
    );
}
