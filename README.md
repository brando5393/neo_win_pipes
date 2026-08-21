# neo_win_pipes

A modern, cross-platform (Windows / macOS / Linux) recreation of the
classic Windows "3D Pipes" screensaver — the chrome pipes one, from Windows
NT 4.0 onward. Goal: an installable, OS-selectable screensaver on all three
platforms, built on a fully unit-tested Rust simulation core.

**Status: Phase 2.5 — windowed rendering + a settings app.** Real pipes,
growing and turning in a real window, plus a "Pipes Settings" app with a
live preview and a settings drawer (style, count, speed, camera, color
palette, grid size). No installer/native screensaver packaging yet. See
[`docs/ROADMAP.md`](docs/ROADMAP.md) for what's built and what's next.

![A window full of colored 3D pipes growing through a grid](docs/screenshots/phase2-first-render-seed3.png)

## Quick start

```sh
cargo test --workspace                  # run the full test suite (36 tests)
cargo run -p pipes-app -- --seed 1      # the screensaver itself
cargo run -p pipes-settings             # live preview + settings drawer
```

See [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) for prerequisites
(including a Windows-on-ARM64 build caveat) and
[`docs/USAGE.md`](docs/USAGE.md) for CLI flags and environment variables.

## Documentation

| Doc | Covers |
|-----|--------|
| [`docs/RESEARCH.md`](docs/RESEARCH.md) | What the original screensaver actually did, and sources. |
| [`docs/FEATURE_IDEAS.md`](docs/FEATURE_IDEAS.md) | Community-sourced backlog — what users of prior pipes projects actually asked for. |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Crate layout, why the core has no rendering deps. |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Phased plan from headless core to installable packages. |
| [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) | Build/test setup, testing philosophy, contribution rules. |
| [`docs/USAGE.md`](docs/USAGE.md) | How to run what exists today. |
| [`docs/LOGGING.md`](docs/LOGGING.md) | The human-readable log event catalog. |
| [`CLAUDE.md`](CLAUDE.md) | Conventions for AI-assisted work in this repo. |

## License

MIT — see [`LICENSE`](LICENSE).
