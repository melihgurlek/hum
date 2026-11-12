/**
 * Touch ripples, as Android apps have: a circle grows from the finger over whatever was
 * pressed, clipped to its shape, and fades when the finger lifts.
 *
 * Started on pointerdown rather than through CSS :active, which the WebView holds back while it
 * decides between a tap and a scroll (the press looked late). A scroll cancels it. The ripple is
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
  // A quick tap still lets the circle finish growing, so it is seen at all.
  const wait = fast ? 0 : Math.max(0, 200 - (performance.now() - r.born))
  setTimeout(() => {
    r.dot.style.transition = `opacity ${fast ? 100 : 250}ms ease-out, transform 200ms ease-out`
    r.dot.style.opacity = '0'
    setTimeout(() => r.clip.remove(), 300)
  }, wait)
}

function down(e: PointerEvent) {
  if (e.pointerType !== 'touch' || !e.isPrimary) return
  const hit = (e.target as Element | null)?.closest?.(SELECTOR) as HTMLElement | null
  if (!hit || (hit as HTMLButtonElement).disabled || hit.closest('[data-no-ripple]')) return
  release(true)
  const el = surface(hit)
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
  const x = e.clientX - box.left
  const y = e.clientY - box.top
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
  live = { clip, dot, x: e.clientX, y: e.clientY, born: performance.now() }
  requestAnimationFrame(() => (dot.style.transform = 'scale(1)'))
}

function move(e: PointerEvent) {
  if (live && Math.hypot(e.clientX - live.x, e.clientY - live.y) > SLOP) release(true)
}

export function ripples() {
  addEventListener('pointerdown', down, { passive: true, capture: true })
  addEventListener('pointermove', move, { passive: true, capture: true })
  addEventListener('pointerup', () => release(), { passive: true, capture: true })
  addEventListener('pointercancel', () => release(true), { passive: true, capture: true })
  // A scroll inside a list doesn't always cancel the pointer first.
  addEventListener('scroll', () => release(true), { passive: true, capture: true })
}
