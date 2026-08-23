import { useEffect, useState } from 'react'
import { ShieldCheck, Copy, Check } from 'lucide-react'

const REPO = 'brando5393/neo_win_pipes'

// GitHub computes and serves a SHA-256 digest for every release asset
// itself (the `digest` field on each asset from the Releases API) — so
// this reads straight from there instead of us maintaining our own
// checksums file. It's automatically correct for whatever the latest
// release actually is, with nothing to regenerate or keep in sync by hand.
function stripPrefix(digest) {
  return digest?.startsWith('sha256:') ? digest.slice('sha256:'.length) : digest
}

function CopyableHash({ value }) {
  const [copied, setCopied] = useState(false)

  return (
    <button
      type="button"
      onClick={() => {
        navigator.clipboard.writeText(value).then(() => {
          setCopied(true)
          setTimeout(() => setCopied(false), 1500)
        })
      }}
      className="group flex w-full items-center gap-2 overflow-x-auto rounded-md border border-white/10 bg-black/30 px-3 py-2 text-left font-mono text-xs text-slate-300 transition hover:border-cyan-400/30 hover:text-white"
      title="Copy to clipboard"
    >
      <span className="flex-1 whitespace-nowrap">{value}</span>
      {copied ? (
        <Check className="h-3.5 w-3.5 shrink-0 text-emerald-400" strokeWidth={2} />
      ) : (
        <Copy className="h-3.5 w-3.5 shrink-0 text-slate-500 group-hover:text-cyan-300" strokeWidth={2} />
      )}
    </button>
  )
}

export default function Checksums() {
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
        setAssets((release.assets ?? []).filter((a) => a.digest))
      })
      .catch(() => {
        if (!cancelled) setFailed(true)
      })
    return () => {
      cancelled = true
    }
  }, [])

  if (failed || (assets && assets.length === 0)) return null

  return (
    <div className="mt-10 overflow-hidden rounded-xl border border-white/10 bg-white/5 p-6">
      <div className="flex items-center gap-2">
        <ShieldCheck className="h-5 w-5 text-emerald-400" strokeWidth={1.75} />
        <h3 className="text-lg font-semibold text-white">Verify your download</h3>
      </div>
      <p className="mt-2 text-sm text-slate-400">
        SHA-256 checksums for {version ? <span className="text-slate-300">{version}</span> : 'the latest release'},
        served directly by GitHub for each file — not hand-maintained here, so they can't go stale.
      </p>
      <div className="mt-4 flex flex-col gap-3">
        {assets === null && !failed && <p className="text-sm text-slate-500">Looking up the latest release…</p>}
        {assets?.map((asset) => (
          <div key={asset.name}>
            <p className="mb-1 text-sm font-medium text-slate-300">{asset.name}</p>
            <CopyableHash value={stripPrefix(asset.digest)} />
          </div>
        ))}
      </div>
    </div>
  )
}
