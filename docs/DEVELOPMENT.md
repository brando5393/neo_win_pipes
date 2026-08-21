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
# One-time setup:
rustup target add x86_64-pc-windows-msvc
rustup toolchain install stable-x86_64-pc-windows-msvc --force-non-host
rustup override set stable-x86_64-pc-windows-msvc   # sets this per-directory, doesn't affect other projects
```

Then, **once per new PowerShell session**, load the x64 (not arm64) MSVC
dev environment before building/testing. `scripts/dev-shell.ps1` automates
this (it locates `vcvarsall.bat` and dot-sources the resulting `PATH`/
`LIB`/`INCLUDE`/`LIBPATH`):

```powershell
. .\scripts\dev-shell.ps1
cargo test --workspace
```

This is purely a local-machine workaround (this repo does not pin any
particular host toolchain — a normal x64 or properly-provisioned ARM64
Windows machine needs none of this).

## Building the Windows installer (`.msi`)

Prerequisites (no admin rights needed for any of this, since the Windows
Installer engine itself does the elevation dance later, at install time,
not at build time):

- [WiX Toolset v7+](https://wixtoolset.org/) as a per-user `dotnet` tool
  (avoids WiX v3's admin-only .NET Framework 3.5 Windows Feature
  dependency):
  ```powershell
  # One-time setup — .NET SDK and WiX, both installed per-user:
  Invoke-WebRequest -Uri "https://dot.net/v1/dotnet-install.ps1" -OutFile "$env:TEMP\dotnet-install.ps1"
  & "$env:TEMP\dotnet-install.ps1" -Channel LTS -InstallDir "$env:USERPROFILE\.dotnet"
  $env:Path = "$env:USERPROFILE\.dotnet;$env:Path"; $env:DOTNET_ROOT = "$env:USERPROFILE\.dotnet"
  dotnet tool install --global wix
  ```
- **WiX v7 requires accepting its EULA once** (the "Open Source
  Maintenance Fee" terms — free unless your project/org clears
  $10,000/year in revenue from projects using WiX, which doesn't apply
  here, but it's still a real legal acceptance, so we asked before doing
  it rather than accepting it silently):
  ```powershell
  $env:Path = "$env:USERPROFILE\.dotnet;$env:USERPROFILE\.dotnet\tools;" + $env:Path
  $env:DOTNET_ROOT = "$env:USERPROFILE\.dotnet"
  wix eula accept wix7
  wix extension add WixToolset.UI.wixext
  ```

Then, build release binaries and the `.msi`:

```powershell
. .\scripts\dev-shell.ps1   # if on Windows-on-ARM64, see the caveat above
cargo build --release -p pipes-app -p pipes-settings

$env:Path = "$env:USERPROFILE\.dotnet;$env:USERPROFILE\.dotnet\tools;" + $env:Path
$env:DOTNET_ROOT = "$env:USERPROFILE\.dotnet"
wix build installer\main.wxs -ext WixToolset.UI.wixext -arch x64 `
  -d PipesAppExe="target\release\pipes-app.exe" `
  -d PipesSettingsExe="target\release\pipes-settings.exe" `
  -o installer\out\neo_win_pipes.msi
```

`-arch x64` is required, not optional: `cargo build --release` produces
native 64-bit binaries, and without it WiX defaults to x86, silently
installing into the wrong `Program Files` directory (see the "Settings
button doesn't open" entry in `docs/ROADMAP.md`'s known-issues history for
the real bug this caused).

### Validating the `.msi` without actually installing it

Because `installer/main.wxs` puts a file in `System32`, the built `.msi`
requires admin elevation to actually install — which typically means a
UAC prompt, not something a non-interactive/automated session can click
through. Two checks that *don't* need elevation:

```powershell
# ICE validation (catches most authoring mistakes) — expect exactly one
# benign ICE09 warning about a "non-permanent system component", which is
# correct here: we want the .scr removed from System32 on uninstall.
wix msi validate installer\out\neo_win_pipes.msi

# Administrative extract (unpacks to an arbitrary folder, no elevation
# needed) — confirms the file layout is exactly right without installing:
msiexec /a installer\out\neo_win_pipes.msi /qn TARGETDIR="$env:TEMP\msi_check"
Get-ChildItem -Recurse "$env:TEMP\msi_check"   # expect PFiles\neo_win_pipes\pipes-settings.exe and System\neo_win_pipes.scr
```

The actual elevated install (`msiexec /i ...`, or just double-clicking the
`.msi`) needs to be done interactively by someone who can approve the UAC
prompt.

## Cutting a release

`.github/workflows/release.yml` does the rest once you push a tag:

```sh
git tag v0.2.0
git push origin v0.2.0
```

That single tag drives everything: CI patches `Cargo.toml`'s
`[workspace.package]` version to `0.2.0` (so the binaries' embedded
`CARGO_PKG_VERSION` matches), runs the full test suite, builds release
binaries and the `.msi` (`-d ProductVersion=0.2.0`, so the installer's
version matches too), and publishes a GitHub Release with the `.msi`
attached. This is also what `pipes-settings`' in-app update checker
watches for — see
[ARCHITECTURE.md](ARCHITECTURE.md#auto-update-pipes-settingsupdate) — so
a pushed tag is the entire "ship an update to everyone who's installed
this" step, not a separate manual upload.

Tag format matters: it must match `v*.*.*` (e.g. `v1.2.3`) both to
trigger the workflow and because `pipes-settings::update` strips the
leading `v` and parses the rest as semver — a malformed tag either won't
trigger a release or won't be recognized as an update by installed
copies.

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
