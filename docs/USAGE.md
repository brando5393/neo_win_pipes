# Usage

> **Status**: Phase 1 (headless simulation core) is what exists today.
> There is no window, no installer, and nothing to "select as your
> screensaver" yet — see [ROADMAP.md](ROADMAP.md) for what's coming and
> when this document will start covering it.

## Running the simulation today

The only user-facing artifact right now is `pipes-app`, a CLI that runs the
simulation headlessly and logs what it's doing (see
[LOGGING.md](LOGGING.md) for how to read the output):

```sh
cargo run -p pipes-app -- --ticks 500 --seed 1
```

### Options

| Flag       | Default | Meaning |
|------------|---------|---------|
| `--ticks`  | `200`   | How many simulation steps to run before exiting. |
| `--seed`   | `1`     | RNG seed. Same seed + same tick count always reproduces the exact same run — see [ARCHITECTURE.md](ARCHITECTURE.md#pipes-core) on determinism. |

### Environment variables

| Variable   | Meaning |
|------------|---------|
| `RUST_LOG` | Log verbosity/filter, e.g. `RUST_LOG=debug`. See [LOGGING.md](LOGGING.md). |

## Once Phase 2/3/4 land

This section will be filled in as each phase ships:

- **Phase 2** (windowed rendering): `cargo run -p pipes-app` opens an
  actual window and renders the pipes, instead of only logging.
- **Phase 3/4** (installable, selectable screensaver): download a
  platform installer from GitHub Releases, run it, then select
  "neo_win_pipes" from:
  - Windows: *Settings → Personalization → Lock screen → Screen saver*.
  - macOS: *System Settings → Screen Saver*.
  - Linux (via xscreensaver): `xscreensaver-demo`'s hack list.

Until then, treat this file's "today" section above as authoritative and
everything else as forward-looking.
