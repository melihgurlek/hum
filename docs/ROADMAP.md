# hum roadmap

Updated in every PR. Newest done items at the top of each section.

## Done

| PR | What |
|---|---|
| #8 | Now Playing background picks the cover's real color, not tinted black (Relapse was green) |
| #7 | Synced lyrics: LRCLIB, YouTube Music fallback, cached; click a line to seek |
| #6 | Neutral default accent: all color comes from the current cover |
| #5 | Listening history, likes, taste-ranked radio and quick picks, "Haven't played these in a while", reasons in Up next, Library (Liked songs, History) |
| #4 | Button labels centered on their capital letters (were ~1.5 px low) |
| #3 | Media keys and OS media controls (MPRIS on Linux, SMTC on Windows) |
| #2 | Fix unstyled UI after dev-server restarts |
| #1 | First playable app: search, play, radio autoplay, Now Playing tinted by cover, Settings, cover-art cache, Auto quality |
| — | Spike: rustypipe broken against YouTube (2026-11), switched to ytmusic-rs; click→sound ~460 ms cold, ~140 ms prefetched |

## Next

1. **Playlists**: create, rename, add/remove/reorder songs, play.
2. **Artist and album pages**: top songs, albums, singles, artist radio; clicking an artist or album name anywhere opens them.
3. **Auto quality that reacts**: step down when mpv stalls for cache; "data used" counter in Settings.
4. **Windows build** in GitHub Actions (bundle libmpv DLL), smoke-tested.
5. **Google sign-in** (webview cookie login) to sync likes and playlists with MetroList.

## Later

- Song cache on disk (replays use no data), offline mode for cached songs.
- Crossfade, equalizer, skip silence, sleep timer.
- Discord status, Last.fm scrobbling.
- Mini player, tray menu.
- Android port: same `ui/` and `crates/core`, Media3 player plugin.

## Known issues

- Home recommendations start empty; they fill in as history builds up.
- Auto quality currently always picks the best stream (no stepping down yet).
- The dev app needs `~/.local/share/com.melihgurlek.hum/WebKitCache` cleared if the window
  ever shows unstyled after switching branches.
