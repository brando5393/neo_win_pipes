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

Download `neo_win_pipes.msi` from the
[latest GitHub Release](https://github.com/brando5393/neo_win_pipes/releases/latest),
or build it yourself — see
[DEVELOPMENT.md](DEVELOPMENT.md#building-the-windows-installer-msi) for
how. Then just run it:

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

### If something goes wrong

A release build has no console window, so there's nothing to watch live
— but every run still writes a plain-text log file under
`%APPDATA%\neo-win-pipes\neo_win_pipes\data\logs\` (one file per binary
per day), and an unhandled crash shows a dialog pointing at that same
file with the technical detail included, so you don't have to go find it
yourself. See [LOGGING.md](LOGGING.md) for the full story.

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

### Update notifications

Pipes Settings checks GitHub for a newer release in the background each
time you open it. If one's available, a banner appears at the top:
**Update Now** downloads and launches the new installer (one UAC prompt,
same as installing — that's a real Windows security boundary, not
something an update can skip), **Release notes** opens what changed in
your browser, and **Dismiss** hides the banner for this session only
(it'll check again next time you open the app). See
[ARCHITECTURE.md](ARCHITECTURE.md#auto-update-pipes-settingsupdate) for
why this is the free, no-service, one-click version of "automatic
updates" rather than a fully silent one.

### On Windows-on-ARM64

If `cargo run` fails to link, see the toolchain caveat in
[DEVELOPMENT.md](DEVELOPMENT.md) — dot-source `scripts/dev-shell.ps1` first.

## Linux

> **Status**: real rendering code and a real `.deb`/AppImage exist and
> build/lint clean on actual `ubuntu-latest` CI — but nobody has
> installed either on a real machine and watched it render. See
> [ROADMAP.md](ROADMAP.md#phase-3--native-screensaver-wrappers) for the
> full honest breakdown of what's compiled-and-verified versus what's
> still unconfirmed.

Download `pipes-xscreensaver_<version>_amd64.deb` from the
[latest GitHub Release](https://github.com/brando5393/neo_win_pipes/releases/latest):

```sh
sudo apt install ./pipes-xscreensaver_<version>_amd64.deb
```

That installs the hack (into `/usr/libexec/xscreensaver/`, alongside its
config XML so `xscreensaver-demo` can find it) and `pipes-settings` (as a
regular app, with a Start-Menu-equivalent launcher entry). Then:

1. Open `xscreensaver-demo` (or your desktop environment's screen saver
   settings, if it wraps xscreensaver) and select **Neo Pipes** from the
   hack list — named that way, not "Pipes", since the real xscreensaver
   package already ships its own, different "Pipes" hack.
2. Open **Pipes Settings** from your application menu for the same live
   preview + settings drawer Windows has — same shared config file, same
   simulation/rendering code.
3. `sudo apt remove pipes-xscreensaver` to uninstall.

**Prefer a portable option, or not on a Debian-based distro?** Download
`PipesSettings-<version>-x86_64.AppImage`, `chmod +x` it, and run it — no
install, no root. This is `pipes-settings` only, not the screensaver hack
itself: an AppImage is an isolated bundle by design, and
`xscreensaver`'s driver discovers hacks by finding real files in real
system locations, which a portable bundle can't provide (the same reason
a portable `.zip` can't register itself in Windows' Screen Saver
dropdown without a real installer) — the `.deb` above is still the only
path to actually installing the screensaver.

### Testing the hack directly, without a full xscreensaver setup

```sh
pipes-xscreensaver -root       # draws on the root window
pipes-xscreensaver             # same thing (root is the default)
```

`-window-id <id>` (decimal or hex) is what `xscreensaver`'s driver
actually passes when running it for real — pass a specific X11 window ID
to test that path manually.

## macOS

Not installable yet, and no code exists at all — not even argument
parsing, unlike Linux. See
[ROADMAP.md](ROADMAP.md#macos--not-started) for why (no way to compile
Objective-C/Swift or link a Mach-O binary from this project's Windows dev
machine). Once it exists, the end state is the same shape as the others:
select "Neo Pipes" from *System Settings → Screen Saver*.
