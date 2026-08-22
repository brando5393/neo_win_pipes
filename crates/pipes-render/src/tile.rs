//! Off-axis ("asymmetric frustum") projection math for `MonitorMode::Span`.
//!
//! The trick behind any tiled-display / multi-projector setup (this is the
//! same principle physical planetarium/simulator rigs use, generalized by
//! Kooima's "Generalized Perspective Projection"): give every tile the
//! *same* view matrix (same eye, same look-at target), but a projection
//! matrix that only covers that tile's own slice of one shared, wider
//! frustum. Render every tile with its own slice and — placed edge to
//! edge, as separate monitors physically are — they reconstruct one
//! continuous wide view with no visible seam, entirely without needing a
//! single surface spanning multiple GPUs/windows.
//!
//! Pure math, deliberately: no `wgpu`/window types anywhere in this file,
//! so every case here is a plain unit test, no GPU or display required —
//! see `docs/DEVELOPMENT.md#testing-philosophy`.

use glam::{Mat4, Vec4};

/// Computes this tile's near-plane frustum bounds (`left, right, bottom,
/// top`) as its proportional slice of the *full* virtual canvas's frustum.
///
/// `fov_y_radians`/`near` describe the full canvas's frustum as if it were
/// one single window sized `canvas_wh` — not this tile's own aspect ratio,
/// which is what keeps the horizontal/vertical field of view consistent
/// across every monitor instead of distorting edge tiles. `tile_rect` is
/// `(x, y, width, height)` of this monitor's slice within that canvas, in
/// the same pixel space as `canvas_wh` (winit's `MonitorHandle::position()`/
/// `size()` — Y grows downward, the window-system convention, which is why
/// the vertical mapping below is flipped relative to the frustum's
/// upward-growing Y).
fn tile_frustum(
    fov_y_radians: f32,
    near: f32,
    canvas_wh: (f32, f32),
    tile_rect: (f32, f32, f32, f32),
) -> (f32, f32, f32, f32) {
    let (canvas_w, canvas_h) = canvas_wh;
    let (tx, ty, tw, th) = tile_rect;

    let full_top = near * (fov_y_radians * 0.5).tan();
    let full_bottom = -full_top;
    let full_aspect = canvas_w / canvas_h.max(1.0);
    let full_right = full_top * full_aspect;
    let full_left = -full_right;
    let full_width = full_right - full_left;
    let full_height = full_top - full_bottom;

    let left = full_left + full_width * (tx / canvas_w);
    let right = full_left + full_width * ((tx + tw) / canvas_w);
    // Flipped: canvas Y grows downward, frustum Y grows upward.
    let top = full_top - full_height * (ty / canvas_h);
    let bottom = full_top - full_height * ((ty + th) / canvas_h);

    (left, right, bottom, top)
}

/// Right-handed off-center (asymmetric) perspective projection with a 0..1
/// depth range — the same convention `glam::Mat4::perspective_rh` uses,
/// generalized to independent left/right/bottom/top bounds instead of a
/// single symmetric FOV. `glam` has no built-in off-center variant; this is
/// the standard formula (e.g. Real-Time Rendering, 4th ed., §4.6.2),
/// verified below to reduce to `perspective_rh` exactly when the bounds are
/// symmetric.
fn perspective_off_center_rh(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> Mat4 {
    let x = (2.0 * near) / (right - left);
    let y = (2.0 * near) / (top - bottom);
    let a = (right + left) / (right - left);
    let b = (top + bottom) / (top - bottom);
    let r = far / (near - far);
    Mat4::from_cols(
        Vec4::new(x, 0.0, 0.0, 0.0),
        Vec4::new(0.0, y, 0.0, 0.0),
        Vec4::new(a, b, r, -1.0),
        Vec4::new(0.0, 0.0, r * near, 0.0),
    )
}

/// Public entry point: the off-axis projection matrix for one monitor's
/// tile of a `MonitorMode::Span` virtual canvas. See the module doc for
/// the overall technique and `Renderer::frustum_params` for where
/// `fov_y_radians`/`near`/`far` should come from (the same values a
/// single, non-spanned window would use, so spanning doesn't change the
/// field of view — only how it's sliced across displays).
pub fn tile_projection(
    fov_y_radians: f32,
    near: f32,
    far: f32,
    canvas_wh: (f32, f32),
    tile_rect: (f32, f32, f32, f32),
) -> Mat4 {
    let (left, right, bottom, top) = tile_frustum(fov_y_radians, near, canvas_wh, tile_rect);
    perspective_off_center_rh(left, right, bottom, top, near, far)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_mat4_close(a: Mat4, b: Mat4) {
        let (a, b) = (a.to_cols_array(), b.to_cols_array());
        for (x, y) in a.iter().zip(b.iter()) {
            assert!((x - y).abs() < 1e-4, "matrices differ: {a:?} vs {b:?}");
        }
    }

    #[test]
    fn a_tile_covering_the_whole_canvas_matches_the_ordinary_symmetric_projection() {
        // The degenerate "one monitor, Span mode selected" case must be
        // numerically identical to today's single-window rendering — this
        // is the strongest guarantee that Span mode never regresses the
        // single-display case, verifiable without a second monitor.
        let fov = 45f32.to_radians();
        let (near, far) = (0.5, 200.0);
        let canvas = (1920.0, 1080.0);
        let proj = tile_projection(fov, near, far, canvas, (0.0, 0.0, canvas.0, canvas.1));
        let expected = Mat4::perspective_rh(fov, canvas.0 / canvas.1, near, far);
        assert_mat4_close(proj, expected);
    }

    #[test]
    fn adjacent_tiles_share_a_seamless_boundary() {
        // Two 1280x1080 monitors side by side. The left tile's right bound
        // and the right tile's left bound describe the exact same
        // near-plane point — the geometric definition of "no seam".
        let fov = 60f32.to_radians();
        let near = 0.5;
        let canvas = (2560.0, 1080.0);
        let (_, left_right, _, _) = tile_frustum(fov, near, canvas, (0.0, 0.0, 1280.0, 1080.0));
        let (right_left, _, _, _) = tile_frustum(fov, near, canvas, (1280.0, 0.0, 1280.0, 1080.0));
        assert!((left_right - right_left).abs() < 1e-5);
    }

    #[test]
    fn stacked_tiles_share_a_seamless_boundary_too() {
        // Same check, vertically: a monitor arrangement with one display
        // above another.
        let fov = 60f32.to_radians();
        let near = 0.5;
        let canvas = (1920.0, 2160.0);
        let (_, _, top_bottom, _) = tile_frustum(fov, near, canvas, (0.0, 0.0, 1920.0, 1080.0));
        let (_, _, _, bottom_top) = tile_frustum(fov, near, canvas, (0.0, 1080.0, 1920.0, 1080.0));
        assert!((top_bottom - bottom_top).abs() < 1e-5);
    }

    #[test]
    fn a_tile_spanning_the_full_canvas_height_touches_both_vertical_extremes() {
        let fov = 45f32.to_radians();
        let near = 0.5;
        let canvas = (1000.0, 500.0);
        let (_, _, bottom, top) = tile_frustum(fov, near, canvas, (0.0, 0.0, 1000.0, 500.0));
        let full_top = near * (fov * 0.5).tan();
        assert!((top - full_top).abs() < 1e-5);
        assert!((bottom + full_top).abs() < 1e-5);
    }

    #[test]
    fn off_center_reduces_to_the_symmetric_case_when_bounds_are_symmetric() {
        let (near, far) = (0.5, 100.0);
        let (right, top) = (0.3, 0.2);
        let ours = perspective_off_center_rh(-right, right, -top, top, near, far);
        let fov_y = 2.0 * (top / near).atan();
        let aspect = right / top;
        let expected = Mat4::perspective_rh(fov_y, aspect, near, far);
        assert_mat4_close(ours, expected);
    }
}
