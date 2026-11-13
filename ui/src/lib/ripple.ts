/**
 * Touch ripples, as Android apps have: a circle grows from the finger over whatever was
 * pressed, clipped to its shape, and fades when the finger lifts.
 *
 * Started on pointerdown rather than through CSS :active, which the WebView holds back while it
 * decides between a tap and a scroll (the press looked late). A scroll cancels it. Inside a
 * scrolling list it waits 100 ms first, as Android does, so a touch that turns into a scroll
 * shows none (the ripple stays where it was drawn while the list moves away). The ripple is
 * drawn in a fixed layer over the page, so no control needs `position` or `overflow` for it.
 * Buttons stretched over a row or card through an absolute ::after ripple over that row.
 */

const SELECTOR = 'button, a[href], [role="button"], summary, label:has(input)'
const SLOP = 10 // px of finger travel that makes it a scroll, not a press

interface Live {
  clip: HTMLElement
  dot: HTMLElement
  x: number
  y: number
  born: number
}

let layer: HTMLElement | null = null
let live: Live | null = null
// A press inside a scrolling list, waiting to see whether it becomes a scroll.
let pending: { timer: number; start: () => void } | null = null
const TAP_TIMEOUT = 100 // ms, Android's ViewConfiguration.getTapTimeout()

function inScroller(el: HTMLElement): boolean {
  for (let p = el.parentElement; p && p !== document.body; p = p.parentElement) {
    const s = getComputedStyle(p)
    if (/(auto|scroll)/.test(s.overflowY) && p.scrollHeight > p.clientHeight) return true
    if (/(auto|scroll)/.test(s.overflowX) && p.scrollWidth > p.clientWidth) return true
  }
  return false
}

function cancel() {
  if (pending) clearTimeout(pending.timer)
  pending = null
  release(true)
}

function surface(el: HTMLElement): HTMLElement {
  const after = getComputedStyle(el, '::after')
  if (after.content !== 'none' && after.position === 'absolute' && el.offsetParent instanceof HTMLElement) {
    return el.offsetParent
  }
  return el
}

function release(fast = false) {
  const r = live
  if (!r) return
  live = null
  // Cut off by a scroll: gone at once, not left fading where the row used to be.
  if (fast) {
    r.clip.remove()
    return
  }
  // A quick tap still lets the circle finish growing, so it is seen at all.
  const wait = Math.max(0, 200 - (performance.now() - r.born))
  setTimeout(() => {
    r.dot.style.transition = 'opacity 250ms ease-out, transform 200ms ease-out'
    r.dot.style.opacity = '0'
    setTimeout(() => r.clip.remove(), 300)
  }, wait)
}

function down(e: PointerEvent) {
  if (e.pointerType !== 'touch' || !e.isPrimary) return
  const hit = (e.target as Element | null)?.closest?.(SELECTOR) as HTMLElement | null
  if (!hit || (hit as HTMLButtonElement).disabled || hit.closest('[data-no-ripple]')) return
  cancel()
  const el = surface(hit)
  const at = { x: e.clientX, y: e.clientY }
  const start = () => show(hit, el, at)
  if (inScroller(el)) pending = { timer: window.setTimeout(() => ((pending = null), start()), TAP_TIMEOUT), start }
  else start()
}

function show(hit: HTMLElement, el: HTMLElement, at: { x: number; y: number }) {
  const box = el.getBoundingClientRect()
  if (!box.width || !box.height) return
  const style = getComputedStyle(el)
  let radius = el === hit ? style.borderRadius : getComputedStyle(hit, '::after').borderRadius
  // A square-cornered control (a text link, a tab) gets soft corners: a hard-edged box looked
  // like a selection, not a press.
  if (parseFloat(radius) === 0) radius = '10px'

  if (!layer) {
    layer = document.createElement('div')
    layer.setAttribute('aria-hidden', 'true')
    Object.assign(layer.style, { position: 'fixed', inset: '0', pointerEvents: 'none', zIndex: '2147483647' })
    document.body.append(layer)
  }
  const clip = document.createElement('div')
  Object.assign(clip.style, {
    position: 'absolute',
    left: `${box.left}px`,
    top: `${box.top}px`,
    width: `${box.width}px`,
    height: `${box.height}px`,
    borderRadius: radius,
    overflow: 'hidden',
  })
  // Big enough to reach the farthest corner from the finger.
  const x = at.x - box.left
  const y = at.y - box.top
  const size = 2 * Math.hypot(Math.max(x, box.width - x), Math.max(y, box.height - y))
  const dot = document.createElement('div')
  Object.assign(dot.style, {
    position: 'absolute',
    left: `${x - size / 2}px`,
    top: `${y - size / 2}px`,
    width: `${size}px`,
    height: `${size}px`,
    borderRadius: '50%',
    background: getComputedStyle(hit).color,
    opacity: '0.2',
    transform: 'scale(0.1)',
    transition: 'transform 350ms cubic-bezier(0.2, 0, 0, 1)',
  })
  clip.append(dot)
  layer.append(clip)
  live = { clip, dot, x: at.x, y: at.y, born: performance.now() }
  requestAnimationFrame(() => (dot.style.transform = 'scale(1)'))
}

let downAt = { x: 0, y: 0 }
function move(e: PointerEvent) {
  if (!live && !pending) return
  if (Math.hypot(e.clientX - downAt.x, e.clientY - downAt.y) > SLOP) cancel()
}

// A tap shorter than the wait still gets its ripple, drawn as the finger lifts.
function up() {
  if (pending) {
    clearTimeout(pending.timer)
    const { start } = pending
    pending = null
    start()
  }
  release()
}

export function ripples() {
  addEventListener('pointerdown', (e) => ((downAt = { x: e.clientX, y: e.clientY }), down(e)), { passive: true, capture: true })
  addEventListener('pointermove', move, { passive: true, capture: true })
  addEventListener('pointerup', up, { passive: true, capture: true })
  addEventListener('pointercancel', cancel, { passive: true, capture: true })
  // A scroll inside a list doesn't always cancel the pointer first.
  addEventListener('scroll', cancel, { passive: true, capture: true })
}
