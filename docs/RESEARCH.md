# Research: the original 3D Pipes screensaver

This document captures what we know about the classic Windows "3D Pipes"
screensaver, as the reference point for what neo_win_pipes recreates. It
exists so design decisions in this repo can be traced back to *why* the
original behaved the way it did, rather than to vague nostalgia.

## Origin

3D Pipes shipped with Windows NT 3.5x/4.0 as part of a broader push by the
Windows OpenGL team to demonstrate the newly-added hardware-accelerated
OpenGL support, which otherwise had no visible presence in the OS. Several
competing OpenGL screensaver demos (3D Text, 3D Maze, 3D Flying Objects, 3D
Pipes) were built internally and put to an informal vote among the Windows
NT team; a marketing staffer who saw them decided to ship all of them rather
than pick a winner. ([The Old New Thing, June 2024](https://devblogs.microsoft.com/oldnewthing/20240611-00/?p=109881))

From Windows NT 4.0 through Windows Me, the screensaver (`sspipes.scr`) was
rendered with OpenGL. Windows XP shipped a Direct3D reimplementation.
Original OpenGL source lived in the Windows NT 4.0 SDK, under
`MSTOOLS\SAMPLES\OPENGL\SCRSAVE`.

A well-known easter egg in the OpenGL-era version: with style set to
"Mixed" and resolution at maximum with multiple tubes, a teapot (the
classic Utah teapot, a standard OpenGL test model) would occasionally
render at a pipe corner instead of a joint.

## Observed behavior (from the original and from prior clones)

- Several pipes (commonly four to six) grow concurrently through a 3D
  scene, starting from random points.
- Each pipe advances in one of six axis-aligned directions (±X, ±Y, ±Z) one
  grid-unit at a time. It never reverses directly into the cell it just
  came from.
- Continuing straight is far more likely than turning at any given step —
  pipes read as long runs punctuated by occasional turns, not constant
  zig-zag. Community re-implementations model this as a large weight bias
  toward the current direction (e.g. a well-known JS/Three.js clone weights
  the current direction 10x versus each alternative).
- A turn is rendered as either a smooth elbow bend or a ball/sphere joint.
  One well-documented clone uses a 75% elbow / 25% ball split, which read
  as visually close to the original and is what we've adopted as our
  default (`SimConfig::elbow_probability`).
- Pipes have at least two cross-section styles: round (chrome/reflective —
  the signature look) and square/rectangular ("mixed" style mixes both in
  one scene).
- Pipe segments occupy grid cells; once every neighboring cell of the
  current head is either occupied or out of the bounding volume, the pipe
  stops growing and a new one starts elsewhere.
- The scene periodically clears (fades/cuts to black) and restarts once it
  gets sufficiently full, rather than growing forever or looping a single
  fixed-length animation.
- Configuration in the original Windows dialog included: pipe style
  (traditional / mixed, each with a "small" variant), number of pipes,
  surface resolution, whether the camera slowly rotates, and (via registry
  in some versions) texture/material choice ("chrome" being the iconic
  default).

## Prior art consulted

- [devblogs.microsoft.com — "The origin story of the Windows 3D Pipes screen saver"](https://devblogs.microsoft.com/oldnewthing/20240611-00/?p=109881) — authoritative history from Raymond Chen (Microsoft).
- [microsoft.fandom.com/wiki/3D_Pipes](https://microsoft.fandom.com/wiki/3D_Pipes) — version history (OpenGL → Direct3D transition), easter egg details.
- [1j01/pipes](https://github.com/1j01/pipes) and [Alex313031/webgl-pipes](https://github.com/Alex313031/webgl-pipes) — web-based clones; useful for confirming the grid/direction-weighting/joint-mix approach against another independent reimplementation.
- [eschluntz/pipes-screensaver](https://github.com/eschluntz/pipes-screensaver) — a Three.js clone (built largely with Claude) confirming the 75/25 elbow/ball joint split and the "weight current direction ~10x" turning model; also a useful cautionary example (no pipe/pipe collision detection, packaged only as a static HTML page, not a real OS screensaver) of what neo_win_pipes intentionally does differently — see [ARCHITECTURE.md](ARCHITECTURE.md) for how we keep collision detection and native packaging in scope from the start.

## What we're deliberately doing differently

The original (and most clones) are single-file, render-only programs with
no automated tests and no real OS-level screensaver packaging (most clones
are just an HTML page or a loose executable). neo_win_pipes' explicit goals
per [ARCHITECTURE.md](ARCHITECTURE.md) are: a simulation core with zero
rendering dependencies so it's fully unit-testable, deterministic seeded
simulation (for reproducible tests and bug reports), structured
human-readable logging of every simulation event (see
[LOGGING.md](LOGGING.md)), and — eventually — genuine installable,
OS-selectable screensaver packages on all three platforms (see
[ROADMAP.md](ROADMAP.md)), not just a fullscreen app you have to wire up by
hand.
