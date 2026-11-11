import { api, onUpdate, type Palette, type PlaylistSummary, type Quality, type Track } from './api'

export type View = 'home' | 'search' | 'library' | 'settings' | 'playlist'
export type ThemePref = 'system' | 'dark' | 'light'

function load<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(key)
    return v ? (JSON.parse(v) as T) : fallback
  } catch {
    return fallback
  }
}
function save(key: string, v: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(v))
  } catch {
    /* storage unavailable: fine */
  }
}

class Player {
  current = $state<Track | null>(null)
  queue = $state<Track[]>([])
  index = $state(0)
  position = $state(0)
  duration = $state(0)
  paused = $state(true)
  /** Wall-clock ms when `position` was last reported, for the progress bar's transition. */
  positionAt = $state(0)
  lastTiming = $state<{ ms: number; cached: boolean } | null>(null)
  palette = $state<Palette | null>(null)
  liked = $state(false)
  loading = $state(false)

  upNext = $derived(this.queue.slice(this.index + 1))
}

class Ui {
  view = $state<View>(load<View>('hum.view', 'home'))
  nowPlayingOpen = $state(false)
  query = $state('')
  theme = $state<ThemePref>(load<ThemePref>('hum.theme', 'system'))
  quality = $state<Quality>(load<Quality>('hum.quality', 'auto'))
  autoplay = $state(load('hum.autoplay', true))
  toast = $state<string | null>(null)
  playlistId = $state<number | null>(load<number | null>('hum.playlistId', null))
  playlists = $state<PlaylistSummary[]>([])
  libraryTab = $state<'liked' | 'history'>('liked')
  /** The "Add to playlist" menu: which songs, and where to show it. */
  addMenu = $state<{ tracks: Track[]; x: number; y: number } | null>(null)
  /** Set right after creating a playlist from the sidebar, so its page focuses the name. */
  renameNext = $state(false)
  /** Recently played, newest first, from the library (plus the current song, optimistically). */
  recent = $state<Track[]>([])
}

export const player = new Player()
export const ui = new Ui()

let toastTimer: ReturnType<typeof setTimeout> | undefined
export function showToast(msg: string) {
  ui.toast = msg
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (ui.toast = null), 4000)
}

export function go(view: View) {
  ui.view = view
  ui.nowPlayingOpen = false
  save('hum.view', view)
}

export function setTheme(t: ThemePref) {
  ui.theme = t
  save('hum.theme', t)
  applyTheme()
}

export function setQuality(q: Quality) {
  ui.quality = q
  save('hum.quality', q)
  api.setQuality(q)
}

export function setAutoplay(on: boolean) {
  ui.autoplay = on
  save('hum.autoplay', on)
  api.setAutoplay(on)
}

function isDark(): boolean {
  if (ui.theme === 'system') return matchMedia('(prefers-color-scheme: dark)').matches
  return ui.theme === 'dark'
}

export function applyTheme() {
  document.documentElement.dataset.theme = isDark() ? 'dark' : 'light'
  applyPalette()
}

// ---- cover-derived colors -------------------------------------------------

type Hsl = [number, number, number]
function toHsl({ r, g, b }: { r: number; g: number; b: number }): Hsl {
  r /= 255; g /= 255; b /= 255
  const max = Math.max(r, g, b), min = Math.min(r, g, b)
  const l = (max + min) / 2
  if (max === min) return [0, 0, l]
  const d = max - min
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
  const h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4
  return [h * 60, s, l]
}
const hsl = ([h, s, l]: Hsl) => `hsl(${h.toFixed(0)} ${(s * 100).toFixed(0)}% ${(l * 100).toFixed(0)}%)`

function applyPalette() {
  const root = document.documentElement.style
  const p = player.palette
  if (!p) {
    root.removeProperty('--accent')
    root.removeProperty('--np-bg')
    root.removeProperty('--np-bg-2')
    return
  }
  const dark = isDark()
  const [vh, vs] = toHsl(p.vivid)
  const [dh, ds] = toHsl(p.dominant)
  // Accent: the cover's vivid hue, at a lightness that reads on the theme background.
  const accent: Hsl = [vh, Math.max(vs, 0.45), dark ? 0.66 : 0.4]
  // Now Playing: a deep (or pale) shade of the cover's dominant color.
  const bg: Hsl = dark ? [dh, Math.min(ds, 0.5), 0.17] : [dh, Math.min(ds, 0.45), 0.9]
  const bg2: Hsl = dark ? [dh, Math.min(ds, 0.5), 0.1] : [dh, Math.min(ds, 0.4), 0.82]
  // A colorless cover (black and white, grey) has no hue worth amplifying: stay neutral.
  if (vs < 0.18) root.removeProperty('--accent')
  else root.setProperty('--accent', hsl(accent))
  root.setProperty('--np-bg', hsl(bg))
  root.setProperty('--np-bg-2', hsl(bg2))
}

// ---- engine events --------------------------------------------------------

export async function startListening() {
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme)
  applyTheme()
  api.setQuality(ui.quality)
  api.setAutoplay(ui.autoplay)
  refreshRecent()
  refreshPlaylists()
  await onUpdate((u) => {
    switch (u.type) {
      case 'now_playing': {
        player.current = u.track
        player.index = u.index
        player.position = 0
        player.positionAt = performance.now()
        player.duration = u.track.duration ?? 0
        player.loading = true
        ui.recent = [u.track, ...ui.recent.filter((t) => t.id !== u.track.id)].slice(0, 30)
        player.liked = false
        setTimeout(refreshRecent, 1500)
        api.isLiked(u.track.id).then((l) => {
          if (player.current?.id === u.track.id) player.liked = l
        }).catch(() => {})
        const art = u.track.thumb
        if (art) {
          api
            .coverPalette(art)
            .then((p) => {
              if (player.current?.id === u.track.id) {
                player.palette = p
                applyPalette()
              }
            })
            .catch(() => {})
        }
        break
      }
      case 'queue':
        player.queue = u.tracks
        player.index = u.index
        break
      case 'position':
        player.position = u.secs
        player.positionAt = performance.now()
        player.loading = false
        break
      case 'duration':
        player.duration = u.secs
        break
      case 'paused':
        player.paused = u.paused
        break
      case 'timing':
        player.lastTiming = { ms: u.ms, cached: u.cached }
        player.loading = false
        break
      case 'error':
        showToast(u.message)
        player.loading = false
        break
    }
  })
}

/** Start a list playing from `index`, keeping the list as the queue. */
export async function playList(tracks: Track[], index: number) {
  player.queue = tracks
  player.index = index
  player.current = tracks[index]
  player.loading = true
  try {
    await api.play(tracks, index)
  } catch (e) {
    showToast(String(e))
  }
}

export async function playRadio(track: Track) {
  player.queue = [track]
  player.index = 0
  player.current = track
  player.loading = true
  try {
    await api.playRadio(track)
  } catch (e) {
    showToast(String(e))
  }
}

/** Resolve a stream ahead of a likely click. Skipped in data saver to avoid wasted requests. */
let hoverTimer: ReturnType<typeof setTimeout> | undefined
export function hoverPrefetch(id: string) {
  if (ui.quality === 'data_saver') return
  clearTimeout(hoverTimer)
  hoverTimer = setTimeout(() => api.prefetch(id), 60)
}
export function hoverCancel() {
  clearTimeout(hoverTimer)
}

export async function refreshRecent() {
  try {
    const fromDb = await api.recent(30)
    const cur = player.current
    ui.recent = cur ? [cur, ...fromDb.filter((t) => t.id !== cur.id)] : fromDb
  } catch {
    /* library unavailable: keep what we have */
  }
}

export async function toggleLike() {
  const t = player.current
  if (!t) return
  const next = !player.liked
  player.liked = next
  try {
    await api.setLiked(t, next)
    showToast(next ? 'Added to Liked songs' : 'Removed from Liked songs')
  } catch (e) {
    player.liked = !next
    showToast(String(e))
  }
}

const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']

/** Why a song is in Up next, in plain words. */
export function reason(n: import('./api').TrackNote | undefined): string {
  if (!n || n.plays === 0) return n?.liked ? 'You like this' : 'New to you'
  if (n.liked) return 'You like this'
  const days = n.last_played ? (Date.now() / 1000 - n.last_played) / 86400 : 0
  if (days > 30 && n.last_played) return `Not since ${MONTHS[new Date(n.last_played * 1000).getMonth()]}`
  if (n.plays >= 3) return `Played ${n.plays} times`
  return 'Played before'
}

// ---- playlists ------------------------------------------------------------

export async function refreshPlaylists() {
  try {
    ui.playlists = await api.playlists()
  } catch {
    /* library unavailable */
  }
}

export function openPlaylist(id: number) {
  ui.playlistId = id
  save('hum.playlistId', id)
  go('playlist')
}

export function openLiked() {
  ui.libraryTab = 'liked'
  go('library')
}

export async function newPlaylistFromSidebar() {
  const id = await api.createPlaylist('New playlist')
  await refreshPlaylists()
  ui.renameNext = true
  openPlaylist(id)
}

export function openAddMenu(tracks: Track[], e: MouseEvent) {
  e.preventDefault()
  e.stopPropagation()
  const r = (e.currentTarget as HTMLElement | null)?.getBoundingClientRect()
  // Right-click opens at the pointer; the + button opens under itself.
  const x = e.type === 'contextmenu' || !r ? e.clientX : r.right
  const y = e.type === 'contextmenu' || !r ? e.clientY : r.bottom + 4
  ui.addMenu = { tracks, x, y }
}

export async function addToPlaylist(id: number, tracks: Track[]) {
  ui.addMenu = null
  const name = ui.playlists.find((p) => p.id === id)?.name ?? 'playlist'
  try {
    const added = await api.addToPlaylist(id, tracks)
    showToast(added ? `Added to ${name}` : `Already in ${name}`)
    refreshPlaylists()
  } catch (e) {
    showToast(String(e))
  }
}

export async function addToNewPlaylist(name: string, tracks: Track[]) {
  ui.addMenu = null
  try {
    await api.createPlaylist(name, tracks)
    showToast(`Created ${name.trim() || 'New playlist'}`)
    refreshPlaylists()
  } catch (e) {
    showToast(String(e))
  }
}
