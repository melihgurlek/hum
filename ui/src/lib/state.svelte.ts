import { api, onSync, onUpdate, type AlbumPage, type ArtistPage, type HomeShelf, type QuickPicks, type SearchResults, type Palette, type PlaylistSummary, type Quality, type Repeat, type Track } from './api'

export type View = 'home' | 'search' | 'library' | 'settings' | 'playlist' | 'artist' | 'album'
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
  /** 0-100, the player bar's slider. */
  volume = $state(100)
  /** Wall-clock ms when `position` was last reported, for the progress bar's transition. */
  positionAt = $state(0)
  lastTiming = $state<{ ms: number; cached: boolean } | null>(null)
  palette = $state<Palette | null>(null)
  liked = $state(false)
  loading = $state(false)
  /** Auto quality stepped down to data saver because the connection stalled. */
  autoLow = $state(false)

  upNext = $derived(this.queue.slice(this.index + 1))
}

class Ui {
  view = $state<View>(load<View>('hum.view', 'home'))
  nowPlayingOpen = $state(false)
  /** Phone Now Playing: the Up next and lyrics sheet is pulled up over the player. */
  npSheet = $state(false)
  query = $state('')
  theme = $state<ThemePref>(load<ThemePref>('hum.theme', 'system'))
  // 'normal' was the same stream as 'high' and is gone from Settings.
  quality = $state<Quality>(load<Quality>('hum.quality', 'auto') === 'normal' ? 'high' : load<Quality>('hum.quality', 'auto'))
  autoplay = $state(load('hum.autoplay', true))
  repeat = $state<Repeat>(load<Repeat>('hum.repeat', 'off'))
  shuffle = $state(load('hum.shuffle', false))
  normalize = $state(load('hum.normalize', true))
  accountRadio = $state(load('hum.accountRadio', false))
  toast = $state<string | null>(null)
  /** A report the toast offers to copy (a song that wouldn't play). */
  toastDetails = $state<string | null>(null)
  playlistId = $state<number | null>(load<number | null>('hum.playlistId', null))
  artistId = $state<string | null>(load<string | null>('hum.artistId', null))
  albumId = $state<string | null>(load<string | null>('hum.albumId', null))
  /** Pages visited before this one, for Back (button, Alt+Left, mouse back). */
  history = $state<Place[]>([])
  playlists = $state<PlaylistSummary[]>([])
  libraryTab = $state<'liked' | 'history' | 'playlists'>('liked')
  /** The "Add to playlist" menu: which songs, and where to show it. */
  addMenu = $state<{ tracks: Track[]; x: number; y: number } | null>(null)
  /** Set right after creating a playlist from the sidebar, so its page focuses the name. */
  renameNext = $state(false)
  /** Recently played, newest first, from the library (plus the current song, optimistically). */
  recent = $state<Track[]>([])
  /** Bumped when the account sync changed likes or playlists, so open views reload. */
  synced = $state(0)
}

export const player = new Player()
export const ui = new Ui()

let toastTimer: ReturnType<typeof setTimeout> | undefined
export function showToast(msg: string, details: string | null = null) {
  ui.toast = msg
  ui.toastDetails = details
  clearTimeout(toastTimer)
  // Long enough to reach the Copy button.
  toastTimer = setTimeout(() => ((ui.toast = null), (ui.toastDetails = null)), details ? 10000 : 4000)
}

/** Copy the toast's report. The async clipboard API, then the old copy command as a fallback. */
export async function copyToastDetails() {
  const text = ui.toastDetails
  if (!text) return
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    const area = document.createElement('textarea')
    area.value = text
    document.body.append(area)
    area.select()
    document.execCommand('copy')
    area.remove()
  }
  showToast('Copied. Paste it into a bug report.')
}

interface Place { view: View; playlistId: number | null; artistId: string | null; albumId: string | null }
const here = (): Place => ({ view: ui.view, playlistId: ui.playlistId, artistId: ui.artistId, albumId: ui.albumId })

function show(p: Place) {
  ui.view = p.view
  ui.playlistId = p.playlistId
  ui.artistId = p.artistId
  ui.albumId = p.albumId
  ui.nowPlayingOpen = false
  ui.npSheet = false
  save('hum.view', p.view)
  save('hum.playlistId', p.playlistId)
  save('hum.artistId', p.artistId)
  save('hum.albumId', p.albumId)
}

function navigate(change: Partial<Place>) {
  const from = here()
  const to = { ...from, ...change }
  const same = to.view === from.view && to.playlistId === from.playlistId && to.artistId === from.artistId && to.albumId === from.albumId
  if (!same) {
    ui.history = [...ui.history.slice(-49), from]
    markBackStep()
  }
  show(to)
}

// Android's back gesture goes back in the WebView's own history, and with none left it closes
// the app. So each step hum can go back (a page, an open Now Playing) leaves a history entry,
// and going back through one closes Now Playing or returns to the previous page.
const android = typeof navigator !== 'undefined' && /Android/.test(navigator.userAgent)
export function markBackStep() {
  if (android) history.pushState(null, '')
}
// Closing from the screen (the chevron, a drag) goes back through those entries itself, so
// the gesture doesn't later spend a back on a step that's already closed.
let skipPops = 0
if (android) {
  addEventListener('popstate', () => {
    if (skipPops > 0) skipPops--
    else if (ui.npSheet) ui.npSheet = false
    else if (ui.nowPlayingOpen) ui.nowPlayingOpen = false
    else back()
  })
}
function unwind(steps: number) {
  if (!android || !steps) return
  skipPops += steps
  history.go(-steps)
}

/** True under 640px, where the phone layout (tab bar, mini player, carousels) takes over. */
const phoneQuery = typeof matchMedia !== 'undefined' ? matchMedia('(max-width: 640px)') : null
export const layout = $state({ phone: phoneQuery?.matches ?? false })
phoneQuery?.addEventListener('change', (e) => (layout.phone = e.matches))

/** Open Now Playing (a back step on Android). */
export function openNowPlaying() {
  if (ui.nowPlayingOpen) return
  ui.nowPlayingOpen = true
  markBackStep()
}

/** Close Now Playing from the screen, its Up next sheet with it. */
export function closeNowPlaying() {
  if (!ui.nowPlayingOpen) return
  unwind(1 + (ui.npSheet ? 1 : 0))
  ui.npSheet = false
  ui.nowPlayingOpen = false
}

/** Pull the Up next and lyrics sheet up over phone Now Playing, or put it away. */
export function setNpSheet(open: boolean) {
  if (open === ui.npSheet) return
  ui.npSheet = open
  if (open) markBackStep()
  else unwind(1)
}

export function go(view: View) {
  navigate({ view })
}

export function back() {
  const prev = ui.history.at(-1)
  if (!prev) return
  ui.history = ui.history.slice(0, -1)
  show(prev)
}

/** Set `key`, dropping the oldest entries past `cap` (Maps keep insertion order). */
export function capSet<K, V>(map: Map<K, V>, key: K, value: V, cap: number) {
  map.delete(key)
  map.set(key, value)
  while (map.size > cap) map.delete(map.keys().next().value as K)
}

// Session caches, so Back and revisits are instant. They live here, not in the views, because
// a view's own variables are thrown away whenever you navigate off it.
export const searchCache = new Map<string, SearchResults>()
export const quickPicksCache = new Map<string, QuickPicks>()
/** YouTube Music's home page, once per session. */
export const homeFeed = $state<{ shelves: HomeShelf[] | null }>({ shelves: null })
const artistCache = new Map<string, Promise<ArtistPage>>()
const albumCache = new Map<string, Promise<AlbumPage>>()
function cached<T>(cache: Map<string, Promise<T>>, id: string, fetch: (id: string) => Promise<T>): Promise<T> {
  let p = cache.get(id)
  if (!p) {
    p = fetch(id)
    p.catch(() => cache.delete(id))
    capSet(cache, id, p, 30)
  }
  return p
}
export const loadArtist = (id: string) => cached(artistCache, id, api.artist)
export const loadAlbum = (id: string) => cached(albumCache, id, api.album)

export function openArtist(id: string | null | undefined) {
  if (id) navigate({ view: 'artist', artistId: id })
}

/** An artist we only have the name of (a featured artist on an older saved song): find the
 *  page by searching for the name. */
export async function openArtistNamed(name: string) {
  try {
    const r = await api.search(name)
    const same = (n: string) => n.localeCompare(name, undefined, { sensitivity: 'base' }) === 0
    if (r.top?.kind === 'artist' && same(r.top.name)) return openArtist(r.top.id)
    for (const s of r.songs) {
      const i = s.artists.findIndex(same)
      const id = i >= 0 ? (s.artist_ids?.[i] ?? (i === 0 ? s.artist_id : null)) : null
      if (id) return openArtist(id)
    }
    showToast(`Couldn't find ${name}'s page`)
  } catch (e) {
    showToast(String(e))
  }
}
export function openAlbum(id: string | null | undefined) {
  if (id) navigate({ view: 'album', albumId: id })
}

export async function playAlbum(id: string, shuffle = false) {
  try {
    const a = await loadAlbum(id)
    if (a.tracks.length) playList(shuffle ? shuffled(a.tracks) : a.tracks, 0)
  } catch (e) {
    showToast(String(e))
  }
}

/** Keep `track`'s artist out of radio and quick picks; skips the song playing if it's theirs. */
export async function blockArtist(track: Track) {
  const name = track.artists[0] ?? track.title
  ui.addMenu = null
  try {
    await api.blockArtist(track)
    quickPicksCache.clear()
    const cur = player.current
    const same = (t: Track) => (track.artist_id ? t.artist_id === track.artist_id : t.artists[0]?.toLowerCase() === name.toLowerCase())
    if (cur && same(cur)) await api.next()
    showToast(`You won't hear ${name} in radio. Undo in Settings.`)
  } catch (e) {
    showToast(String(e))
  }
}

/** Play a YouTube Music playlist (Home cards have no page of their own). */
export async function playYtPlaylist(id: string) {
  player.loading = true
  try {
    const tracks = await api.ytPlaylist(id)
    if (tracks.length) playList(tracks, 0)
    else player.loading = false
  } catch (e) {
    player.loading = false
    showToast(String(e))
  }
}

export async function playArtistRadio(id: string, name: string) {
  player.loading = true
  try {
    await api.playArtistRadio(id)
  } catch (e) {
    player.loading = false
    showToast(String(e))
  }
}

export function shuffled<T>(list: T[]): T[] {
  const t = [...list]
  for (let i = t.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1))
    ;[t[i], t[j]] = [t[j], t[i]]
  }
  return t
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

export function setAccountRadio(on: boolean) {
  ui.accountRadio = on
  save('hum.accountRadio', on)
  api.setAccountRadio(on)
}

export function setAutoplay(on: boolean) {
  ui.autoplay = on
  save('hum.autoplay', on)
  api.setAutoplay(on)
  // Radio keeps adding songs, so a queue that repeats would never come round again.
  if (on && ui.repeat === 'all') setRepeat('off')
}

/** Shuffle mixes what's after the current song, and every queue you start; off restores. */
export function setShuffle(on: boolean) {
  ui.shuffle = on
  save('hum.shuffle', on)
  api.setShuffle(on)
}

/** Off, then the whole queue, then this song; the button cycles through them. */
export function setRepeat(r: Repeat) {
  ui.repeat = r
  save('hum.repeat', r)
  api.setRepeat(r)
  if (r === 'all' && ui.autoplay) setAutoplay(false)
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

export function setNormalize(on: boolean) {
  ui.normalize = on
  save('hum.normalize', on)
  api.setNormalize(on)
}

// Sleep timer: pause after some minutes or at the end of this song, fading out over the last
// 30 s (10 s for end of song), then put the volume back so the next play isn't silent.
export const sleep = $state<{ until: number | null; endOfSong: boolean }>({ until: null, endOfSong: false })
let sleepTick: ReturnType<typeof setInterval> | undefined
let fading = false

function stopSleep() {
  clearInterval(sleepTick)
  sleepTick = undefined
  sleep.until = null
  sleep.endOfSong = false
  if (fading) api.setVolume(player.volume)
  fading = false
}

/** Minutes, 'song' for the end of this song, or null to turn it off. */
export function setSleep(choice: number | 'song' | null) {
  stopSleep()
  if (choice === null) return
  if (choice === 'song') sleep.endOfSong = true
  else sleep.until = Date.now() + choice * 60_000
  sleepTick = setInterval(sleepStep, 500)
}

function sleepStep() {
  let left: number // seconds until the pause
  let fadeFor: number
  if (sleep.endOfSong) {
    if (!player.duration) return
    const pos = player.position + (player.paused ? 0 : (performance.now() - player.positionAt) / 1000)
    left = player.duration - pos - 1 // pause just before mpv moves on to the next song
    fadeFor = 10
  } else if (sleep.until) {
    left = (sleep.until - Date.now()) / 1000
    fadeFor = 30
  } else return
  if (left <= 0) {
    api.setPaused(true)
    stopSleep()
    showToast('Sleep timer: paused')
  } else if (left <= fadeFor && !player.paused) {
    fading = true
    api.setVolume(Math.round(player.volume * (left / fadeFor)))
  }
}

// The queue and where you were in it, saved as you listen and put back (paused) on launch.
interface Session { queue: Track[]; index: number; position: number }
let sessionSavedAt = 0
function saveSession(force = false) {
  if (!player.current || !player.queue.length) return
  const now = Date.now()
  if (!force && now - sessionSavedAt < 5000) return
  sessionSavedAt = now
  // Keep it small: what came just before and the next 200.
  const from = Math.max(0, player.index - 20)
  const queue = player.queue.slice(from, player.index + 200)
  save('hum.session', { queue, index: player.index - from, position: player.position } satisfies Session)
}

function restoreSession() {
  const s = load<Session | null>('hum.session', null)
  const track = s?.queue[s.index]
  if (!s || !track || player.current) return
  api.restoreQueue(s.queue, s.index, s.position).catch(() => {})
  player.queue = s.queue
  player.index = s.index
  player.current = track
  player.position = s.position
  player.positionAt = performance.now()
  player.duration = track.duration ?? 0
  player.paused = true
  player.loading = false
  api.isLiked(track.id).then((l) => (player.liked = l)).catch(() => {})
  if (track.thumb) {
    api
      .coverPalette(track.thumb)
      .then((p) => {
        if (player.current?.id === track.id) {
          player.palette = p
          applyPalette()
        }
      })
      .catch(() => {})
  }
}

export async function startListening() {
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme)
  applyTheme()
  api.setQuality(ui.quality)
  api.setAutoplay(ui.autoplay)
  api.setRepeat(ui.repeat)
  api.setShuffle(ui.shuffle)
  api.setNormalize(ui.normalize)
  api.setAccountRadio(ui.accountRadio)
  refreshRecent()
  refreshPlaylists()
  onSync(
    () => {
      ui.synced++
      refreshPlaylists()
    },
    showToast,
  )
  await onUpdate((u) => {
    if (u.type === 'now_playing' || u.type === 'queue') queueMicrotask(() => saveSession(true))
    else if (u.type === 'position' || u.type === 'paused') queueMicrotask(() => saveSession())
    switch (u.type) {
      case 'now_playing': {
        player.current = u.track
        player.index = u.index
        player.position = 0
        player.positionAt = performance.now()
        player.duration = u.track.duration ?? 0
        player.loading = true
        // Loading a song always unpauses mpv, and mpv only reports pause when it changes, so
        // the first song after startup would otherwise keep showing the play triangle.
        player.paused = false
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
      case 'auto_quality':
        player.autoLow = u.low
        if (u.low) showToast('Slow connection: switched to lower sound quality')
        break
      case 'error':
        showToast(u.message, u.details)
        player.loading = false
        break
    }
  })
  restoreSession()
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
/** `use:prefetchOnHover={id}`: resolve the song a button would start, while the pointer is on
    it. `null` (nothing to play yet) does nothing. */
export function prefetchOnHover(node: HTMLElement, id: string | null | undefined) {
  let current = id
  const enter = () => current && hoverPrefetch(current)
  node.addEventListener('mouseenter', enter)
  node.addEventListener('mouseleave', hoverCancel)
  return {
    update(next: string | null | undefined) {
      current = next
    },
    destroy() {
      node.removeEventListener('mouseenter', enter)
      node.removeEventListener('mouseleave', hoverCancel)
    },
  }
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
  navigate({ view: 'playlist', playlistId: id })
}

export function openLiked() {
  ui.libraryTab = 'liked'
  go('library')
}

export function openHistory() {
  ui.libraryTab = 'history'
  go('library')
}

// ---- recent searches ------------------------------------------------------

/** What you played or opened from search results, newest first. */
export type Recent =
  | { kind: 'song'; track: Track }
  | { kind: 'artist'; id: string; name: string; thumb: string | null }
  | { kind: 'album'; id: string; title: string; detail: string | null; thumb: string | null }
export const recentKey = (r: Recent) => (r.kind === 'song' ? 'song:' + r.track.id : `${r.kind}:${r.id}`)

// Entries saved as plain query strings by an earlier version are dropped.
export const searches = $state<{ list: Recent[] }>({
  list: load<unknown[]>('hum.searches', []).filter((r): r is Recent => typeof r === 'object' && r !== null),
})
export function rememberPick(r: Recent) {
  const key = recentKey(r)
  searches.list = [r, ...searches.list.filter((x) => recentKey(x) !== key)].slice(0, 12)
  save('hum.searches', searches.list)
}
export function forgetPick(key: string | null) {
  searches.list = key == null ? [] : searches.list.filter((x) => recentKey(x) !== key)
  save('hum.searches', searches.list)
}

export async function playArtistTop(id: string) {
  try {
    const a = await loadArtist(id)
    if (a.top.length) playList(a.top, 0)
  } catch (e) {
    showToast(String(e))
  }
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
    refreshPlaylists()
  } catch (e) {
    showToast(String(e))
  }
}
