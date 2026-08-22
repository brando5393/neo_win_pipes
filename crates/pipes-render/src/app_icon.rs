//! The window icon (title bar, taskbar button, Alt-Tab) shared by
//! `pipes-app` and `pipes-settings`.
//!
//! This is a *separate* concern from `build.rs` + `winres`: embedding the
//! `.ico` as a PE resource makes Explorer, Start Menu shortcuts, and
//! Programs-and-Features show the right icon for the `.exe` file itself,
//! but it does **not** automatically become a running window's title
//! bar/taskbar icon — Windows only falls back to the exe's resource icon
//! for windows that never set one themselves, and winit registers its own
//! window class without setting one, so without this a running window
//! shows Windows' generic default icon regardless of what's embedded in
//! the exe. `winit::window::Icon` needs raw RGBA pixels, not a `.ico`
//! file, hence the separate raw dump below rather than reusing `icon.ico`
//! directly.

const ICON_SIZE: u32 = 128;
const ICON_RGBA: &[u8] = include_bytes!("../../../assets/icon/icon-128-rgba.raw");

/// The shared app icon, ready to pass to `WindowBuilder::with_window_icon`.
/// `None` only if the embedded pixel data is malformed, which would be a
/// build-time asset bug, not something that can happen at runtime.
pub fn window_icon() -> Option<winit::window::Icon> {
    match winit::window::Icon::from_rgba(ICON_RGBA.to_vec(), ICON_SIZE, ICON_SIZE) {
        Ok(icon) => Some(icon),
        Err(err) => {
            tracing::warn!(?err, "failed to build window icon from embedded pixel data");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_pixel_data_matches_declared_dimensions() {
        assert_eq!(ICON_RGBA.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
    }

    #[test]
    fn window_icon_builds_successfully() {
        assert!(window_icon().is_some());
    }
}
