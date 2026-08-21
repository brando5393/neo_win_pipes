# Usage

> **Status**: Phase 4 (Windows) is live — there's a real `.msi` installer.
> macOS/Linux aren't installable screensavers yet, and there's no
> `.pkg`/`.deb` installer for them — see [ROADMAP.md](ROADMAP.md).

## Running the screensaver (dev/manual testing)

```sh
cargo run -p pipes-app -- --seed 1
```

A window opens and pipes start growing immediately — since no
screensaver flag was recognized, this behaves like `/s` (see below).
Press **Escape**, click, move the mouse, or close the window to quit
(there's a short grace period right after startup so opening the window
itself doesn't immediately count as input).

### Options

| Flag     | Default | Meaning |
|----------|---------|---------|
| `--seed` | `1`     | RNG seed. Same seed always reproduces the exact same run — see [ARCHITECTURE.md](ARCHITECTURE.md#pipes-core) on determinism. |

## Testing the Windows screensaver contract directly

`pipes-app` understands the same command-line contract Windows itself
uses to drive a `.scr` — see
[ARCHITECTURE.md](ARCHITECTURE.md#native-screensaver-wrappers-phase-3)
for the full writeup. You can exercise each mode manually:

```sh
cargo run -p pipes-app -- /s              # fullscreen, exits on any input
cargo run -p pipes-app -- /c              # launches pipes-settings, exits
cargo run -p pipes-app -- /p 123456       # embeds into HWND 123456 (from another app)
```

## Installing it as your actual Windows screensaver

Build (or download, once GitHub Releases carries one — see
[ROADMAP.md](ROADMAP.md)) `neo_win_pipes.msi` — see
[DEVELOPMENT.md](DEVELOPMENT.md#building-the-windows-installer-msi) for
how it's built. Then just run it:

1. Double-click `neo_win_pipes.msi`. Windows will prompt for
   administrator approval (UAC) — installing to `System32` is genuinely a
   system-wide change, so this prompt is expected and correct, not a bug.
2. Click through the installer (Welcome → license → Install → Finish).
   That's it — no manual file copying, no finding `System32` yourself.
3. Open *Settings → Personalization → Lock screen → Screen saver*, pick
   "neo_win_pipes" from the dropdown. The installer does **not** select it
   for you automatically — it only makes it available, the same as
   installing any other screensaver, so it never silently overrides
   whatever you had configured before.
4. A **Pipes Settings** shortcut is added to the Start Menu too, so you
   can open the live-preview settings drawer any time, independent of the
   screensaver being active — the same app the `/c` config button opens.
5. Uninstall from *Settings → Apps* like any other program — this cleanly
   removes both the `.scr` from `System32` and the settings app.

### Environment variables

| Variable   | Meaning |
|------------|---------|
| `RUST_LOG` | Log verbosity/filter, e.g. `RUST_LOG=debug`. See [LOGGING.md](LOGGING.md). |

## Running the settings app

```sh
cargo run -p pipes-settings
```

Opens "Pipes Settings": a live 3D preview on the left, a settings drawer
on the right (pipe style & count, speed & camera, color palette, grid size
& reset threshold). Every change applies to the live preview immediately
and autosaves to the shared config file shown at the bottom of the drawer
— the next time you run `pipes-app`, it picks up the same settings. Click
**Reset to defaults** to discard all customization.

The config file lives at the OS's standard per-user config location (e.g.
`%APPDATA%\neo_win_pipes\config.toml` on Windows) — see
[ARCHITECTURE.md](ARCHITECTURE.md#pipes-render) for the full `AppConfig`
shape if you want to hand-edit it (it's validated/clamped on load either
way, so a bad edit can't break the app).

### On Windows-on-ARM64

If `cargo run` fails to link, see the toolchain caveat in
[DEVELOPMENT.md](DEVELOPMENT.md) — dot-source `scripts/dev-shell.ps1` first.

## macOS / Linux

Not installable screensavers yet — see
[ROADMAP.md](ROADMAP.md#phase-3--native-screensaver-wrappers) for exactly
what's done (Linux: argument parsing) versus not started (macOS). Once
they land, the end state is the same shape as Windows above: select
"neo_win_pipes" from *System Settings → Screen Saver* (macOS) or
`xscreensaver-demo`'s hack list (Linux).

## Once macOS/Linux installers land

Same shape as Windows above: download a `.pkg`/`.deb` (or AppImage) from
GitHub Releases and run it — no manual file copying. Not there yet; see
[ROADMAP.md](ROADMAP.md).
