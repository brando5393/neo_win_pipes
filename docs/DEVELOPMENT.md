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

## Benchmarks

`pipes-core` and `pipes-render` each have a [Criterion](https://bheisler.github.io/criterion.rs/book/)
benchmark suite (`benches/`), separate from `cargo test`:

```sh
cargo bench -p pipes-core      # Scene::step — the simulation tick every front-end calls
cargo bench -p pipes-render    # build_instances — rebuilt from scratch every frame
```

Each has a `default_config` case (the shipped defaults) and a `large_scene`
case (a much bigger grid/pipe count, stressing the same path a user could
reach via the settings app's "Grid size & reset"/"Pipe style & count"
panels). As of this writing, on this project's own dev machine (Qualcomm
Adreno X1-85, Windows-on-ARM64):

| Benchmark | default_config | large_scene (64³ grid, 200 pipes) |
|---|---|---|
| `scene_step` | ~3.7 µs | ~159 µs |
| `build_instances` | ~77 µs | ~14 ms |

The gap matters: `build_instances` at the large-scene scale costs ~14ms —
most of a 60fps frame's entire 16.6ms budget — while `scene_step` at the
*same* scale is 88x cheaper. **The simulation logic is not what limits how
large a scene you can run smoothly; rebuilding the per-frame instance
buffers is.** Keep this in mind before assuming a slowdown is `pipes-core`'s
fault, and see `crates/pipes-settings/src/benchmark.rs` for the in-app
benchmark end users can run themselves against their own real GPU (these
Criterion benchmarks are CPU-only and don't touch a `Renderer` at all).

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
  -loc installer\strings.en-us.wxl `
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

## Building the Linux packages (`.deb` / AppImage)

Needs a real Linux machine — `installer/linux/*.sh` are bash scripts
using `dpkg-deb`/`curl`, and `pipes-xscreensaver`'s `x11-dl` dependency,
while it costs nothing to *compile* elsewhere, obviously does nothing
useful anywhere but Linux. This project's own dev machine doesn't have
one; `.github/workflows/release.yml`'s `linux-packages` job (a real
`ubuntu-latest` runner) is where this actually gets built and validated
today.

```sh
sudo apt-get install -y libx11-dev libxi-dev libxcursor-dev libxrandr-dev \
  libxkbcommon-dev libwayland-dev libudev-dev libasound2-dev pkg-config

cargo build --release -p pipes-xscreensaver -p pipes-settings

installer/linux/build-deb.sh <version> \
  target/release/pipes-xscreensaver \
  target/release/pipes-settings \
  installer/out/pipes-xscreensaver_<version>_amd64.deb

dpkg-deb --info installer/out/pipes-xscreensaver_<version>_amd64.deb
dpkg-deb --contents installer/out/pipes-xscreensaver_<version>_amd64.deb

installer/linux/build-appimage.sh <version> \
  target/release/pipes-settings \
  "installer/out/PipesSettings-<version>-x86_64.AppImage"
```

`build-appimage.sh` downloads `appimagetool` itself (no local install
needed) and runs it with `APPIMAGE_EXTRACT_AND_RUN=1` — CI runners
(and plenty of desktop Linux setups) don't have FUSE available, which
`appimagetool` and the AppImage it produces would otherwise need to
mount themselves.

Neither script's output has actually been installed on a real machine yet
— see the honest gap called out in `docs/ROADMAP.md` and
`docs/ARCHITECTURE.md`'s Linux sections. If you have real Linux access,
confirming that gap (does `xscreensaver-demo` actually list and run "Neo
Pipes"? does a window actually render?) is the single most valuable thing
you could check.

## Cutting a release

`.github/workflows/release.yml` does the rest once you push a tag:

```sh
git tag vX.Y.Z
git push origin vX.Y.Z
```

That single tag drives everything: CI patches `Cargo.toml`'s
`[workspace.package]` version to `X.Y.Z` (so the binaries' embedded
`CARGO_PKG_VERSION` matches), runs the full test suite, builds release
binaries and the `.msi` (`-d ProductVersion=X.Y.Z`, so the installer's
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
- **When a change is genuinely user-facing** (a new feature someone would
  actually notice, a platform going from unverified to confirmed-working,
  a new install method) **check whether `site/` (the neowinpipes.com
  splash page) needs a corresponding update.** It's a hand-maintained
  static site, not generated from these docs or the wiki — nothing keeps
  it in sync automatically. Not every change needs this (an internal
  refactor or a bug fix usually doesn't); use judgment, but check rather
  than assume it's someone else's job.
- No `--no-verify`, no skipped hooks, no force-pushing shared branches.

## Where things live

See [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) for the crate layout and
[`docs/ROADMAP.md`](ROADMAP.md) for what's built vs. planned.
