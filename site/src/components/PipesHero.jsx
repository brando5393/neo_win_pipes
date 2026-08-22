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
// Comfortably above GRID.w*h*d * RESET_OCCUPANCY_RATIO (600 * 0.5 = 300):
// every occupied cell but a pipe's own starting one gets a segment
// instance, so the cap must clear the reset threshold or segments would
// silently stop appearing for the last stretch before a reset fires.
const MAX_SEGMENT_INSTANCES = 320
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
      this.cell = next
      this.dir = dir
      this.length += 1
      return { from, to: next, color: this.color }
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

  scene.add(new THREE.AmbientLight(0xffffff, 0.35))
  const key1 = new THREE.DirectionalLight(0xffffff, 1.1)
  key1.position.set(6, 9, 4)
  scene.add(key1)
  const fill = new THREE.DirectionalLight(0x88aaff, 0.4)
  fill.position.set(-5, 2, -6)
  scene.add(fill)

  const segmentGeo = new THREE.CylinderGeometry(0.16, 0.16, 1, 10)
  const jointGeo = new THREE.SphereGeometry(0.22, 10, 10)
  const material = new THREE.MeshStandardMaterial({ vertexColors: true, metalness: 0.55, roughness: 0.35 })

  const segments = new THREE.InstancedMesh(segmentGeo, material, MAX_SEGMENT_INSTANCES)
  const joints = new THREE.InstancedMesh(jointGeo, material, MAX_SEGMENT_INSTANCES)
  segments.count = 0
  joints.count = 0
  scene.add(segments, joints)

  const center = new THREE.Vector3((GRID.w - 1) / 2, (GRID.h - 1) / 2, (GRID.d - 1) / 2)
  const radius = Math.max(GRID.w, GRID.h, GRID.d) * 1.15

  return { scene, camera, renderer, segments, joints, center, radius, segmentGeo, jointGeo, material }
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
      cleanup = setupAnimation(THREE, ctx, canvasRef)
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
      <canvas
        ref={canvasRef}
        className={`${className} ${useCanvas ? '' : 'hidden'}`}
        aria-hidden="true"
      />
      {!useCanvas && (
        <img
          src={screensaverHero}
          alt={alt}
          className={className}
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
function setupAnimation(THREE, ctx, canvasRef) {
  const { scene, camera, renderer, segments, joints, center, radius, segmentGeo, jointGeo, material } = ctx
  const occupied = new Set()
  let pipes = []
  let segmentCursor = 0
  let jointCursor = 0
  const dummy = new THREE.Object3D()
  const color = new THREE.Color()

  function placeJoint(cell) {
    if (jointCursor >= MAX_SEGMENT_INSTANCES) return
    dummy.position.set(cell[0] * CELL, cell[1] * CELL, cell[2] * CELL)
    dummy.rotation.set(0, 0, 0)
    dummy.scale.setScalar(1)
    dummy.updateMatrix()
    joints.setMatrixAt(jointCursor, dummy.matrix)
    joints.setColorAt(jointCursor, color)
    jointCursor += 1
    joints.count = jointCursor
    joints.instanceColor.needsUpdate = true
  }

  function placeSegment(from, to) {
    if (segmentCursor >= MAX_SEGMENT_INSTANCES) return
    const a = new THREE.Vector3(from[0] * CELL, from[1] * CELL, from[2] * CELL)
    const b = new THREE.Vector3(to[0] * CELL, to[1] * CELL, to[2] * CELL)
    const mid = a.clone().add(b).multiplyScalar(0.5)
    dummy.position.copy(mid)
    dummy.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), b.clone().sub(a).normalize())
    dummy.scale.set(1, 1, 1)
    dummy.updateMatrix()
    segments.setMatrixAt(segmentCursor, dummy.matrix)
    segments.setColorAt(segmentCursor, color)
    segmentCursor += 1
    segments.count = segmentCursor
    segments.instanceColor.needsUpdate = true
  }

  function resetScene() {
    occupied.clear()
    pipes = []
    segmentCursor = 0
    jointCursor = 0
    segments.count = 0
    joints.count = 0
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
      color.set(pipe.color)
      const result = pipe.step(occupied)
      if (result) {
        placeSegment(result.from, result.to)
        placeJoint(result.to)
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

  function frame(now) {
    if (!running) return
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
    rafId = requestAnimationFrame(frame)
  }

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

  return () => {
    running = false
    cancelAnimationFrame(rafId)
    resizeObserver.disconnect()
    document.removeEventListener('visibilitychange', handleVisibility)
    segmentGeo.dispose()
    jointGeo.dispose()
    material.dispose()
    renderer.dispose()
  }
}
