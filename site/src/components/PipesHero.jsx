import { useEffect, useRef, useState } from 'react'
import screensaverHero from '../assets/screensaver-hero.png'

// three.js is dynamically imported inside the effect below (not statically
// here) so it's a separate chunk the browser only fetches when the
// animation is actually going to run — never at all for
// prefers-reduced-motion, and not blocking first paint for anyone else.

// A deliberately simplified, decorative re-implementation of the actual
// pipes algorithm — grown in the browser with three.js, not a port of the
// real Rust simulation (pipes-core). It's the hero background showing the
// product doing the thing, live, instead of a static screenshot of it.
const GRID = { w: 10, h: 6, d: 10 }
const CELL = 1
const MAX_CONCURRENT_PIPES = 5
// Comfortably above (GRID.w*h*d * RESET_OCCUPANCY_RATIO) / PALETTE.length
// (600 * 0.5 / 5 = 60) — every occupied cell but a pipe's own starting one
// gets a segment instance *of that pipe's color*, so each color's own cap
// must clear its plausible worst-case share or segments would silently
// stop appearing for the last stretch before a reset fires. Generous
// headroom here since color assignment is random, not evenly split.
const MAX_INSTANCES_PER_COLOR = 150
const MAX_PIPE_LENGTH = 55
const RESET_OCCUPANCY_RATIO = 0.5
const TICK_MS = 130
const PALETTE = ['#22d3ee', '#a78bfa', '#34d399', '#f472b6', '#facc15']

const DIRECTIONS = [
  [1, 0, 0], [-1, 0, 0],
  [0, 1, 0], [0, -1, 0],
  [0, 0, 1], [0, 0, -1],
]

function key(x, y, z) {
  return `${x},${y},${z}`
}

function randomDirection(exclude) {
  const options = exclude
    ? DIRECTIONS.filter((d) => !(d[0] === -exclude[0] && d[1] === -exclude[1] && d[2] === -exclude[2]))
    : DIRECTIONS
  return options[Math.floor(Math.random() * options.length)]
}

function randomCell() {
  return [
    Math.floor(Math.random() * GRID.w),
    Math.floor(Math.random() * GRID.h),
    Math.floor(Math.random() * GRID.d),
  ]
}

class Pipe {
  constructor(occupied) {
    let start
    do {
      start = randomCell()
    } while (occupied.has(key(...start)))
    this.cell = start
    this.dir = randomDirection(null)
    this.color = PALETTE[Math.floor(Math.random() * PALETTE.length)]
    this.length = 0
    this.alive = true
    occupied.add(key(...start))
  }

  // Returns a { from, to } segment when it successfully advances, or null
  // when this pipe is stuck/finished (caller should retire it).
  step(occupied) {
    if (this.length >= MAX_PIPE_LENGTH) {
      this.alive = false
      return null
    }
    // Weighted like the real sim: straight runs are far more common than
    // turns, so pipes read as deliberate corridors, not random static.
    const tryOrder =
      Math.random() < 0.78
        ? [this.dir, randomDirection(this.dir), randomDirection(this.dir)]
        : [randomDirection(this.dir), this.dir, randomDirection(this.dir)]

    for (const dir of tryOrder) {
      const next = [this.cell[0] + dir[0], this.cell[1] + dir[1], this.cell[2] + dir[2]]
      const inBounds =
        next[0] >= 0 && next[0] < GRID.w &&
        next[1] >= 0 && next[1] < GRID.h &&
        next[2] >= 0 && next[2] < GRID.d
      if (!inBounds || occupied.has(key(...next))) continue

      occupied.add(key(...next))
      const from = this.cell
      const turned = dir !== this.dir
      this.cell = next
      this.dir = dir
      this.length += 1
      return { from, to: next, color: this.color, turned }
    }
    this.alive = false
    return null
  }
}

function buildScene(THREE, canvas) {
  const scene = new THREE.Scene()
  const camera = new THREE.PerspectiveCamera(45, 1, 0.5, 100)

  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true })
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
  renderer.setClearColor(0x000000, 0)

  const segmentGeo = new THREE.CylinderGeometry(0.16, 0.16, 1, 10)
  const jointGeo = new THREE.SphereGeometry(0.27, 10, 10)

  // One InstancedMesh pair (segments + joints) per palette color, each
  // with its own solid, unlit material.color — rather than one shared
  // InstancedMesh using per-instance vertex/instanceColor. Unlit
  // (MeshBasicMaterial) so no lighting/environment tuning is needed for
  // colors to show correctly at all. This is deliberately the simpler,
  // more verbose option: per-instance instanceColor rendered solid black
  // in real-device testing here for reasons that didn't resolve with the
  // documented API (setColorAt + instanceColor.needsUpdate) — likely a
  // driver/three.js-version-specific quirk — so this sidesteps that
  // mechanism entirely rather than trying to root-cause it further.
  const perColor = PALETTE.map((hex) => {
    const material = new THREE.MeshBasicMaterial({ color: hex })
    const segments = new THREE.InstancedMesh(segmentGeo, material, MAX_INSTANCES_PER_COLOR)
    const joints = new THREE.InstancedMesh(jointGeo, material, MAX_INSTANCES_PER_COLOR)
    // InstancedMesh's default bounding sphere comes from the base geometry
    // alone (a small unit shape near the origin) — it's never recomputed
    // as instances get placed across the grid, so the automatic
    // frustum-cull check sees a stale, tiny bounding volume and culls the
    // entire mesh the moment the orbiting camera looks anywhere else.
    // This is a small, always-in-view decorative scene, so disabling
    // culling entirely is simpler and cheaper than recomputing bounds
    // every frame.
    segments.frustumCulled = false
    joints.frustumCulled = false
    segments.count = 0
    joints.count = 0
    scene.add(segments, joints)
    return { hex, material, segments, joints, segmentCursor: 0, jointCursor: 0 }
  })

  const center = new THREE.Vector3((GRID.w - 1) / 2, (GRID.h - 1) / 2, (GRID.d - 1) / 2)
  // Closer than a "properly framed" shot would be — deliberately, so the
  // scene reads as filling the hero edge to edge the way the static photo
  // it replaces did, rather than looking like a small diorama floating in
  // a mostly-empty dark rectangle.
  const radius = Math.max(GRID.w, GRID.h, GRID.d) * 0.75

  return { scene, camera, renderer, perColor, center, radius, segmentGeo, jointGeo }
}

export default function PipesHero({ className = '', alt }) {
  const canvasRef = useRef(null)
  const [useCanvas, setUseCanvas] = useState(false)

  useEffect(() => {
    const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    if (prefersReducedMotion) return undefined
    if (!canvasRef.current) return undefined

    let cancelled = false
    let cleanup = () => {}

    import('three').then((THREE) => {
      if (cancelled || !canvasRef.current) return

      let ctx
      try {
        ctx = buildScene(THREE, canvasRef.current)
      } catch {
        return
      }
      if (!ctx.renderer.getContext()) return
      setUseCanvas(true)
      // onFailure: something went wrong *after* setup succeeded (a WebGL
      // context loss, a runtime exception mid-frame) — tear down and fall
      // back to the static image rather than leave a frozen or corrupted
      // canvas on screen with nothing recovering it.
      cleanup = setupAnimation(THREE, ctx, canvasRef, () => {
        if (!cancelled) setUseCanvas(false)
      })
    }).catch(() => {
      // three.js failed to load (offline, blocked, etc.) — the fallback
      // <img> is already what's rendered since useCanvas never flips true.
    })

    return () => {
      cancelled = true
      cleanup()
    }
  }, [])

  return (
    <>
      {/* A photo needs the same opacity-40 dimming the old static hero
          used for text legibility; this dark, comparatively sparse 3D
          scene reads as intentional (not an invisible mistake) at a much
          lighter dim instead — the two aren't interchangeable even though
          they share the same layout classes. */}
      <canvas
        ref={canvasRef}
        className={`${className} opacity-80 ${useCanvas ? '' : 'hidden'}`}
        aria-hidden="true"
      />
      {!useCanvas && (
        <img
          src={screensaverHero}
          alt={alt}
          className={`${className} opacity-40`}
        />
      )}
    </>
  )
}

// Everything from here down only ever runs after three.js has actually
// loaded — see the dynamic import() above — so it's a plain function
// rather than living inline in the effect, both for readability and so
// `buildScene`/this share the same `THREE` module reference explicitly
// instead of a module-level import.
function setupAnimation(THREE, ctx, canvasRef, onFailure) {
  const { scene, camera, renderer, perColor, center, radius, segmentGeo, jointGeo } = ctx
  const occupied = new Set()
  let pipes = []
  const dummy = new THREE.Object3D()
  const colorEntry = new Map(perColor.map((entry) => [entry.hex, entry]))

  function placeJoint(hex, cell) {
    const entry = colorEntry.get(hex)
    if (entry.jointCursor >= MAX_INSTANCES_PER_COLOR) return
    dummy.position.set(cell[0] * CELL, cell[1] * CELL, cell[2] * CELL)
    dummy.rotation.set(0, 0, 0)
    dummy.scale.setScalar(1)
    dummy.updateMatrix()
    entry.joints.setMatrixAt(entry.jointCursor, dummy.matrix)
    entry.jointCursor += 1
    entry.joints.count = entry.jointCursor
    entry.joints.instanceMatrix.needsUpdate = true
  }

  function placeSegment(hex, from, to) {
    const entry = colorEntry.get(hex)
    if (entry.segmentCursor >= MAX_INSTANCES_PER_COLOR) return
    const a = new THREE.Vector3(from[0] * CELL, from[1] * CELL, from[2] * CELL)
    const b = new THREE.Vector3(to[0] * CELL, to[1] * CELL, to[2] * CELL)
    const mid = a.clone().add(b).multiplyScalar(0.5)
    dummy.position.copy(mid)
    dummy.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), b.clone().sub(a).normalize())
    dummy.scale.set(1, 1, 1)
    dummy.updateMatrix()
    entry.segments.setMatrixAt(entry.segmentCursor, dummy.matrix)
    entry.segmentCursor += 1
    entry.segments.count = entry.segmentCursor
    entry.segments.instanceMatrix.needsUpdate = true
  }

  function resetScene() {
    occupied.clear()
    pipes = []
    for (const entry of perColor) {
      entry.segmentCursor = 0
      entry.jointCursor = 0
      entry.segments.count = 0
      entry.joints.count = 0
    }
    for (let i = 0; i < MAX_CONCURRENT_PIPES; i += 1) pipes.push(new Pipe(occupied))
  }
  resetScene()

  function tick() {
    if (occupied.size / (GRID.w * GRID.h * GRID.d) > RESET_OCCUPANCY_RATIO) {
      resetScene()
      return
    }
    // pipes.length is always exactly MAX_CONCURRENT_PIPES — a dead pipe
    // is replaced in place, never removed, so there's nothing to filter.
    for (let i = 0; i < pipes.length; i += 1) {
      const pipe = pipes[i]
      const result = pipe.step(occupied)
      if (result) {
        placeSegment(pipe.color, result.from, result.to)
        // Only at an actual turn, like the real app's ball/elbow joints —
        // not on every cell, which just clutters a straight run with a
        // sphere at both ends of every segment. The corner itself is the
        // cell the pipe turned *at* (from), not the first cell of its new
        // direction (to) — placing it at `to` put every joint one cell
        // past the actual bend.
        if (result.turned) placeJoint(pipe.color, result.from)
      } else {
        pipes[i] = new Pipe(occupied)
      }
    }
  }

  let running = true
  let rafId
  let lastTick = performance.now()
  let angle = 0

  function resize() {
    const el = canvasRef.current
    if (!el) return
    // The very first call can land before the browser has committed
    // layout for a canvas that just replaced the fallback <img> (both
    // reading 0), which — if skipped outright rather than clamped —
    // left the renderer stuck at three.js's internal default 300x150
    // drawing buffer forever, since nothing else ever calls resize()
    // again absent an actual window resize. Clamping to at least 1px
    // means this call always actually sets a size; the next real
    // ResizeObserver callback (guaranteed to fire with the correct
    // post-layout box) corrects it for good.
    const width = Math.max(el.clientWidth, 1)
    const height = Math.max(el.clientHeight, 1)
    camera.aspect = width / height
    camera.updateProjectionMatrix()
    renderer.setSize(width, height, false)
  }
  resize()
  const resizeObserver = new ResizeObserver(resize)
  resizeObserver.observe(canvasRef.current)

  let tornDown = false
  function teardown() {
    if (tornDown) return
    tornDown = true
    running = false
    cancelAnimationFrame(rafId)
    resizeObserver.disconnect()
    document.removeEventListener('visibilitychange', handleVisibility)
    canvasRef.current?.removeEventListener('webglcontextlost', handleContextLost)
    segmentGeo.dispose()
    jointGeo.dispose()
    for (const entry of perColor) entry.material.dispose()
    renderer.dispose()
  }

  // A frame throwing (or the GPU context dying under it) shouldn't leave a
  // frozen or corrupted canvas on screen forever with nothing recovering
  // it — tear everything down and let the caller fall back to the static
  // image instead, the same as if three.js/WebGL had never come up at all.
  function fail() {
    teardown()
    onFailure()
  }

  function frame(now) {
    if (!running) return
    try {
      if (now - lastTick >= TICK_MS) {
        lastTick = now
        tick()
      }
      angle += 0.0018
      camera.position.set(
        center.x + Math.cos(angle) * radius,
        center.y + radius * 0.55,
        center.z + Math.sin(angle) * radius,
      )
      camera.lookAt(center)
      renderer.render(scene, camera)
    } catch {
      fail()
      return
    }
    rafId = requestAnimationFrame(frame)
  }

  function handleContextLost(event) {
    event.preventDefault()
    fail()
  }
  canvasRef.current?.addEventListener('webglcontextlost', handleContextLost)

  function handleVisibility() {
    running = document.visibilityState === 'visible'
    if (running) {
      lastTick = performance.now()
      rafId = requestAnimationFrame(frame)
    } else {
      cancelAnimationFrame(rafId)
    }
  }
  document.addEventListener('visibilitychange', handleVisibility)
  rafId = requestAnimationFrame(frame)

  return teardown
}
