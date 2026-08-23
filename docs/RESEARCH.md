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
rendered with OpenGL. Windows XP shipped a Direct3D reimplementation. 3D
Pipes shipped in Windows 95 as well as NT, and was removed as a built-in
screensaver starting with Windows Vista.

Original OpenGL source lived in the Windows NT 4.0 SDK, under
`MSTOOLS\SAMPLES\OPENGL\SCRSAVE` (also distributed on the Visual C++ 5.0
Professional CD, under `devstudio\vc\samples\sdk\opengl\scrsave\`). A
mirror of that MSDN CD content, including the real `pipes` source in full
(`SSPIPES.CXX`, `PIPE.CXX`, `NPIPE.CXX`/`FPIPE.CXX`, `NODE.CXX`,
`OBJECTS.CXX`, `DIALOG.C`, etc.), is browsable at
[ianhan/MSDN_OPENGL_Samples](https://github.com/ianhan/MSDN_OPENGL_Samples/tree/main/SCRSAVE/PIPES)
— 2026-08-23 research for this project read that source directly rather
than inferring behavior from clones; see below for what it actually says,
some of which corrects assumptions in earlier revisions of this doc.

**The teapot easter egg is not in this OpenGL-era source.** A thorough
search of the real 1994-95 `SCRSAVE\PIPES` source (`SSPIPES.CXX`,
`PIPE.CXX`, `NPIPE.CXX`, `FPIPE.CXX`, `OBJECTS.CXX`, `EVAL.CXX`) turns up
no reference to a teapot or the Utah teapot at all. The widely-reported
teapot easter egg most likely belongs to the later Windows XP Direct3D
reimplementation instead, not this original OpenGL version — worth
treating as unconfirmed for the OpenGL era specifically, rather than
restating it as established fact the way earlier community writeups
(including an earlier draft of this doc) have.

## What the real original source actually does (verified by reading it, 2026-08-23)

Reading the actual source (not just clones or retrospectives) turned up
several things neo_win_pipes and every community clone surveyed below do
differently:

- **Per-pipe randomized "personality," not one fixed global straight/turn
  ratio.** `NPIPE.CXX`'s `NORMAL_PIPE` constructor: a 1-in-20 chance
  (`!ss_iRand(20)`) rolls `weightStraight` from a *high* range (25-100 out
  of a max of 100, heavily biased toward continuing straight for that
  pipe's whole life); the other 19-in-20 chance rolls it from a *low*
  range (1-4, heavily biased toward turning, close to a random walk).
  This is decided once per pipe at spawn, not per step. The visual effect:
  most pipes on screen are short/zigzaggy, and occasionally one pipe
  becomes a long straight runner — a genuinely different (and more
  organic-looking) pacing than a single fixed ratio produces uniformly
  across every pipe. **neo_win_pipes currently uses a single fixed
  `straight_weight: 10` / `turn_weight: 1` ratio applied to every pipe,
  every step** (`SimConfig::default()` in `crates/pipes-core/src/scene.rs`)
  — the same "weight straight ~10x" convention the community clones below
  use, not the original's per-pipe model. Worth considering as a real fix;
  see `docs/FEATURE_IDEAS.md`.
- **A second, fundamentally different pipe rendering mode: "Flex" pipes.**
  The dialog exposed `IDC_RADIO_NORMAL` vs `IDC_RADIO_FLEX`
  (`DIALOG.H`/`DIALOG.C`), and `FPIPE.CXX` ("Flex pipes") builds its
  geometry through an `EVAL` class (`EVAL.CXX`) — OpenGL evaluator/NURBS
  surfaces — producing a continuously-curved, bending tube through turns,
  rather than rigid straight cylinder segments joined by a separate elbow
  or ball at each corner. "Normal" pipes (`NPIPE.CXX`) are the
  segment-plus-joint model neo_win_pipes and every clone below implement.
  Flex mode appears to be a real, shipped, alternate visual style that
  nothing surveyed here (including neo_win_pipes) reproduces.
- **A textured surface option, not just solid chrome.** `ulSurfStyle` in
  `DIALOG.H` is a three-way enum: `SURFSTYLE_SOLID`, `SURFSTYLE_TEX`,
  `SURFSTYLE_WIREFRAME` — a real wireframe rendering mode existed
  alongside solid and (bitmap-)textured surfaces (`STRIPE.BMP`/
  `STRIPECY.BMP` ship in the same source directory as example textures,
  and the dialog supports up to `MAX_TEXTURES` (8) different texture
  files with a texture-quality toggle, `TEXQUAL_DEFAULT`/`TEXQUAL_HIGH`).
  neo_win_pipes only has solid materials today.
- **Four joint types, not two.** `JOINT_ELBOW`, `JOINT_BALL`,
  `JOINT_MIXED`, and `JOINT_CYCLE` (`DIALOG.H`). "Cycle" is distinct from
  "Mixed" — presumably cycling deterministically through joint styles
  rather than randomizing per joint, though the exact cycle order wasn't
  read in this pass. neo_win_pipes has elbow/ball/mixed
  (`elbow_probability`) but no cycle mode.
- **A tessellation-quality slider** (`fTesselFact`, 0.0-2.0, dialog
  trackbar `DLG_SETUP_TESSEL`) controlling how finely pipe surfaces are
  subdivided — a user-facing smoothness/performance tradeoff control we
  don't expose (our segment counts are fixed in code).
- **"Single pipe" vs "multiple pipes" was a binary radio button**
  (`IDC_RADIO_SINGLE_PIPE`/`IDC_RADIO_MULTIPLE_PIPES`), not a count slider.
  neo_win_pipes' `max_pipes` (1-32 slider) is strictly more capable here,
  not a gap — worth noting as a place we already exceed the original
  rather than something to "fix."

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
- [FaceFTW/rust-pipes](https://github.com/FaceFTW/rust-pipes) — the closest existing peer project (also Rust). Uses the `three-d` crate for rendering, explicitly references the original NT4 SDK source for behavior, and targets native + web/WASM (live demo at `pipes.faceftw.dev`) rather than a real OS-level screensaver package — no native `.scr`/`xscreensaver`-hack/`.saver` install path, which is exactly the gap neo_win_pipes' Phase 3/4 work fills that this peer doesn't attempt.
- [ianhan/MSDN_OPENGL_Samples](https://github.com/ianhan/MSDN_OPENGL_Samples/tree/main/SCRSAVE/PIPES) — the actual original 1994-95 OpenGL C++ source, not a clone or retrospective. Read directly for this doc's "What the real original source actually does" section above; supersedes assumptions inferred from clones wherever the two disagree.
- Origin-story press coverage of Raymond Chen's June 2024 post — [How-To Geek](https://www.howtogeek.com/3d-pipes-windows-screensaver-origin-story/), [TechSpot](https://www.techspot.com/news/103359-microsoft-programmer-reveals-how-iconic-3d-pipes-screensaver.html), [Neowin](https://www.neowin.net/news/we-now-know-how-and-why-microsoft-added-the-popular-3d-pipes-screensaver-to-windows/) — consistent with each other and with the original post; confirms Windows 95 inclusion and Vista-era removal, but none add developer regrets/wishlist commentary beyond the origin story itself (a deliberate blank: no source found claims to know what the original team wished they'd shipped).

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
