//! Parses the Windows screensaver command-line contract, so that once
//! renamed to a `.scr` and registered, Windows can drive this binary the
//! same way it drives every other screensaver: fullscreen when the idle
//! timer fires, a config dialog from the Settings UI, and a tiny live
//! preview inside the Settings UI's screensaver dropdown.
//!
//! Deliberately OS-independent and side-effect-free (no Win32 calls here)
//! so it's unit-testable on any host — only `winsaver.rs` (the actual
//! HWND reparenting) is Windows-only.

/// Which of the four ways Windows can invoke a `.scr` this run is.
/// See <https://learn.microsoft.com/en-us/windows/win32/w8cookbook/screen-saver--desktop-> style
/// docs, or any `.scr`'s properties: Windows always calls it with one of
/// `/s`, `/c[:<hwnd>]`, `/p <hwnd>`, or `/a <hwnd>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreensaverMode {
    /// Run fullscreen; exit on any input. The default when there are no
    /// recognized screensaver flags at all (so plain `cargo run` still
    /// behaves like the screensaver, just without a real invoking HWND).
    Show,
    /// Show the settings/config UI. Windows may pass the parent HWND of
    /// its own dialog as `/c:<hwnd>`; we don't need it since our config
    /// app (`pipes-settings`) is a separate top-level window, not a child
    /// control embedded in Windows' own dialog.
    Configure,
    /// Render (small, live, non-fullscreen, doesn't exit on input) inside
    /// the given existing window — the thumbnail in the screensaver
    /// dropdown in Settings.
    Preview(isize),
    /// The legacy "change password" invocation. Not applicable to modern
    /// Windows (password change lives elsewhere in Settings); every real
    /// screensaver we've checked either ignores this or no-ops it, so we
    /// do the same rather than pretending to support it.
    ChangePassword,
}

/// Parses argv (excluding argv[0], the program name) per the contract
/// above. Unrecognized/malformed input falls back to `Show` — a
/// screensaver invoked unexpectedly should still do something reasonable
/// (run fullscreen) rather than silently exit.
pub fn parse_screensaver_args<S: AsRef<str>>(args: &[S]) -> ScreensaverMode {
    let mut iter = args.iter().map(|s| s.as_ref());
    while let Some(arg) = iter.next() {
        let lower = arg.to_ascii_lowercase();
        match lower.as_str() {
            "/s" | "-s" => return ScreensaverMode::Show,
            "/c" | "-c" => return ScreensaverMode::Configure,
            "/a" | "-a" => return ScreensaverMode::ChangePassword,
            "/p" | "-p" => {
                if let Some(hwnd) = iter.next().and_then(|s| s.trim().parse::<isize>().ok()) {
                    return ScreensaverMode::Preview(hwnd);
                }
                return ScreensaverMode::Show;
            }
            _ if lower.starts_with("/c:") || lower.starts_with("-c:") => {
                return ScreensaverMode::Configure
            }
            _ if lower.starts_with("/p:") || lower.starts_with("-p:") => {
                let (_, hwnd_str) = arg.split_at(3);
                return match hwnd_str.trim().parse::<isize>() {
                    Ok(hwnd) => ScreensaverMode::Preview(hwnd),
                    Err(_) => ScreensaverMode::Show,
                };
            }
            _ if lower.starts_with("/a:") || lower.starts_with("-a:") => {
                return ScreensaverMode::ChangePassword
            }
            _ => continue,
        }
    }
    ScreensaverMode::Show
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_defaults_to_show() {
        assert_eq!(parse_screensaver_args::<&str>(&[]), ScreensaverMode::Show);
    }

    #[test]
    fn slash_s_is_show() {
        assert_eq!(parse_screensaver_args(&["/s"]), ScreensaverMode::Show);
        assert_eq!(
            parse_screensaver_args(&["/S"]),
            ScreensaverMode::Show,
            "must be case-insensitive"
        );
        assert_eq!(
            parse_screensaver_args(&["-s"]),
            ScreensaverMode::Show,
            "dash variant for manual testing"
        );
    }

    #[test]
    fn slash_c_is_configure_with_or_without_hwnd() {
        assert_eq!(parse_screensaver_args(&["/c"]), ScreensaverMode::Configure);
        assert_eq!(
            parse_screensaver_args(&["/c:123456"]),
            ScreensaverMode::Configure
        );
    }

    #[test]
    fn slash_p_with_hwnd_is_preview() {
        assert_eq!(
            parse_screensaver_args(&["/p", "123456"]),
            ScreensaverMode::Preview(123456)
        );
        assert_eq!(
            parse_screensaver_args(&["/p:123456"]),
            ScreensaverMode::Preview(123456)
        );
    }

    #[test]
    fn slash_p_missing_or_malformed_hwnd_falls_back_to_show() {
        assert_eq!(parse_screensaver_args(&["/p"]), ScreensaverMode::Show);
        assert_eq!(
            parse_screensaver_args(&["/p", "not-a-number"]),
            ScreensaverMode::Show
        );
    }

    #[test]
    fn slash_a_is_change_password() {
        assert_eq!(
            parse_screensaver_args(&["/a", "123456"]),
            ScreensaverMode::ChangePassword
        );
        assert_eq!(
            parse_screensaver_args(&["/a:123456"]),
            ScreensaverMode::ChangePassword
        );
    }

    #[test]
    fn unrecognized_args_fall_back_to_show() {
        assert_eq!(
            parse_screensaver_args(&["--seed", "5"]),
            ScreensaverMode::Show
        );
    }

    #[test]
    fn recognized_flag_after_unrecognized_ones_still_matches() {
        assert_eq!(
            parse_screensaver_args(&["--seed", "5", "/c"]),
            ScreensaverMode::Configure
        );
    }
}
