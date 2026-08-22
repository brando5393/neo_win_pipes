//! Shared startup diagnostics for both `pipes-app` and `pipes-settings`:
//! persistent file logging (so there's still something to inspect after
//! the fact now that release builds have no console — see
//! `#![windows_subsystem = "windows"]` in both crates' `main.rs`) and a
//! panic hook that shows a human-readable native error dialog instead of
//! the process just silently vanishing. See `docs/LOGGING.md`.

use std::path::PathBuf;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

/// Per-user log directory, e.g. `%APPDATA%\neo-win-pipes\neo_win_pipes\data\logs`
/// on Windows. `None` if the OS's home directory can't be resolved (rare) —
/// callers should still log to stdout in that case.
pub fn log_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "neo-win-pipes", "neo_win_pipes")
        .map(|dirs| dirs.data_dir().join("logs"))
}

/// Sets up logging to stdout (as before — visible in `cargo run`'s console)
/// and, when the log directory can be created, a daily-rotating plain-text
/// file under [`log_dir`]. Returns a guard that must be kept alive for the
/// life of `main()` — dropping it stops the file writer from flushing.
///
/// Log file retention/cleanup isn't implemented yet (files accumulate one
/// per day indefinitely) — acceptable for now given how small these logs
/// are, but a real gap if this ever runs unattended for months.
pub fn init_logging(app_name: &str) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let env_filter =
        || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .compact();

    match log_dir() {
        Some(dir) if std::fs::create_dir_all(&dir).is_ok() => {
            let file_appender = tracing_appender::rolling::daily(&dir, format!("{app_name}.log"));
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            let file_layer = tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_ansi(false)
                .with_writer(non_blocking);

            tracing_subscriber::registry()
                .with(env_filter())
                .with(stdout_layer)
                .with(file_layer)
                .init();
            Some(guard)
        }
        _ => {
            tracing_subscriber::registry()
                .with(env_filter())
                .with(stdout_layer)
                .init();
            None
        }
    }
}

/// Installs a panic hook that (1) still runs the default hook (so a
/// debug-build console keeps showing the usual backtrace), (2) logs the
/// panic through `tracing` (so it lands in the file log too), and (3)
/// shows a native "something went wrong" dialog so a real user isn't just
/// left staring at a window that silently vanished.
pub fn install_panic_hook(app_name: &str) {
    let app_name = app_name.to_string();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        let message = panic_message(info);
        tracing::error!(panic_message = %message, "unhandled panic — showing error dialog");
        show_fatal_error_dialog(&app_name, &message);
    }));
}

fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let text = if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "(no panic message)".to_string()
    };
    match info.location() {
        Some(loc) => format!(
            "{text}\n\nat {}:{}:{}",
            loc.file(),
            loc.line(),
            loc.column()
        ),
        None => text,
    }
}

#[cfg(windows)]
fn show_fatal_error_dialog(app_name: &str, message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let title = to_wide(&format!("{app_name} - unexpected error"));
    let body = to_wide(&format!(
        "{app_name} hit an unexpected error and needs to close.\n\n\
         {message}\n\n\
         Details were written to the log file (see docs/LOGGING.md)."
    ));
    // SAFETY: both buffers are NUL-terminated UTF-16 owned by this
    // function's stack and outlive the call; hwnd=null targets no
    // particular window, which is valid for MessageBoxW.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            body.as_ptr(),
            title.as_ptr(),
            MB_ICONERROR | MB_OK,
        );
    }
}

#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(not(windows))]
fn show_fatal_error_dialog(_app_name: &str, _message: &str) {
    // No native dialog on other platforms yet — Phase 4 is Windows-only
    // (see CLAUDE.md). The panic is still logged to stderr (default hook)
    // and to the file log above.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_dir_contains_app_identity_and_logs_suffix() {
        let dir = log_dir().expect("should resolve on any supported OS");
        let dir_str = dir.to_string_lossy();
        assert!(dir_str.contains("neo_win_pipes"));
        assert!(dir_str.ends_with("logs"));
    }

    #[test]
    fn panic_message_includes_string_payload_and_location() {
        let (tx, rx) = std::sync::mpsc::channel();
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            tx.send(panic_message(info)).ok();
        }));
        let result = std::panic::catch_unwind(|| {
            panic!("boom");
        });
        std::panic::set_hook(default_hook);
        assert!(result.is_err());
        let message = rx.recv().expect("hook should have sent a message");
        assert!(message.contains("boom"));
        assert!(message.contains("diagnostics.rs"));
    }
}
