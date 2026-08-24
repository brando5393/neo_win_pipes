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

    let egui_ctx = egui::Context::default();
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
                        let raw_input = egui_state.take_egui_input(&window);
                        let mut outcome = ui::Outcome::default();
                        let full_output = egui_ctx.run(raw_input, |ctx| {
                            outcome =
                                ui::draw(ctx, &mut app_config, update_banner, update_downloading);
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
                        if outcome.reset_to_defaults {
                            app_config = AppConfig::default();
                        }
                        if outcome.sim_changed || outcome.reset_to_defaults {
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
                        // match below: `set_device_lost_callback` already
                        // logged this once when it actually happened, so
                        // there's nothing more to do here every frame
                        // except not call into the dead device again.
                        let render_result = if renderer.is_device_lost() {
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
                        if let Err(err) = render_result {
                            tracing::warn!(?err, "render error");
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

fn rand_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
}
