//! Linux xscreensaver "hack" wrapper — **not yet functional as a
//! screensaver**. What's real: the argument parsing (`args.rs`, tested).
//! What's missing, honestly: actually opening an X11 connection,
//! resolving the target window (root or the given `-window-id`), building
//! a `wgpu` surface from it via a raw Xlib/XCB window handle, and running
//! the same `pipes-render`/`pipes-core` loop `pipes-app` does. That needs
//! `x11rb` or `x11-dl` plus careful `raw-window-handle` construction —
//! deliberately not written yet, because it can't be compiled or run on
//! this project's current (Windows) development machine, and shipping
//! unverified FFI glue as if it were tested would be worse than being
//! honest that it isn't built yet. See `docs/ROADMAP.md` for the plan.

mod args;

use args::{parse_xscreensaver_args, TargetWindow};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .compact()
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let target = parse_xscreensaver_args(&args);

    match target {
        TargetWindow::Root => tracing::info!("resolved target: root window"),
        TargetWindow::Explicit(id) => {
            tracing::info!(window_id = id, "resolved target: explicit window id")
        }
    }

    tracing::error!(
        "pipes-xscreensaver does not render yet — argument parsing is implemented and tested, \
         but X11 window embedding + the pipes-render pipeline aren't wired up. See docs/ROADMAP.md."
    );
    std::process::exit(1);
}
