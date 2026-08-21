# Usage

> **Status**: Phase 2 (windowed rendering) is live — there's a real window
> with real pipes now. There is still no installer and nothing to "select
> as your screensaver" yet — see [ROADMAP.md](ROADMAP.md) for Phase 3/4.

## Running it

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
