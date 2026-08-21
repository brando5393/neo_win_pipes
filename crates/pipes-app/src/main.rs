//! Windowed entry point: ticks the pipes-core simulation on a fixed
//! interval and renders it with wgpu. Run with `cargo run -p pipes-app --
//! --seed 1`; press Escape or close the window to quit.

mod geometry;
mod instance;
mod renderer;

use std::sync::Arc;
use std::time::{Duration, Instant};

use pipes_core::{Scene, SceneEvent, SimConfig};
use tracing::info;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowBuilder;

use instance::{build_instances, PipeVisuals};
use renderer::Renderer;

struct Args {
    seed: u64,
}

fn parse_args() -> Args {
    let mut seed = 1u64;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--seed" => {
                if let Some(v) = args.next() {
                    seed = v.parse().unwrap_or(seed);
                }
            }
            other => eprintln!("warning: ignoring unknown argument '{other}'"),
        }
    }
    Args { seed }
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

const TICK_INTERVAL: Duration = Duration::from_millis(120);

fn main() {
    init_logging();
    let args = parse_args();

    let event_loop = EventLoop::new().expect("failed to create event loop");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("neo_win_pipes")
            .with_inner_size(LogicalSize::new(1280.0, 800.0))
            .build(&event_loop)
            .expect("failed to create window"),
    );

    let config = SimConfig::default();
    let bounds = (
        config.bounds.width,
        config.bounds.height,
        config.bounds.depth,
    );
    let mut scene = Scene::new(config, args.seed);
    let visuals = PipeVisuals::default();

    let mut renderer = pollster::block_on(Renderer::new(window.clone(), bounds));

    info!(seed = args.seed, "neo_win_pipes window opened");
    let start = Instant::now();
    let mut last_tick = Instant::now();

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(KeyCode::Escape),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => elwt.exit(),
                WindowEvent::Resized(size) => renderer.resize(size.width, size.height),
                WindowEvent::RedrawRequested => {
                    if last_tick.elapsed() >= TICK_INTERVAL {
                        last_tick = Instant::now();
                        for event in scene.step() {
                            if event == SceneEvent::SceneReset {
                                info!("scene reset — restarting");
                            }
                        }
                    }

                    let sets = build_instances(&scene, &visuals);
                    let orbit_seconds = start.elapsed().as_secs_f32();
                    if let Err(err) = renderer.render(
                        orbit_seconds,
                        &sets.round_segments,
                        &sets.square_segments,
                        &sets.joints,
                    ) {
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
