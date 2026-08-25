//! "Pipes Settings": a standalone window with a live pipes preview on the
//! left and a settings drawer on the right. Both this app and the actual
//! screensaver (`pipes-app`) read/write the same `AppConfig` file, so
//! changes made here take effect the next time the screensaver runs (and
//! are visible immediately in this window's own preview).

// Debug builds keep the console (so `cargo run`/`tracing` output is visible
// in the terminal); release builds drop it. Without this, Rust defaults to
// the console subsystem on Windows for every binary, so launching this app
// pops up a visible console window that comes to the foreground — and
// closing that console window sends its default control handler a close
// event that kills this whole process, not just the console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pipes_core::{Scene, SceneEvent};
use pipes_render::{build_instances, AppConfig, Renderer};
use tracing::info;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

// winit's cross-platform `with_window_icon` only sets the small (title
// bar) icon on Windows — the taskbar button uses a separate "big" icon
// that has no cross-platform builder method, only this Windows-specific
// extension trait. Without it, the taskbar button shows Windows' generic
// default icon even though the title bar shows the right one.
#[cfg(windows)]
use winit::platform::windows::WindowBuilderExtWindows;

mod benchmark;
mod notify;
mod ui;
mod update;

/// Kicks off the update check on a background thread so it never delays
/// showing the window, and can't hang the app if GitHub is slow/down —
/// `check_for_update` itself already treats any failure as "no update"
/// (see update.rs), so this just adds "don't block the UI thread either."
fn spawn_update_check() -> mpsc::Receiver<Option<update::AvailableUpdate>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
            .expect("CARGO_PKG_VERSION is valid semver");
        let _ = tx.send(update::check_for_update(&current));
    });
    rx
}

/// Downloads and launches the installer on a background thread, then
/// exits this process — an MSI upgrade needs pipes-settings.exe not to be
/// running so its own file can be replaced cleanly. On the success path
/// the process exits and `failed_tx` is moot; on any failure (network,
/// launch, or a checksum mismatch — see `update::verify_checksum`) it
/// reports back so the caller can stop showing "Downloading…" forever
/// with no way to retry.
fn spawn_update_install(available: update::AvailableUpdate, failed_tx: mpsc::Sender<()>) {
    std::thread::spawn(move || match update::download_installer(&available) {
        Ok(path) => match update::launch_installer(&path) {
            Ok(_) => std::process::exit(0),
            Err(err) => {
                tracing::error!(?err, "failed to launch downloaded installer");
                let _ = failed_tx.send(());
            }
        },
        Err(err) => {
            tracing::error!(?err, "failed to download update");
            let _ = failed_tx.send(());
        }
    });
}

fn main() {
    // Held for the whole process lifetime: dropping it stops the file
    // logger's background writer from flushing further lines.
    let _log_guard = pipes_render::diagnostics::init_logging("pipes-settings");
    pipes_render::diagnostics::install_panic_hook("Pipes Settings");

    let mut app_config = AppConfig::load();
    app_config.sanitize();
    info!(config_path = ?AppConfig::config_path(), "loaded AppConfig (or defaults if missing)");

    let event_loop = EventLoop::new().expect("failed to create event loop");
    // `mut` is only needed inside the cfg(windows) block below - on other
    // platforms nothing ever reassigns builder, so clippy flags it as
    // unused there.
    #[allow(unused_mut)]
    let mut builder = WindowBuilder::new()
        .with_title("Pipes Settings")
        .with_window_icon(pipes_render::app_icon::window_icon())
        .with_inner_size(LogicalSize::new(1200.0, 760.0));
    #[cfg(windows)]
    {
        builder = builder.with_taskbar_icon(pipes_render::app_icon::window_icon());
    }
    let window = Arc::new(builder.build(&event_loop).expect("failed to create window"));

    let bounds = (
        app_config.sim.bounds.width,
        app_config.sim.bounds.height,
        app_config.sim.bounds.depth,
    );
    let window_size = window.inner_size();
    let mut renderer = pollster::block_on(Renderer::new(
        window.clone(),
        (window_size.width, window_size.height),
        bounds,
    ));
    let mut scene = Scene::new(app_config.sim.clone(), rand_seed());

    let mut egui_ctx = egui::Context::default();
    let mut egui_state = egui_winit::State::new(
        egui_ctx.clone(),
        egui::ViewportId::ROOT,
        &window,
        None,
        None,
    );
    let mut egui_renderer =
        egui_wgpu::Renderer::new(renderer.device(), renderer.surface_format(), None, 1);

    let start = Instant::now();
    let mut last_tick = Instant::now();
    let update_check_rx = spawn_update_check();
    let mut available_update: Option<update::AvailableUpdate> = None;
    let mut update_dismissed = false;
    let mut update_downloading = false;
    // The toast's Activated callback fires on a WinRT callback thread, not
    // this event loop thread, so it can only ask to be focused via a
    // channel like this - it can't touch `window` directly.
    let (focus_tx, focus_rx) = mpsc::channel::<()>();
    let mut update_toast_shown = false;
    // Must outlive the call that creates it: a `ToastNotification`'s
    // `Activated` (click) event only has something to fire on for as
    // long as the object itself is alive, so this holds it for the rest
    // of the program's life instead of letting it drop the instant
    // `notify_update_available` returns — the actual bug behind "clicking
    // the toast doesn't open Settings" (found by a real user on a real
    // install, since nothing here could be verified any other way).
    let mut _update_toast: Option<notify::ToastHandle> = None;
    // Same idea: spawn_update_install runs on its own thread and can't
    // touch `update_downloading` directly.
    let (update_failed_tx, update_failed_rx) = mpsc::channel::<()>();
    // The in-app benchmark (see benchmark.rs) drives itself forward one
    // real rendered frame at a time from right here, rather than owning
    // any window/event-loop code of its own.
    let mut benchmark_run: Option<benchmark::Run> = None;
    let mut benchmark_report: Option<benchmark::Report> = None;

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => {
                let response = egui_state.on_window_event(&window, &event);
                if response.repaint {
                    window.request_redraw();
                }
                if response.consumed {
                    return;
                }
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(size) => renderer.resize(size.width, size.height),
                    WindowEvent::RedrawRequested => {
                        if let Ok(result) = update_check_rx.try_recv() {
                            if let Some(update) = &result {
                                info!(version = %update.version, "update available");
                            }
                            available_update = result;
                        }
                        if !update_toast_shown {
                            if let Some(update) = &available_update {
                                update_toast_shown = true;
                                let focus_tx = focus_tx.clone();
                                _update_toast = notify::notify_update_available(
                                    &update.version.to_string(),
                                    move || {
                                        let _ = focus_tx.send(());
                                    },
                                );
                            }
                        }
                        if focus_rx.try_recv().is_ok() {
                            window.focus_window();
                        }

                        let tick_interval =
                            Duration::from_millis(app_config.tick_interval_ms as u64);
                        if last_tick.elapsed() >= tick_interval {
                            last_tick = Instant::now();
                            for event in scene.step() {
                                if event == SceneEvent::SceneReset {
                                    info!("preview scene reset — restarting");
                                }
                            }
                        }

                        if update_failed_rx.try_recv().is_ok() {
                            update_downloading = false;
                        }

                        let update_banner = if update_dismissed {
                            None
                        } else {
                            available_update.as_ref()
                        };
                        let benchmark_progress = benchmark_run.as_ref().map(|run| {
                            let (stage_number, stage_count, frames_done, frames_needed) =
                                run.progress();
                            ui::BenchmarkProgress {
                                stage_name: run.current_stage_name().to_string(),
                                stage_number,
                                stage_count,
                                frames_done,
                                frames_needed,
                            }
                        });
                        let raw_input = egui_state.take_egui_input(&window);
                        let mut outcome = ui::Outcome::default();
                        let full_output = egui_ctx.run(raw_input, |ctx| {
                            outcome = ui::draw(
                                ctx,
                                &mut app_config,
                                update_banner,
                                update_downloading,
                                benchmark_progress.as_ref(),
                                benchmark_report.as_ref(),
                            );
                        });
                        egui_state.handle_platform_output(&window, full_output.platform_output);

                        if outcome.update_dismissed {
                            update_dismissed = true;
                        }
                        if outcome.update_now_clicked {
                            if let Some(update) = available_update.clone() {
                                update_downloading = true;
                                spawn_update_install(update, update_failed_tx.clone());
                            }
                        }
                        if outcome.run_benchmark_clicked && benchmark_run.is_none() {
                            let run = benchmark::Run::new();
                            scene = populated_scene(
                                run.current_config().clone(),
                                rand_seed(),
                                BENCHMARK_WARMUP_TICKS,
                            );
                            benchmark_run = Some(run);
                            benchmark_report = None;
                        }
                        if outcome.reset_to_defaults {
                            app_config = AppConfig::default();
                        }
                        // Guarded on `benchmark_run.is_none()`: a benchmark
                        // owns `scene` for its own stage configs while
                        // running (see above and below), and a sim-changing
                        // edit to some other slider mid-run would otherwise
                        // silently overwrite it out from under the run.
                        if (outcome.sim_changed || outcome.reset_to_defaults)
                            && benchmark_run.is_none()
                        {
                            scene = Scene::new(app_config.sim.clone(), rand_seed());
                        }
                        if outcome.changed() {
                            app_config.sanitize();
                            if let Err(err) = app_config.save() {
                                tracing::warn!(?err, "failed to save config");
                            }
                        }

                        let size = window.inner_size();
                        let ppp = full_output.pixels_per_point;
                        let preview_px = (
                            (outcome.preview_rect.min.x * ppp) as u32,
                            (outcome.preview_rect.min.y * ppp) as u32,
                            (outcome.preview_rect.width() * ppp) as u32,
                            (outcome.preview_rect.height() * ppp) as u32,
                        );

                        let clipped_primitives =
                            egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
                        let screen_descriptor = egui_wgpu::ScreenDescriptor {
                            size_in_pixels: [size.width, size.height],
                            pixels_per_point: full_output.pixels_per_point,
                        };

                        let sets = build_instances(&scene, &app_config.visuals);
                        let orbit_seconds = start.elapsed().as_secs_f32();
                        // Checked up front rather than left to the Err
                        // match below: `recover_if_needed` attempts a real
                        // hot recovery (new Device/Queue/Surface) once per
                        // loss episode before giving up and just freezing
                        // on the last good frame — see its doc comment.
                        let was_lost = renderer.is_device_lost();
                        let can_render = renderer.recover_if_needed();
                        if was_lost && can_render {
                            // A successful recovery gives `renderer` a
                            // brand new wgpu::Device — `egui_renderer`
                            // above was built against the old (now
                            // destroyed) one, and kept using it silently
                            // caused every buffer write into it to fail
                            // validation ("missing the COPY_DST usage
                            // flag") on the very next frame, which looked
                            // like recovery itself was broken (an
                            // immediate second "device lost") until this
                            // was traced back to egui_renderer specifically.
                            egui_renderer = egui_wgpu::Renderer::new(
                                renderer.device(),
                                renderer.surface_format(),
                                None,
                                1,
                            );
                            // The fresh renderer's texture map starts empty,
                            // but `egui_ctx` still thinks the font atlas was
                            // already delivered (it was, to the now-gone
                            // renderer) and has no way to know it needs
                            // resending — confirmed dead ends: `set_fonts`
                            // is a no-op when the definitions are unchanged
                            // (checked by value, not by whether a renderer
                            // actually has the atlas), and `TextureManager`
                            // doesn't expose the pixel data needed to
                            // reconstruct a fresh delta by hand. A whole new
                            // `Context`/`State` sidesteps the question
                            // entirely: on its first `run()` it allocates
                            // the atlas from scratch, which unconditionally
                            // enqueues a full delta — no cache to be stale.
                            // Costs only this rare recovery moment's
                            // transient UI state (which section was
                            // expanded, etc.), not app_config. Without this,
                            // egui logged "Missing texture: Managed(0)"
                            // forever and drew no text/icons at all after a
                            // recovery — found by actually triggering one
                            // and watching it happen, not by reasoning
                            // about egui's internals up front.
                            egui_ctx = egui::Context::default();
                            egui_state = egui_winit::State::new(
                                egui_ctx.clone(),
                                egui::ViewportId::ROOT,
                                &window,
                                None,
                                None,
                            );
                            info!(
                                "rebuilt the egui renderer/context against the recovered GPU device"
                            );
                        }
                        let render_start = Instant::now();
                        let render_result = if !can_render {
                            Ok(())
                        } else {
                            renderer.render_with(
                                orbit_seconds,
                                &app_config.camera,
                                Some(preview_px),
                                &sets,
                                |device, queue, encoder, view| {
                                    for (id, delta) in &full_output.textures_delta.set {
                                        egui_renderer.update_texture(device, queue, *id, delta);
                                    }
                                    egui_renderer.update_buffers(
                                        device,
                                        queue,
                                        encoder,
                                        &clipped_primitives,
                                        &screen_descriptor,
                                    );
                                    {
                                        let mut pass = encoder.begin_render_pass(
                                            &wgpu::RenderPassDescriptor {
                                                label: Some("egui pass"),
                                                color_attachments: &[Some(
                                                    wgpu::RenderPassColorAttachment {
                                                        view,
                                                        resolve_target: None,
                                                        ops: wgpu::Operations {
                                                            load: wgpu::LoadOp::Load,
                                                            store: wgpu::StoreOp::Store,
                                                        },
                                                    },
                                                )],
                                                depth_stencil_attachment: None,
                                                occlusion_query_set: None,
                                                timestamp_writes: None,
                                            },
                                        );
                                        egui_renderer.render(
                                            &mut pass,
                                            &clipped_primitives,
                                            &screen_descriptor,
                                        );
                                    }
                                    for id in &full_output.textures_delta.free {
                                        egui_renderer.free_texture(id);
                                    }
                                },
                            )
                        };
                        let render_ms = render_start.elapsed().as_secs_f32() * 1000.0;
                        if let Err(err) = render_result {
                            tracing::warn!(?err, "render error");
                        }

                        // Only counts real, actually-rendered frames — a
                        // frame skipped because the device was lost isn't
                        // representative and would otherwise pull the
                        // stage's average toward zero.
                        if can_render {
                            let stage_before = benchmark_run.as_ref().map(|r| r.progress().0);
                            let finished = benchmark_run
                                .as_mut()
                                .map(|r| r.record_frame(render_ms))
                                .unwrap_or(false);
                            if finished {
                                let run = benchmark_run.take().expect("finished implies Some");
                                let report = run.into_report(
                                    env!("CARGO_PKG_VERSION").to_string(),
                                    renderer.adapter_name(),
                                );
                                info!(stages = report.stages.len(), "benchmark finished");
                                benchmark_report = Some(report);
                                scene = Scene::new(app_config.sim.clone(), rand_seed());
                            } else if let Some(run) = &benchmark_run {
                                if Some(run.progress().0) != stage_before {
                                    scene = populated_scene(
                                        run.current_config().clone(),
                                        rand_seed(),
                                        BENCHMARK_WARMUP_TICKS,
                                    );
                                }
                            }
                        }

                        window.request_redraw();
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .expect("event loop error");
}

/// Enough ticks that even the "Heavy" benchmark stage (a 64³ grid) has a
/// real chance to fill in before measurement starts — matches the
/// `large_scene` case in `cargo bench -p pipes-render`'s own warmup.
const BENCHMARK_WARMUP_TICKS: u32 = 400;

/// Builds a `Scene` and immediately steps it forward `warmup_ticks` times
/// before handing it back — used for benchmark stages, where a fixed
/// number of rendered *frames* is not the same thing as a fixed number of
/// simulation *ticks* (gated by `tick_interval_ms`, decoupled from frame
/// rate). Without this, a fast GPU renders a "Heavy" stage's still-nearly-
/// empty scene just as quickly as an actually-populated one and the
/// benchmark ends up measuring almost nothing — confirmed by actually
/// running it and seeing "Heavy" report a *higher* FPS than "Light", the
/// opposite of what a real stress test should show. `Scene::step` is cheap
/// (~µs, see `cargo bench -p pipes-core`), so doing this synchronously
/// when a stage starts doesn't visibly block anything.
fn populated_scene(config: pipes_core::SimConfig, seed: u64, warmup_ticks: u32) -> Scene {
    let mut scene = Scene::new(config, seed);
    for _ in 0..warmup_ticks {
        scene.step();
    }
    scene
}

fn rand_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
}
