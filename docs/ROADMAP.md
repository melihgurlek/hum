# hum roadmap

Updated in every PR. Newest done items at the top of each section.

## Done

| PR | What |
|---|---|
| #22 | A song played from search continues with its own radio, not the other search results |
| #21 | Now Playing fits the window: lyrics panel no longer runs under the player bar, cover shrinks in short windows |
| #20 | Player bar takes the cover tint while Now Playing is open; sidebar stays neutral |
| #19 | Design cleanup: equalizer marks the playing song (the white default accent hid it), search border quiet until focused; dropped Home greeting and fake mood chips, hint text, confirmation toasts and the timing stat; heading scale, three corner sizes, one filled button per screen |
| #18 | Search top result follows YouTube Music: an artist card ("justin") or album card ("radiohead ok computer"), else the first song. Recent searches are the songs, artists and albums you picked, one click to replay or open |
| #16 | Auto steps back up only when a recent download ran at 3x what High needs, and waits twice as long after each step-down (15, 30, 60, 120 min), so a borderline connection settles instead of stuttering every 15 minutes |
| #15 | Quality changes apply to the playing song at once (same spot); Back button stays in view while scrolling; History in the sidebar; recent searches; spacing fixes (row durations, Now Playing album line) |
| #14 | Auto quality reacts: after two stalls in two minutes it switches to Data saver (~50 kbps), reloading the current song at the same spot; back to High after 15 calm minutes. "Data used" (this month, in all) in Settings. Removed "Normal", which was the same stream as High |
| #13 | Play icon right on the first song after startup; search field text centered |
| #12 | Artist and album pages: top songs, albums, singles, the artist's own Radio (YouTube Music's Mix); artist names and albums are links everywhere (rows, player bar, Now Playing, search); Back button, Alt+Left and mouse back |
| #10 | Playlists: create from the sidebar or the add menu, inline rename, drag to reorder, remove, delete; add songs with the hover +, right-click, or from Now Playing |
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

1. **Windows build** in GitHub Actions (bundle libmpv DLL), smoke-tested.
2. **Google sign-in** (webview cookie login) to sync likes and playlists with MetroList.

## Later

- Song cache on disk (replays use no data), offline mode for cached songs.
- Crossfade, equalizer, skip silence, sleep timer.
- Discord status, Last.fm scrobbling.
- Mini player, tray menu.
- Android port: same `ui/` and `crates/core`, Media3 player plugin.

## Known issues

- Artist top songs have no durations until played (YouTube Music doesn't list them there).
- Only a song's first artist links to a page; YouTube Music's song lists carry one artist id.
- Home starts nearly empty (one search button); it fills in as history builds up.
- Skipping a song early still costs up to ~1 MB: mpv buffers about a minute ahead.
- Only two stream sizes are used (~130 and ~50 kbps AAC); YouTube's ~65 kbps Opus stream would
  need ytmusic-rs to stop preferring AAC.
- The dev app needs `~/.local/share/com.melihgurlek.hum/WebKitCache` cleared if the window
  ever shows unstyled after switching branches.
