import { Workflow, Palette, Sparkles, SlidersHorizontal, Bug, Monitor, Gem, CheckCircle2, AlertTriangle, MinusCircle, GitFork, BookOpen, Tag, Download } from 'lucide-react'
import DownloadButtons from './components/DownloadButtons'
import Checksums from './components/Checksums'
import PipesHero from './components/PipesHero'
import Reveal from './components/Reveal'
import pipesSettings from './assets/pipes-settings.png'

const REPO = 'brando5393/neo_win_pipes'

const FEATURES = [
  {
    icon: Workflow,
    title: 'Procedural pipe growth',
    body: 'Colored pipes grow through a 3D grid one segment at a time, turning at random joints, until the scene fills up and dissolves away to start again.',
  },
  {
    icon: Palette,
    title: 'Themes, one click',
    body: '"Classic \'96", "Neon", and "Monochrome" bundle a whole look — palette, pipe style, and speed — together. Every setting underneath is still tunable by hand.',
  },
  {
    icon: Sparkles,
    title: 'The teapot easter egg',
    body: "A rare, separate roll occasionally renders a procedural Utah teapot at a joint instead of the usual ball or elbow — an honest nod to the original screensaver's own hidden teapot, not a pixel-exact recreation.",
  },
  {
    icon: SlidersHorizontal,
    title: 'Live-preview settings',
    body: 'Pipes Settings shows the simulation running right next to every slider — pipe count, speed, style, palette, grid size — so you see exactly what you’re about to set as your screensaver.',
  },
  {
    icon: Bug,
    title: 'Report a bug in two clicks',
    body: 'A "Report Issue / Feedback…" button right in the settings drawer opens a pre-filled GitHub issue — category, title, description, and (optionally) recent log output, with your home directory path redacted first.',
  },
  {
    icon: Monitor,
    title: 'Multi-monitor, done properly',
    body: 'Each screen gets its own independent scene by default — not a mirrored copy. Prefer one continuous scene instead? A "One big screen" mode shares a single simulation across every display, with pipes visually traveling from one monitor onto the next.',
  },
  {
    icon: Gem,
    title: 'A real chrome material',
    body: 'Pipes reflect a procedural sky environment, not just a flat specular highlight — genuinely shiny, tinted by each pipe\'s own color rather than washing it out to plain grey.',
  },
]

const STATUS = [
  { platform: 'Windows', state: 'Shipped', detail: 'Real .msi installer, auto-update checker', tone: 'good' },
  { platform: 'Linux', state: 'Builds, unverified visually', detail: 'Real X11 rendering + .deb/AppImage, compiled clean on CI — not yet watched on a real display', tone: 'mid' },
  { platform: 'macOS', state: 'Not started', detail: 'No .saver bundle yet', tone: 'none' },
]

const toneClasses = {
  good: 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30',
  mid: 'bg-amber-500/15 text-amber-300 border-amber-500/30',
  none: 'bg-slate-500/15 text-slate-400 border-slate-500/30',
}

const toneIcons = {
  good: CheckCircle2,
  mid: AlertTriangle,
  none: MinusCircle,
}

export default function App() {
  return (
    <div className="min-h-screen bg-[#0b0d12] text-slate-200">
      {/* Hero */}
      <header className="relative overflow-hidden">
        <PipesHero
          alt="neo_win_pipes screensaver running fullscreen, showing multicolored pipes and two teapot easter eggs"
          className="absolute inset-0 h-full w-full object-cover"
        />
        <div className="absolute inset-0 bg-gradient-to-b from-[#0b0d12]/40 via-[#0b0d12]/70 to-[#0b0d12]" />
        <div className="relative mx-auto max-w-4xl px-6 py-28 text-center sm:py-36">
          <a
            href={`https://github.com/${REPO}`}
            className="inline-flex items-center gap-1.5 rounded-full border border-white/10 bg-white/5 px-3 py-1 text-xs font-medium text-slate-300 transition hover:border-cyan-400/40 hover:text-white"
          >
            <GitFork className="h-3.5 w-3.5" strokeWidth={1.75} />
            Open source on GitHub
          </a>
          <h1 className="mt-6 text-4xl font-bold tracking-tight text-white sm:text-6xl">neo_win_pipes</h1>
          <p className="mt-4 text-lg text-slate-300 sm:text-xl">
            A cross-platform recreation of the classic Windows 3D Pipes screensaver, in Rust.
          </p>
          <div className="mt-10">
            <DownloadButtons />
          </div>
        </div>
      </header>

      {/* Platform status */}
      <section className="mx-auto max-w-4xl px-6 py-10">
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
          {STATUS.map((s, i) => {
            const ToneIcon = toneIcons[s.tone]
            return (
              <Reveal key={s.platform} delay={i * 80}>
                <div className={`rounded-lg border px-4 py-3 ${toneClasses[s.tone]}`}>
                  <div className="flex items-center justify-between">
                    <span className="font-semibold text-white">{s.platform}</span>
                    <span className="flex items-center gap-1 text-xs font-medium uppercase tracking-wide">
                      <ToneIcon className="h-3.5 w-3.5" strokeWidth={2.5} />
                      {s.state}
                    </span>
                  </div>
                  <p className="mt-1 text-xs opacity-80">{s.detail}</p>
                </div>
              </Reveal>
            )
          })}
        </div>
      </section>

      {/* Features */}
      <section className="mx-auto max-w-5xl px-6 py-16">
        <Reveal>
          <h2 className="text-center text-2xl font-bold text-white sm:text-3xl">What it does</h2>
        </Reveal>
        <div className="mt-10 grid grid-cols-1 gap-8 sm:grid-cols-2">
          {FEATURES.map((f, i) => (
            <Reveal key={f.title} delay={i * 80}>
              <div className="flex gap-4">
                <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-cyan-400/20 bg-cyan-400/10 text-cyan-300">
                  <f.icon className="h-5 w-5" strokeWidth={1.75} />
                </div>
                <div>
                  <h3 className="text-lg font-semibold text-white">{f.title}</h3>
                  <p className="mt-2 text-slate-400">{f.body}</p>
                </div>
              </div>
            </Reveal>
          ))}
        </div>
      </section>

      {/* Screenshot: settings app */}
      <section className="mx-auto max-w-6xl px-6 py-16">
        <Reveal>
          <h2 className="text-center text-2xl font-bold text-white sm:text-3xl">Pipes Settings</h2>
          <p className="mx-auto mt-3 max-w-2xl text-center text-slate-400">
            A live 3D preview sits right next to the settings drawer — pipe style and count, speed and camera,
            color palette, grid size and reset behavior, and the teapot toggle — all changes apply immediately.
          </p>
        </Reveal>
        <Reveal delay={120}>
          <div className="mt-10 overflow-hidden rounded-xl border border-white/10 shadow-2xl shadow-black/50">
            <img src={pipesSettings} alt="Pipes Settings app showing the live 3D preview and settings drawer" className="w-full" />
          </div>
        </Reveal>
      </section>

      {/* Download, repeated at the bottom for anyone who scrolled past the hero */}
      <Reveal as="section" className="mx-auto max-w-4xl px-6 py-16 text-center">
        <h2 className="flex items-center justify-center gap-2 text-2xl font-bold text-white sm:text-3xl">
          <Download className="h-6 w-6 text-cyan-300" strokeWidth={1.75} />
          Get it
        </h2>
        <div className="mt-8">
          <DownloadButtons />
        </div>
        <Checksums />
      </Reveal>

      {/* Footer */}
      <footer className="border-t border-white/10 px-6 py-10 text-center text-sm text-slate-500">
        <div className="flex flex-wrap items-center justify-center gap-x-6 gap-y-2">
          <a href={`https://github.com/${REPO}`} className="flex items-center gap-1.5 hover:text-slate-300">
            <GitFork className="h-4 w-4" strokeWidth={1.75} />
            GitHub repository
          </a>
          <a href={`https://github.com/${REPO}/wiki`} className="flex items-center gap-1.5 hover:text-slate-300">
            <BookOpen className="h-4 w-4" strokeWidth={1.75} />
            Wiki
          </a>
          <a href={`https://github.com/${REPO}/issues/new`} className="flex items-center gap-1.5 hover:text-slate-300">
            <Bug className="h-4 w-4" strokeWidth={1.75} />
            Report an issue
          </a>
          <a href={`https://github.com/${REPO}/releases`} className="flex items-center gap-1.5 hover:text-slate-300">
            <Tag className="h-4 w-4" strokeWidth={1.75} />
            All releases
          </a>
        </div>
        <p className="mt-6">MIT licensed. Not affiliated with Microsoft.</p>
      </footer>
    </div>
  )
}
