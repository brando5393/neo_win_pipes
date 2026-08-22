//! Real X11 window/display resolution and a raw-window-handle bridge for
//! `wgpu`, so this crate can reuse the exact same `pipes_render::Renderer`
//! that `pipes-app`/`pipes-settings` use, targeting a window it does not
//! itself create — `xscreensaver`'s driver hands us one (or we use the
//! root window for standalone/manual testing).
//!
//! **Verification caveat**: written against `x11-dl`'s and Xlib's
//! documented API, and confirmed to type-check and pass clippy against
//! the `x86_64-unknown-linux-gnu` target (see `docs/ROADMAP.md`), but
//! this project has no Linux machine with a real X server to run it
//! against. Whether a GPU surface actually comes up correctly inside a
//! real `xscreensaver`-managed window is unverified until someone with
//! Linux access confirms it.

use std::ffi::c_void;
use std::os::raw::{c_int, c_ulong};
use std::ptr::NonNull;

use pipes_render::rwh::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle, XlibDisplayHandle, XlibWindowHandle,
};
use x11_dl::xlib::{self, Xlib};

use crate::args::TargetWindow;

/// Owns the X11 connection (via `XOpenDisplay`) and the specific window
/// we've been told to draw into.
pub struct X11Target {
    xlib: Xlib,
    display: *mut xlib::Display,
    window: c_ulong,
    screen: c_int,
}

// SAFETY: this process only ever touches the Xlib connection from the
// single render-loop thread in `main()` — there's no actual concurrent
// access, just a raw pointer that Rust can't infer Send/Sync for on its
// own.
unsafe impl Send for X11Target {}
unsafe impl Sync for X11Target {}

impl X11Target {
    /// Opens the default X display (`$DISPLAY`) and resolves `target`
    /// into a concrete window: the root window, or the specific window ID
    /// `xscreensaver` gave us via `-window-id`.
    pub fn open(target: TargetWindow) -> Self {
        let xlib = Xlib::open().expect(
            "libX11 not found — this is a Linux-only xscreensaver hack, not a general-purpose app; \
             it needs a real X11 environment to run",
        );
        let display = unsafe { (xlib.XOpenDisplay)(std::ptr::null()) };
        assert!(
            !display.is_null(),
            "XOpenDisplay failed — is $DISPLAY set, and is an X server actually running?"
        );
        let screen = unsafe { (xlib.XDefaultScreen)(display) };
        let window = match target {
            TargetWindow::Root => unsafe { (xlib.XDefaultRootWindow)(display) },
            TargetWindow::Explicit(id) => id as c_ulong,
        };
        // StructureNotifyMask is what delivers ConfigureNotify (resize) -
        // a hack is expected to select for this itself, not assume its
        // window never changes size.
        unsafe {
            (xlib.XSelectInput)(display, window, xlib::StructureNotifyMask);
        }
        Self {
            xlib,
            display,
            window,
            screen,
        }
    }

    /// Current size of the target window, queried fresh (not cached) so
    /// it's correct even before the first resize event arrives.
    pub fn size(&self) -> (u32, u32) {
        let mut attrs: xlib::XWindowAttributes = unsafe { std::mem::zeroed() };
        let ok = unsafe { (self.xlib.XGetWindowAttributes)(self.display, self.window, &mut attrs) };
        if ok == 0 {
            tracing::warn!("XGetWindowAttributes failed, defaulting to 800x600");
            return (800, 600);
        }
        (attrs.width.max(1) as u32, attrs.height.max(1) as u32)
    }

    /// Drains pending X events, returning the newest resize (if any) -
    /// non-blocking, so it's safe to call once per frame in the render
    /// loop without stalling it waiting for events that may never come.
    pub fn poll_resize(&self) -> Option<(u32, u32)> {
        let mut latest = None;
        unsafe {
            while (self.xlib.XPending)(self.display) > 0 {
                let mut event: xlib::XEvent = std::mem::zeroed();
                (self.xlib.XNextEvent)(self.display, &mut event);
                if event.get_type() == xlib::ConfigureNotify {
                    let configure = event.configure;
                    latest = Some((
                        configure.width.max(1) as u32,
                        configure.height.max(1) as u32,
                    ));
                }
            }
        }
        latest
    }
}

impl Drop for X11Target {
    fn drop(&mut self) {
        unsafe {
            (self.xlib.XCloseDisplay)(self.display);
        }
    }
}

impl HasWindowHandle for X11Target {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let handle = XlibWindowHandle::new(self.window);
        // SAFETY: the window handle stays valid for as long as `self`
        // (and thus this borrow) is alive, matching WindowHandle's
        // lifetime contract.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Xlib(handle)) })
    }
}

impl HasDisplayHandle for X11Target {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let handle = XlibDisplayHandle::new(NonNull::new(self.display as *mut c_void), self.screen);
        // SAFETY: the display connection stays valid for as long as
        // `self` (and thus this borrow) is alive.
        Ok(unsafe { DisplayHandle::borrow_raw(RawDisplayHandle::Xlib(handle)) })
    }
}
