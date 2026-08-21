# Usage

> **Status**: Phase 2.5 (windowed rendering + a settings app) is live.
> There is still no installer and nothing to "select as your screensaver"
> yet — see [ROADMAP.md](ROADMAP.md) for Phase 3/4.

## Running the screensaver

```sh
cargo run -p pipes-app -- --seed 1
```

A window opens and pipes start growing immediately. Press **Escape** or
close the window to quit. See [LOGGING.md](LOGGING.md) for how to read the
console output alongside the window.

### Options

| Flag     | Default | Meaning |
|----------|---------|---------|
| `--seed` | `1`     | RNG seed. Same seed always reproduces the exact same run — see [ARCHITECTURE.md](ARCHITECTURE.md#pipes-core) on determinism. |

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

## Once Phase 3/4 land

This section will be filled in as each phase ships: download a platform
installer from GitHub Releases, run it, then select "neo_win_pipes" from:

- Windows: *Settings → Personalization → Lock screen → Screen saver*.
- macOS: *System Settings → Screen Saver*.
- Linux (via xscreensaver): `xscreensaver-demo`'s hack list.

Until then, treat this file's "today" section above as authoritative.
