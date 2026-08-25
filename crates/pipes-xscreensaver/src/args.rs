//! Parses the xscreensaver "hack" invocation contract, mirroring
//! `pipes-app::screensaver_args` for the Windows side. Deliberately pure/
//! side-effect-free and OS-independent — the environment is read by the
//! caller and passed in rather than looked up here — so this part is
//! unit-testable on any host, X server or not.
//!
//! **Verified against a live installation** (xscreensaver 6.08 on Linux,
//! replacing what used to be this crate's single biggest unverified
//! assumption). Two facts came out of that, the second of which the
//! original best-guess contract had missed entirely:
//!
//! 1. Hacks do accept `-window-id <id>` and `-root` on the command line —
//!    that part of the guess was right. `xscreensaver-settings` uses
//!    `-window-id` to drive its preview pane.
//! 2. **The `xscreensaver` daemon itself passes no arguments at all.** Its
//!    driver (`xscreensaver-gfx`) hands the hack its window *only* through
//!    the `XSCREENSAVER_WINDOW` environment variable, formatted as
//!    `0x%lX`. Confirmed empirically by running a probe script as a
//!    configured hack under a real daemon: argv was empty and
//!    `XSCREENSAVER_WINDOW=0x60000C` was set. `strings` on both
//!    `xscreensaver-gfx` and the stock hacks agrees.
//!
//! So argv alone is not enough to find the target window: under a real
//! daemon an argv-only parser sees nothing, falls back to the root window,
//! and draws *behind* xscreensaver's own saver window — i.e. a black
//! screen. `resolve_target_window` exists to close that gap, and is what
//! `main` calls; `parse_xscreensaver_args` remains the pure argv half.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetWindow {
    /// Draw directly on the root window (the classic, oldest xscreensaver
    /// hack convention, and this parser's fallback when no window is
    /// specified at all — reasonable for standalone/manual testing).
    Root,
    /// Draw into a specific existing X11 window ID, given by xscreensaver.
    Explicit(u64),
}

pub fn parse_xscreensaver_args<S: AsRef<str>>(args: &[S]) -> TargetWindow {
    let mut iter = args.iter().map(|s| s.as_ref());
    while let Some(arg) = iter.next() {
        match arg {
            "-root" => return TargetWindow::Root,
            "-window-id" => {
                if let Some(id) = iter.next().and_then(parse_window_id) {
                    return TargetWindow::Explicit(id);
                }
                return TargetWindow::Root;
            }
            _ => continue,
        }
    }
    TargetWindow::Root
}

/// The environment variable `xscreensaver`'s driver uses to hand a hack
/// the window it should draw into. This, not argv, is how the real daemon
/// communicates — see this module's header for how that was confirmed.
pub const WINDOW_ENV_VAR: &str = "XSCREENSAVER_WINDOW";

/// Resolves the window to draw into from *both* channels the real
/// xscreensaver contract uses, in precedence order:
///
/// 1. An explicit `-window-id <id>` on the command line — a specific
///    window named directly by whoever launched us
///    (`xscreensaver-settings` drives its preview pane this way), so it
///    outranks everything else.
/// 2. `$XSCREENSAVER_WINDOW` — how the daemon hands over its window.
/// 3. The root window: either `-root` was passed, or nothing was.
///
/// **The environment deliberately outranks `-root`**, which looks
/// backwards until you see how the daemon actually launches a hack: it
/// passes `-root` *and* sets the environment variable, together, on the
/// same invocation. Hack config XML carries a `<command arg="-root"/>`
/// by convention — this project's own
/// `installer/linux/xscreensaver-config/` XML included — so `-root` is
/// present on essentially every real daemon-driven run. Letting it win
/// would mean drawing on the actual root window, behind xscreensaver's
/// saver window, which is exactly the black screen this function exists
/// to prevent. Confirmed by observation, not inference: stock `gears`,
/// launched by a live daemon with `-root` on its command line, still
/// renders into the saver window rather than onto the root — so it, too,
/// prefers the environment variable. `-root` is best read as "no
/// specific window was named", which is why it ranks alongside passing
/// nothing at all.
///
/// Kept pure (the environment is read by the caller and passed in) so it
/// stays unit-testable on any host, exactly like `parse_xscreensaver_args`.
pub fn resolve_target_window<S: AsRef<str>>(args: &[S], env_window: Option<&str>) -> TargetWindow {
    match parse_xscreensaver_args(args) {
        // A specific window named on the command line wins outright.
        explicit @ TargetWindow::Explicit(_) => explicit,
        // Otherwise argv named no particular window — whether it said
        // `-root` or said nothing — so the environment gets its turn.
        TargetWindow::Root => match env_window.and_then(parse_window_id) {
            Some(id) => TargetWindow::Explicit(id),
            // A malformed or empty value is not worth dying over: the
            // root window still draws something, which beats a hack that
            // exits and leaves xscreensaver with a blank screen.
            None => TargetWindow::Root,
        },
    }
}

fn parse_window_id(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_defaults_to_root() {
        assert_eq!(parse_xscreensaver_args::<&str>(&[]), TargetWindow::Root);
    }

    #[test]
    fn root_flag_is_root() {
        assert_eq!(parse_xscreensaver_args(&["-root"]), TargetWindow::Root);
    }

    #[test]
    fn window_id_decimal() {
        assert_eq!(
            parse_xscreensaver_args(&["-window-id", "58720257"]),
            TargetWindow::Explicit(58720257)
        );
    }

    #[test]
    fn window_id_hex() {
        assert_eq!(
            parse_xscreensaver_args(&["-window-id", "0x3800001"]),
            TargetWindow::Explicit(0x3800001)
        );
    }

    // --- resolve_target_window: both channels of the real contract ---

    /// The regression this whole function exists for: the real daemon
    /// passes *no* arguments and only sets the environment variable, so an
    /// argv-only reading finds nothing and silently draws on the root
    /// window — behind xscreensaver's saver window, i.e. a black screen.
    #[test]
    fn daemon_style_invocation_with_empty_argv_uses_the_env_window() {
        let no_args: &[&str] = &[];
        assert_eq!(
            parse_xscreensaver_args(no_args),
            TargetWindow::Root,
            "argv alone still sees nothing — that's the gap being closed"
        );
        assert_eq!(
            resolve_target_window(no_args, Some("0x60000C")),
            TargetWindow::Explicit(0x60000C)
        );
    }

    #[test]
    fn env_window_accepts_the_drivers_uppercase_hex_formatting() {
        // xscreensaver-gfx formats the value with "0x%lX", so the digits
        // arrive upper-case behind a lower-case prefix.
        let no_args: &[&str] = &[];
        assert_eq!(
            resolve_target_window(no_args, Some("0xABCDEF")),
            TargetWindow::Explicit(0xABCDEF)
        );
    }

    #[test]
    fn explicit_window_id_arg_outranks_the_env() {
        assert_eq!(
            resolve_target_window(&["-window-id", "0x2200003"], Some("0x60000C")),
            TargetWindow::Explicit(0x2200003)
        );
    }

    /// The real daemon passes `-root` *and* sets the environment variable
    /// on the same invocation (hack config XML carries
    /// `<command arg="-root"/>` by convention, this project's included),
    /// so `-root` must not shadow the window it was handed — that would
    /// draw behind the saver window. Verified against stock `gears`,
    /// which behaves the same way.
    #[test]
    fn env_window_outranks_a_bare_root_arg() {
        assert_eq!(
            resolve_target_window(&["-root"], Some("0x60000C")),
            TargetWindow::Explicit(0x60000C)
        );
    }

    #[test]
    fn root_arg_still_wins_when_no_env_window_is_set() {
        assert_eq!(resolve_target_window(&["-root"], None), TargetWindow::Root);
    }

    #[test]
    fn malformed_or_empty_env_falls_back_to_root_rather_than_failing() {
        let no_args: &[&str] = &[];
        assert_eq!(resolve_target_window(no_args, Some("")), TargetWindow::Root);
        assert_eq!(
            resolve_target_window(no_args, Some("not-a-window")),
            TargetWindow::Root
        );
        assert_eq!(resolve_target_window(no_args, None), TargetWindow::Root);
    }

    #[test]
    fn a_malformed_window_id_still_defers_to_the_env() {
        // `-window-id -root` is malformed: the id parse fails, so no
        // specific window was successfully named and the environment
        // still gets its turn.
        assert_eq!(
            resolve_target_window(&["-window-id", "-root"], Some("0x60000C")),
            TargetWindow::Explicit(0x60000C)
        );
    }

    #[test]
    fn window_id_missing_or_malformed_falls_back_to_root() {
        assert_eq!(parse_xscreensaver_args(&["-window-id"]), TargetWindow::Root);
        assert_eq!(
            parse_xscreensaver_args(&["-window-id", "not-a-number"]),
            TargetWindow::Root
        );
    }
}
