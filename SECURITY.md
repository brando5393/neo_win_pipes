# Security Policy

neo_win_pipes is a solo/hobby open-source project (a screensaver, MIT
licensed — see [LICENSE](LICENSE)), not a company with a dedicated
security team. This policy is scoped to match that reality rather than
copy a corporate template wholesale: it's informed by
[NIST SP 800-218 (the Secure Software Development Framework)](https://csrc.nist.gov/projects/ssdf)
and [OpenSSF](https://openssf.org/)'s vulnerability-disclosure guidance,
applied at the scale this project actually operates at, not the scale
those documents assume.

## Reporting a vulnerability

**Preferred: [GitHub Private Vulnerability Reporting](https://github.com/brando5393/neo_win_pipes/security/advisories/new)**
(enabled on this repo). This reaches the maintainer directly and privately
— nothing is publicly visible until a fix is ready. If you can't use that
for some reason, open a regular issue asking for a private contact
without describing the vulnerability itself.

**What to expect**: this is one person, not a security operations
team. There's no 24-hour SLA to promise honestly. In practice: an
acknowledgment within a few days, and a fix or a documented mitigation
before any public disclosure, coordinated with you rather than sprung on
you. If you don't hear back within two weeks, that's a signal something
went wrong (spam filter, missed notification) — a follow-up nudge is
welcome, not annoying.

**What's in scope**: the actual shipped artifacts — the Windows `.msi`
and the `.scr`/`pipes-settings.exe` it installs, the Linux `.deb` and
AppImage, and (once it exists) the macOS `.saver` bundle. Also in scope:
the auto-update mechanism (`crates/pipes-settings/src/update.rs`) and the
splash site (`site/`, deployed at neowinpipes.com) to the extent it could
mislead someone into installing something malicious.

**What's out of scope, because it doesn't exist**: this software has no
server backend, no user accounts, no telemetry, and collects no user
data at all — the only network call it ever makes is a `GET` to GitHub's
public Releases API to check for a newer version. There's no attack
surface there beyond "is this response parsed safely," which is already
covered by ordinary test coverage (`crates/pipes-settings/src/update.rs`'s
`parse_update` is pure and unit-tested against malformed/adversarial
input, including plain garbage instead of JSON).

## Supported versions

Only the latest released version gets fixes. There's no LTS branch and,
realistically, no capacity to backport a fix to an older release — the
recommended remediation for a reported issue will always be "update to
the version that fixes it," via the in-app updater or a fresh install
from the [latest release](https://github.com/brando5393/neo_win_pipes/releases/latest).

## Current trust model — stated plainly, not glossed over

Being honest about what protection actually exists here matters more
than sounding secure:

- **The binaries are unsigned.** No code-signing certificate (a paid,
  ongoing cost, deliberately out of scope per [docs/ROADMAP.md](docs/ROADMAP.md)
  for a free hobby project). Windows SmartScreen will warn on first run
  and on updates — that warning is expected, not a sign something's
  wrong, but it also means Windows itself isn't vouching for this
  software's publisher identity the way it would for a signed binary.
- **What *does* get verified**: every release asset has a SHA-256
  checksum, both shown on [the splash site](https://neowinpipes.com)'s
  "Verify your download" panel and checked automatically by the in-app
  updater (`update::verify_checksum`) before it ever runs a downloaded
  `.msi` — using the digest GitHub itself computes for each asset, not a
  file we generate and could get out of sync. This catches a corrupted
  download or tampering in transit/at rest. **It is not a substitute for
  code signing**: it proves the bytes on disk match what GitHub says it
  served, not that the release itself is legitimate. The real trust
  anchor today is GitHub's platform integrity (account security,
  HTTPS/TLS) — a compromised maintainer GitHub account could still
  publish a malicious release with a "correct" checksum for itself.
- **The installer requires one UAC prompt**, both for the original
  install and for every update, because the screensaver executable has
  to live in `System32` (the OS's own convention for where the Screen
  Saver dropdown looks — see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)).
  The installer deliberately does *not* change your active screensaver
  selection or touch `HKCU\Control Panel\Desktop` on its own — installing
  only makes it available and selectable, same restraint any
  well-behaved installer should have toward existing settings it doesn't
  own.
- **`unsafe` Rust is confined to narrow, documented native-OS interop
  points** — Win32 window/notification APIs (`winsaver.rs`, `notify.rs`,
  `diagnostics.rs`) and raw X11 FFI (`pipes-xscreensaver::x11_target`).
  `pipes-core` (the actual simulation logic) contains zero `unsafe` code.
- **Dependencies** are pinned via `Cargo.lock` (committed to the repo,
  not gitignored) so every build — local or CI — resolves the exact same
  dependency versions; nothing installs whatever happens to be newest at
  build time.

## What this project already does, informally aligned with NIST SP 800-218 (SSDF)

Not a claim of formal SSDF compliance — a small hobby project doesn't
have the organizational apparatus that framework assumes (a security
team, formal risk assessments, etc.) — but several of its practices
already line up with SSDF's intent, and it's worth naming them rather
than starting from zero:

- **Produce Well-Secured Software (PW)**: every change to simulation
  logic ships with tests in the same commit — a founding, non-negotiable
  requirement for this repo (see [CLAUDE.md](CLAUDE.md)) — and property-based
  assertions ("never re-enters an occupied cell") are preferred over
  brittle golden-output tests. CI runs the full test suite, `cargo fmt
  --check`, and `cargo clippy -D warnings` on every push/PR across
  Windows, macOS, and Linux.
- **Protect the Software (PS)**: release artifacts are built by CI from
  a tagged commit (`.github/workflows/release.yml`), not hand-assembled
  and uploaded from a developer machine; `Cargo.lock` pins dependency
  versions; the downloaded-update checksum verification above.
- **Respond to Vulnerabilities (RV)**: this document, plus GitHub Private
  Vulnerability Reporting (enabled) as the actual reporting channel.

## Keeping this policy honest

This file, like the rest of `docs/`, is meant to reflect what's actually
true — see [CLAUDE.md](CLAUDE.md)'s documentation-maintenance convention.
If something above stops being accurate (code signing gets added, a new
network call is introduced, the update mechanism changes), update this
file in the same change, not as a follow-up.
