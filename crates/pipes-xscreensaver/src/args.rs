//! Parses the (planned) xscreensaver "hack" invocation contract, mirroring
//! `pipes-app::screensaver_args` for the Windows side. Deliberately pure/
//! side-effect-free and OS-independent, so — unlike the actual X11
//! rendering this crate doesn't implement yet — this part is genuinely
//! unit-tested, on any host.
//!
//! **Caveat**: xscreensaver's real command-line contract needs to be
//! double-checked against a live xscreensaver installation/its `screenhack.c`
//! driver source once there's Linux access to verify against — this is
//! this crate's single biggest unverified assumption. What's implemented
//! here (`-window-id <id>`, `-root`) matches xscreensaver hack conventions
//! as documented in third-party ports we could find without a Linux
//! machine to confirm against; treat it as a documented best guess, not a
//! confirmed fact, until verified.

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

    #[test]
    fn window_id_missing_or_malformed_falls_back_to_root() {
        assert_eq!(parse_xscreensaver_args(&["-window-id"]), TargetWindow::Root);
        assert_eq!(
            parse_xscreensaver_args(&["-window-id", "not-a-number"]),
            TargetWindow::Root
        );
    }
}
