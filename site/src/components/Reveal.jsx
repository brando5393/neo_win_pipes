import { useEffect, useRef, useState } from 'react'

function shouldSkipAnimation() {
  const prefersReducedMotion =
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  return prefersReducedMotion || typeof IntersectionObserver === 'undefined'
}

// Fades/slides children in the first time they scroll into view. No
// animation library needed — just IntersectionObserver — and it degrades
// to "just show it" for prefers-reduced-motion or a browser old enough to
// lack IntersectionObserver, decided once up front so the effect below
// only ever calls setState from the observer's own (already-async)
// callback, never synchronously on mount.
export default function Reveal({ children, as: Tag = 'div', className = '', delay = 0 }) {
  const ref = useRef(null)
  const [visible, setVisible] = useState(shouldSkipAnimation)

  useEffect(() => {
    if (visible) return undefined
    const el = ref.current
    if (!el) return undefined
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setVisible(true)
          observer.disconnect()
        }
      },
      { threshold: 0.15, rootMargin: '0px 0px -40px 0px' },
    )
    observer.observe(el)
    return () => observer.disconnect()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- runs once per mount; `visible` only ever flips true, never needs to re-arm.
  }, [])

  return (
    <Tag
      ref={ref}
      className={`transition-all duration-700 ease-out ${visible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-6'} ${className}`}
      style={{ transitionDelay: `${delay}ms` }}
    >
      {children}
    </Tag>
  )
}
