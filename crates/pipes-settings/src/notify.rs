//! Native OS notification for "an update is available", fired once
//! alongside the in-app banner in `ui.rs`/`main.rs` — not a replacement
//! for it, since the banner is also how the user actually clicks
//! "Update Now". Windows-only for now: this project has no Linux/macOS
//! machine to actually watch a notification render and confirm a click
//! reaches this process, and `CLAUDE.md`'s conventions are explicit about
//! not shipping unverified platform-specific code as if it were tested
//! (see `docs/ROADMAP.md` for this gap). Non-Windows builds just no-op;
//! the in-app banner still works everywhere.

#[cfg(windows)]
mod windows_impl {
    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Foundation::TypedEventHandler;
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

    /// Matches this app's own identity — doesn't need to be pre-registered
    /// anywhere (no COM activator, no MSIX packaging) for a toast to show
    /// and its `Activated` event to reach this same, still-running
    /// process; that simple in-process case is all `on_activated` needs.
    ///
    /// Must match `installer/main.wxs`'s `System.AppUserModel.ID`
    /// `ShortcutProperty` on the Start Menu shortcut *exactly* — nothing
    /// enforces that at compile time or build time, so a future edit to
    /// either string alone silently breaks the toast (it still "succeeds"
    /// per the WinRT API, just never visibly appears — the exact failure
    /// mode this took a real run to diagnose the first time).
    const AUMID: &str = "BrandonWilliams.neo_win_pipes.PipesSettings";

    pub fn notify_update_available(version: &str, on_activated: impl Fn() + Send + 'static) {
        if let Err(err) = try_notify(version, on_activated) {
            tracing::warn!(?err, "failed to show update toast notification");
        }
    }

    fn try_notify(
        version: &str,
        on_activated: impl Fn() + Send + 'static,
    ) -> windows::core::Result<()> {
        // Safety: takes a plain string identifying this process to the
        // shell; no preconditions beyond being called on this process's
        // own thread, which it is.
        unsafe {
            SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(AUMID))?;
        }

        let xml = format!(
            "<toast><visual><binding template='ToastGeneric'>\
             <text>Update available</text>\
             <text>neo_win_pipes v{version} is ready to install. Click to open Pipes Settings.</text>\
             </binding></visual></toast>"
        );
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;

        let toast = ToastNotification::CreateToastNotification(&doc)?;
        toast.Activated(&TypedEventHandler::new(move |_, _| {
            on_activated();
            Ok(())
        }))?;

        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?;
        notifier.Show(&toast)?;
        Ok(())
    }
}

#[cfg(windows)]
pub use windows_impl::notify_update_available;

#[cfg(not(windows))]
pub fn notify_update_available(_version: &str, _on_activated: impl Fn() + Send + 'static) {}
