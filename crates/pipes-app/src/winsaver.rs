//! Windows-only: embeds our winit window into an existing HWND, for the
//! live thumbnail Windows shows in Settings → Screen saver's dropdown
//! (invoked as `/p <hwnd>` — see `screensaver_args.rs`). Everything here
//! is a thin, deliberately narrow wrapper around three Win32 calls; the
//! actual "which mode are we in" decision lives in `main.rs` so this file
//! stays a pure mechanism, not a policy.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, SetParent, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOZORDER, WS_CHILD, WS_VISIBLE,
};

/// Reparents `window` as a child of `preview_hwnd` and resizes it to fill
/// that window's client area. Must be called once, after the winit window
/// exists. There is no live resize handling yet — the Settings dropdown's
/// preview thumbnail is a fixed size for the lifetime of the dialog, so
/// this is called once at startup and left alone (see
/// `docs/ROADMAP.md` for the known limitation if that assumption ever
/// turns out wrong on some Windows version).
pub fn embed_in_preview(window: &winit::window::Window, preview_hwnd: isize) {
    let Ok(handle) = window.window_handle() else {
        tracing::warn!("could not get a window handle to embed in the preview");
        return;
    };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        tracing::warn!("window handle was not a Win32 handle; cannot embed in preview");
        return;
    };

    let child_hwnd = win32.hwnd.get() as HWND;
    let parent_hwnd = preview_hwnd as HWND;

    // SAFETY: `child_hwnd` comes from our own just-created, still-live
    // winit window; `parent_hwnd` is the HWND Windows itself passed us via
    // `/p <hwnd>`, which is guaranteed valid for the lifetime of this
    // process per the screensaver preview contract.
    unsafe {
        SetParent(child_hwnd, parent_hwnd);
        SetWindowLongPtrW(child_hwnd, GWL_STYLE, (WS_CHILD | WS_VISIBLE) as isize);
        fit_to_parent(child_hwnd, parent_hwnd);
    }
}

/// # Safety
/// Both handles must be valid, live windows.
unsafe fn fit_to_parent(child_hwnd: HWND, parent_hwnd: HWND) {
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(parent_hwnd, &mut rect);
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    SetWindowPos(
        child_hwnd,
        std::ptr::null_mut(),
        0,
        0,
        width,
        height,
        SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
    );
}
