import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export interface Track {
  id: string
  title: string
  artists: string[]
  artist_id: string | null
  album: string | null
  album_id: string | null
  duration: number | null
  thumb: string | null
  cover: string | null
}

export interface TrackNote { liked: boolean; plays: number; last_played: number | null; skipped_recently: boolean }
export interface HistoryEntry { track: Track; played_at: number; outcome: 'completed' | 'skipped_early' | 'skipped_late' }

export interface LyricLine { at: number; text: string }
export interface Lyrics { lines: LyricLine[] | null; plain: string | null; source: string }

export interface PlaylistSummary { id: number; name: string; count: number; covers: string[] }
export interface Playlist { id: number; name: string; tracks: Track[] }

export type Quality = 'auto' | 'data_saver' | 'normal' | 'high'

export interface Rgb { r: number; g: number; b: number }
export interface Palette { dominant: Rgb; vivid: Rgb }

export type Update =
  | { type: 'now_playing'; track: Track; index: number }
  | { type: 'queue'; tracks: Track[]; index: number }
  | { type: 'position'; secs: number }
  | { type: 'duration'; secs: number }
  | { type: 'paused'; paused: boolean }
  | { type: 'timing'; video_id: string; ms: number; cached: boolean }
  | { type: 'error'; message: string }

export const api = {
  search: (query: string) => invoke<Track[]>('search', { query }),
  radio: (id: string) => invoke<Track[]>('radio', { id }),
  play: (tracks: Track[], index: number) => invoke<void>('play', { tracks, index }),
  playRadio: (track: Track) => invoke<void>('play_radio', { track }),
  next: () => invoke<void>('next'),
  previous: () => invoke<void>('previous'),
  jump: (index: number) => invoke<void>('jump', { index }),
  togglePause: () => invoke<void>('toggle_pause'),
  seek: (secs: number) => invoke<void>('seek', { secs }),
  setVolume: (volume: number) => invoke<void>('set_volume', { volume }),
  prefetch: (id: string) => invoke<void>('prefetch', { id }),
  setQuality: (quality: Quality) => invoke<void>('set_quality', { quality }),
  getQuality: () => invoke<Quality>('get_quality'),
  setAutoplay: (on: boolean) => invoke<void>('set_autoplay', { on }),
  queue: () => invoke<{ tracks: Track[]; index: number }>('queue'),
  setLiked: (track: Track, liked: boolean) => invoke<void>('set_liked', { track, liked }),
  isLiked: (id: string) => invoke<boolean>('is_liked', { id }),
  liked: () => invoke<Track[]>('liked'),
  history: (limit: number) => invoke<HistoryEntry[]>('history', { limit }),
  recent: (limit: number) => invoke<Track[]>('recent', { limit }),
  trackNotes: (ids: string[]) => invoke<Record<string, TrackNote>>('track_notes', { ids }),
  forgotten: (limit: number) => invoke<{ track: Track; last_played: number }[]>('forgotten', { limit }),
  quickPicks: (seed: string) => invoke<Track[]>('quick_picks', { seed }),
  lyrics: (track: Track) => invoke<Lyrics | null>('lyrics', { track }),
  playlists: () => invoke<PlaylistSummary[]>('playlists'),
  playlist: (id: number) => invoke<Playlist | null>('playlist', { id }),
  createPlaylist: (name: string, tracks: Track[] = []) => invoke<number>('create_playlist', { name, tracks }),
  renamePlaylist: (id: number, name: string) => invoke<void>('rename_playlist', { id, name }),
  deletePlaylist: (id: number) => invoke<void>('delete_playlist', { id }),
  addToPlaylist: (id: number, tracks: Track[]) => invoke<number>('add_to_playlist', { id, tracks }),
  removeFromPlaylist: (id: number, index: number) => invoke<void>('remove_from_playlist', { id, index }),
  moveInPlaylist: (id: number, from: number, to: number) => invoke<void>('move_in_playlist', { id, from, to }),
  coverPalette: (url: string) => invoke<Palette>('cover_palette', { url }),
}

export function onUpdate(cb: (u: Update) => void): Promise<UnlistenFn> {
  return listen<Update>('hum://update', (e) => cb(e.payload))
}

export const artistLine = (t: Track) => t.artists.join(', ')

export function fmtTime(secs: number | null | undefined): string {
  if (secs == null || !isFinite(secs)) return '–:––'
  const s = Math.max(0, Math.floor(secs))
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

// Cover art is served by the Rust side (disk cache). Windows and Android expose custom
// protocols as http://<scheme>.localhost, Linux and macOS as <scheme>://localhost.
const imgBase = /Windows|Android/.test(navigator.userAgent) ? 'http://img.localhost/' : 'img://localhost/'
export function img(url: string | null | undefined): string | undefined {
  return url ? imgBase + encodeURIComponent(url) : undefined
}
