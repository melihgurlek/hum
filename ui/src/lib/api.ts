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
export interface Playlist { id: number; name: string; tracks: Track[]; synced: boolean }

export interface ArtistLink { name: string; id: string | null }
export interface AlbumCard { id: string; title: string; kind: string; year: number | null; artists: ArtistLink[]; thumb: string | null }
export interface ArtistPage { id: string; name: string; subscribers: string | null; banner: string | null; top: Track[]; albums: AlbumCard[]; singles: AlbumCard[] }
export type TopResult =
  | { kind: 'artist'; id: string; name: string; detail: string | null; thumb: string | null }
  | { kind: 'album'; id: string; title: string; detail: string | null; thumb: string | null }
export interface SearchResults { top: TopResult | null; songs: Track[] }
export interface AlbumPage { card: AlbumCard; cover: string | null; tracks: Track[] }
export type HomeCard =
  | ({ type: 'album' } & AlbumCard)
  | { type: 'playlist'; id: string; title: string; subtitle: string | null; thumb: string | null }
export interface HomeShelf { title: string; songs: Track[]; cards: HomeCard[] }
export interface QuickPicks { seeds: Track[]; picks: Track[] }

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
  | { type: 'error'; message: string; details: string | null }
  | { type: 'auto_quality'; low: boolean }

export const api = {
  search: (query: string) => invoke<SearchResults>('search', { query }),
  radio: (id: string) => invoke<Track[]>('radio', { id }),
  artist: (id: string) => invoke<ArtistPage>('artist', { id }),
  album: (id: string) => invoke<AlbumPage>('album', { id }),
  playArtistRadio: (id: string) => invoke<void>('play_artist_radio', { id }),
  play: (tracks: Track[], index: number) => invoke<void>('play', { tracks, index }),
  playRadio: (track: Track) => invoke<void>('play_radio', { track }),
  next: () => invoke<void>('next'),
  previous: () => invoke<void>('previous'),
  jump: (index: number) => invoke<void>('jump', { index }),
  togglePause: () => invoke<void>('toggle_pause'),
  restoreQueue: (tracks: Track[], index: number, position: number) => invoke<void>('restore_queue', { tracks, index, position }),
  seek: (secs: number) => invoke<void>('seek', { secs }),
  setVolume: (volume: number) => invoke<void>('set_volume', { volume }),
  prefetch: (id: string) => invoke<void>('prefetch', { id }),
  setQuality: (quality: Quality) => invoke<void>('set_quality', { quality }),
  getQuality: () => invoke<Quality>('get_quality'),
  setAutoplay: (on: boolean) => invoke<void>('set_autoplay', { on }),
  setNormalize: (on: boolean) => invoke<void>('set_normalize', { on }),
  setAccountRadio: (on: boolean) => invoke<void>('set_account_radio', { on }),
  queue: () => invoke<{ tracks: Track[]; index: number }>('queue'),
  setLiked: (track: Track, liked: boolean) => invoke<void>('set_liked', { track, liked }),
  isLiked: (id: string) => invoke<boolean>('is_liked', { id }),
  liked: () => invoke<Track[]>('liked'),
  history: (limit: number) => invoke<HistoryEntry[]>('history', { limit }),
  recent: (limit: number) => invoke<Track[]>('recent', { limit }),
  trackNotes: (ids: string[]) => invoke<Record<string, TrackNote>>('track_notes', { ids }),
  forgotten: (limit: number) => invoke<{ track: Track; last_played: number }[]>('forgotten', { limit }),
  quickPicks: () => invoke<QuickPicks>('quick_picks'),
  homeFeed: () => invoke<HomeShelf[]>('home_feed'),
  blockArtist: (track: Track) => invoke<void>('block_artist', { track }),
  unblockArtist: (key: string) => invoke<void>('unblock_artist', { key }),
  blockedArtists: () => invoke<{ key: string; name: string }[]>('blocked_artists'),
  ytPlaylist: (id: string) => invoke<Track[]>('yt_playlist', { id }),
  lyrics: (track: Track) => invoke<Lyrics | null>('lyrics', { track }),
  playlists: () => invoke<PlaylistSummary[]>('playlists'),
  playlist: (id: number) => invoke<Playlist | null>('playlist', { id }),
  createPlaylist: (name: string, tracks: Track[] = []) => invoke<number>('create_playlist', { name, tracks }),
  renamePlaylist: (id: number, name: string) => invoke<void>('rename_playlist', { id, name }),
  deletePlaylist: (id: number) => invoke<void>('delete_playlist', { id }),
  addToPlaylist: (id: number, tracks: Track[]) => invoke<number>('add_to_playlist', { id, tracks }),
  removeFromPlaylist: (id: number, index: number) => invoke<void>('remove_from_playlist', { id, index }),
  moveInPlaylist: (id: number, from: number, to: number) => invoke<void>('move_in_playlist', { id, from, to }),
  savedSongs: () => invoke<{ count: number; bytes: number }>('saved_songs'),
  clearSavedSongs: () => invoke<void>('clear_saved_songs'),
  dataUsed: () => invoke<{ month: number; total: number; auto_low: boolean }>('data_used'),
  coverPalette: (url: string) => invoke<Palette>('cover_palette', { url }),
  account: () => invoke<Profile | null>('account'),
  signIn: () => invoke<void>('sign_in'),
  signOut: () => invoke<void>('sign_out'),
  syncLikes: () => invoke<void>('sync_likes'),
  syncPlaylist: (id: number) => invoke<void>('sync_playlist', { id }),
  hidePlaylist: (id: number) => invoke<void>('hide_playlist', { id }),
  accountPlaylists: () => invoke<AccountPlaylist[]>('account_playlists'),
  setPlaylistShown: (id: string, name: string, shown: boolean) => invoke<void>('set_playlist_shown', { id, name, shown }),
}

export interface AccountPlaylist { id: string; name: string; count: number | null; shown: boolean }

/** The account changed likes or playlists here; or a sync failed. */
export function onSync(changed: () => void, failed: (msg: string) => void): Promise<UnlistenFn[]> {
  return Promise.all([listen('hum://library', () => changed()), listen<string>('hum://sync-error', (e) => failed(e.payload))])
}

export interface Profile { name: string; email: string | null; photo: string | null }

export function onAccount(cb: (p: Profile | null) => void, onError: (msg: string) => void): Promise<UnlistenFn[]> {
  return Promise.all([
    listen<Profile | null>('hum://account', (e) => cb(e.payload)),
    listen<string>('hum://account-error', (e) => onError(e.payload)),
  ])
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

/** A track's artists as links. Only the first artist's id is known for a track. */
export const artistLinks = (t: Track): ArtistLink[] => t.artists.map((name, i) => ({ name, id: i === 0 ? t.artist_id : null }))
