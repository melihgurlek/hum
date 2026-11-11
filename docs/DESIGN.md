# hum — design spec

Mockups: https://claude.ai/artifact/1UkBj2AmDwHZ8xpimFGr6i (round 1 A/B/C are the base) and the Claude
Design export (`hum — music player UI.pdf`, directions 1 and 2) for borrowed pieces.

## Screens

| Screen | Base | Borrowed |
|---|---|---|
| Home | Round 1A: sidebar (library + playlists), search field, shelves, bottom player bar | "Keep going with <artist> radio" primary button; **Your stations** cards tinted from their seed cover; "Haven't played these in a while" side list with "Last played in May" |
| Now Playing | Round 1B: big cover left, title/artist/progress/controls; right panel with Lyrics / Up next / Related tabs | Background + accent derived **live from the album cover** (never a fixed color); Up next rows show **why** ("You like", "New to you", "Not since May", "Similar"); Autoplay radio toggle |
| Search | Round 1C as-is: filter chips, Top result card with Play + Radio, Songs list, Albums grid | — |
| Artist / Album / Playlist | Follow Home's card and row styles | — |
| Library | Liked songs, playlists, history | — |
| Settings | One page: theme, sound quality (Auto / Data saver / Normal / High), data used, cache size | — |

## Themes

Both, from one token set, following the OS (`prefers-color-scheme`), overridable in Settings.

| Token | Dark (round 1A) | Light (round 1C) |
|---|---|---|
| `--bg` | `#121214` | `#f6f5f2` |
| `--surface` | `#1b1b1f` | `#ffffff` |
| `--raised` | `#1f1f24` | `#ecebe6` |
| `--line` | `#2e2e35` | `#dcdbd5` |
| `--text` | `#f2f2f3` | `#17171a` |
| `--muted` | `#9a9aa4` | `#5f5f68` |
| `--accent` | from current cover | from current cover |

Default accent when nothing is playing is neutral (dark `#ededef`, light `#1d1d21`): hum has no
color of its own, all color comes from the current cover.

## Cover-derived color

- Extract a palette from the current cover's small thumbnail (≤ 64 px) in Rust, off the UI thread.
  Cache per album id in SQLite.
- `--accent`: the most saturated swatch, adjusted for 4.5:1 contrast against `--bg`.
- Now Playing background: a dark (or light, in light theme) shade of the dominant swatch, as a
  flat color plus one static radial gradient. No blur, no animation.
- Change colors with a 400 ms CSS transition on the custom properties' consumers (not on `:root`
  restyles of the whole tree: set the vars on the Now Playing container and the player bar only).

## Rules

- Fonts: Plus Jakarta Sans (UI). Bundled locally, no network.
- Touch targets ≥ 44 px, visible keyboard focus.
- Performance budget from `~/.claude/plans/i-want-you-to-noble-shamir.md`: virtualized lists,
  thumbnails at display size, no `backdrop-filter`, nothing repaints while paused.
