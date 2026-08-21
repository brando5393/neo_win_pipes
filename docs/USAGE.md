# Usage

> **Status**: Phase 3 (Windows) is live — `pipes-app` understands the real
> Windows screensaver contract now. macOS/Linux aren't installable
> screensavers yet, and there's no `.msi`/`.pkg`/`.deb` installer on any
> platform yet — see [ROADMAP.md](ROADMAP.md).

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

1. Build a release binary: `cargo build --release -p pipes-app -p pipes-settings`.
2. Copy `target\release\pipes-app.exe`, renamed to e.g. `neo_win_pipes.scr`,
   **and** `pipes-settings.exe` into the same folder (`/c` needs
   `pipes-settings.exe` sitting right next to the `.scr`).
3. To appear in *Settings → Personalization → Lock screen → Screen saver*'s
   dropdown, that folder needs to be `%WINDIR%\System32` — which requires
   administrator rights to copy into. **This is a system-wide change, so
   ask before doing it automatically** rather than having an assistant
   copy files into `System32` on your behalf. Alternatively, right-click
   the `.scr` file → *Install* does the same thing through Windows' own
   UI, without needing a manual admin copy.
4. Once installed, Windows drives it entirely through the `/s`/`/c`/`/p`
   contract above — you shouldn't need to touch the command line again.

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

## Once Phase 4 (installers) lands

Right now, "installing" the Windows screensaver means the manual copy
steps above. Phase 4 replaces that with: download a `.msi`/`.pkg`/`.deb`
from GitHub Releases and run it — no manual file copying or `System32`
admin steps needed.
