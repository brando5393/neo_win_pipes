# Development guide

## Prerequisites

- [Rust](https://rustup.rs/) (stable channel). This repo currently targets
  edition 2021, `rust-version` unset (any recent stable works).
- A platform C/C++ toolchain, for the MSVC/native linker Rust needs:
  - **Windows**: [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) with the "Desktop development with C++" workload (installs the MSVC linker `link.exe`/`cl.exe`). See the **Windows on ARM caveat** below if you're on an ARM64 machine.
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`).
  - **Linux**: `build-essential` (Debian/Ubuntu) or your distro's equivalent (`gcc`, `pkg-config`).
- Phase 2 onward will also need GPU drivers with Vulkan/Metal/DX12 support
  for `wgpu` — not required yet for Phase 1 (headless).

## Build & test

```sh
cargo build --workspace
cargo test --workspace
cargo run -p pipes-app -- --ticks 200 --seed 1
```

`cargo test --workspace` is the source of truth for "does this change
work" — every module in `pipes-core` carries its own `#[cfg(test)] mod
tests`, and CI runs this exact command on all three OSes (see
`.github/workflows/ci.yml`) on every push.

## Testing philosophy

- **The simulation core stays headless-testable, permanently.** If a
  change to `pipes-core` can only be verified by looking at a rendered
  window, that logic belongs in `pipes-app`'s rendering layer instead —
  keep geometry/physics/state-machine logic in `pipes-core` where
  `cargo test` can reach it.
- **Determinism is a feature to protect.** `same_seed_produces_identical_*`
  tests exist because a seeded `Scene` must be bit-for-bit reproducible.
  If you touch RNG usage (adding a new `rng.gen_*()` call, reordering
  existing ones, changing iteration order that affects draw order), run
  the determinism tests specifically and expect to need to update them —
  that's a real behavior change, not a false positive.
- **Prefer property-style tests over one golden value** where the
  property is what actually matters (e.g. "never re-enters an occupied
  cell", "never leaves bounds") rather than asserting one exact path,
  which would make the test brittle to unrelated RNG-usage changes.
- **New simulation behavior needs a new test in the same change**, not a
  follow-up. This was a project ground rule from day one, not a
  retrofit — see the commit history.

## Windows on ARM64 caveat (workaround, not a project requirement)

If `cargo build` fails while linking a build script with an error like
`link: extra operand '...'`, your shell's PATH has a non-MSVC `link`
utility shadowing the real linker (e.g. MSYS2/Git Bash's coreutils `link`).
Run from PowerShell instead, or fix PATH ordering so MSVC's `link.exe` from
your Visual Studio Build Tools installation wins.

If you're specifically on **Windows-on-ARM64** and only have the default
Build Tools components installed (no ARM64 C++ workload), you'll hit an
`aarch64-pc-windows-msvc` linker failure because Cargo always compiles
build scripts for your *host* triple, and there's no ARM64 linker
available. The fastest fix, without downloading the (large) ARM64 C++
Build Tools component, is to build under x64 emulation instead:

```powershell
rustup target add x86_64-pc-windows-msvc
rustup toolchain install stable-x86_64-pc-windows-msvc --force-non-host
rustup override set stable-x86_64-pc-windows-msvc   # sets this per-directory, doesn't affect other projects

# Load the x64 (not arm64) MSVC dev environment before building/testing:
$vcvars = "C:\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"   # adjust path to your VS install
cmd /c "`"$vcvars`" x64 >nul 2>&1 && set" | Out-File "$env:TEMP\vcenv.txt" -Encoding utf8
Get-Content "$env:TEMP\vcenv.txt" | Where-Object { $_ -match "^(PATH|LIB|INCLUDE|LIBPATH)=" } | ForEach-Object {
  $i = $_.IndexOf('='); Set-Item "env:$($_.Substring(0,$i))" $_.Substring($i+1)
}

cargo test --workspace
```

This is purely a local-machine workaround (this repo does not pin any
particular host toolchain — a normal x64 or properly-provisioned ARM64
Windows machine needs none of this).

## Commit / PR conventions

- Every change to simulation logic ships with its tests in the same
  commit/PR.
- Every new logged event gets a row in [`docs/LOGGING.md`](LOGGING.md) in
  the same change.
- Keep `docs/ROADMAP.md` checkboxes in sync with what actually shipped.
- No `--no-verify`, no skipped hooks, no force-pushing shared branches.

## Where things live

See [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) for the crate layout and
[`docs/ROADMAP.md`](ROADMAP.md) for what's built vs. planned.
