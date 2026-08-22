import { useEffect, useState } from 'react'
import { AppWindow, Package, PackageOpen, ArrowDownToLine } from 'lucide-react'

const REPO = 'brando5393/neo_win_pipes'
const RELEASES_PAGE = `https://github.com/${REPO}/releases/latest`

// The .msi's filename never changes across releases, so it could be a
// static releases/latest/download/ link — but the .deb and AppImage both
// embed the version in their filename (e.g.
// pipes-xscreensaver_0.3.0_amd64.deb), so a hardcoded link would 404 the
// moment a new version ships. Fetching the real release and matching by
// extension keeps all three correct with zero rebuilds required.
const MATCHERS = [
  { id: 'windows', label: 'Windows', sub: '.msi installer', icon: AppWindow, match: (name) => name.endsWith('.msi') },
  { id: 'linux-deb', label: 'Linux (.deb)', sub: 'Debian / Ubuntu', icon: Package, match: (name) => name.endsWith('.deb') },
  { id: 'linux-appimage', label: 'Linux (AppImage)', sub: 'Pipes Settings only', icon: PackageOpen, match: (name) => name.endsWith('.AppImage') },
]

export default function DownloadButtons() {
  const [assets, setAssets] = useState(null)
  const [version, setVersion] = useState(null)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    let cancelled = false
    fetch(`https://api.github.com/repos/${REPO}/releases/latest`)
      .then((res) => {
        if (!res.ok) throw new Error(`GitHub API returned ${res.status}`)
        return res.json()
      })
      .then((release) => {
        if (cancelled) return
        setVersion(release.tag_name)
        setAssets(release.assets ?? [])
      })
      .catch(() => {
        if (!cancelled) setFailed(true)
      })
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <div className="flex flex-col items-center gap-4">
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 w-full max-w-3xl">
        {MATCHERS.map(({ id, label, sub, icon: Icon, match }) => {
          const asset = assets?.find((a) => match(a.name))
          const href = asset ? asset.browser_download_url : RELEASES_PAGE
          return (
            <a
              key={id}
              href={href}
              className="group flex flex-col items-center justify-center gap-1 rounded-xl border border-white/10 bg-white/5 px-6 py-5 text-center transition hover:border-cyan-400/50 hover:bg-white/10"
            >
              <Icon className="h-7 w-7 text-cyan-300" strokeWidth={1.5} />
              <span className="mt-2 flex items-center gap-1.5 text-lg font-semibold text-white">
                {label}
                <ArrowDownToLine className="h-4 w-4 text-slate-500 transition group-hover:text-cyan-300" strokeWidth={2} />
              </span>
              <span className="text-sm text-slate-400">{sub}</span>
            </a>
          )
        })}
      </div>
      <p className="text-sm text-slate-500">
        {failed
          ? 'Could not reach GitHub to find the latest build — '
          : version
            ? `Latest release: ${version} — `
            : 'Looking up the latest release… '}
        <a href={RELEASES_PAGE} className="underline hover:text-slate-300">
          see all releases
        </a>
      </p>
    </div>
  )
}
