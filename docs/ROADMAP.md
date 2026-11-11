# hum roadmap

Updated in every PR. Newest done items at the top of each section.

## Done

| PR | What |
|---|---|
| #25 | Windows build in GitHub Actions: NSIS installer (38 MB) with shinchiro's libmpv (version and SHA-256 recorded per build), smoke test on every relevant PR; first run green |
| #24 | Performance: idle player sleeps until needed (was 50 wakeups/s), search and quick-picks caches survive navigation, 20 s read-ahead, stripped release binary |
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

1. **Test the Windows installer on a real Windows machine** (CI can't hear audio). Get it from the
   latest main or "Run workflow" run of *Windows build*, artifact `hum-windows-x64`. Unsigned, so
   SmartScreen asks: More info → Run anyway. Check:
   - a song plays **with sound**;
   - **cover images** load (Windows serves them from `http://img.localhost/`, the likeliest break);
   - **media keys** and the Windows media overlay (title, play/pause);
   - Now Playing colors, lyrics, search, an artist page;
   - startup feels as quick as on Linux.
   Problems: screenshot + `%LOCALAPPDATA%\com.melihgurlek.hum\logs\hum.log`; fixes are made from Linux.
   Measured on Windows ([PERFORMANCE.md](PERFORMANCE.md)): engine ready ~1.0 s after launch,
   search ~0.6 s, click→sound 0.5–2.4 s cold and ~0.3 s prefetched, 226 MB private memory idle,
   133 MB installed. Playing uses ~28 % of a core in the webview (to profile).
2. **Faster stream resolution** ([YTMUSIC-RS.md](YTMUSIC-RS.md)): ytmusic-rs probes every URL
   before mpv opens it (an extra round trip per song), and Data saver opens a stream only to pick
   its bitrate. Fork, fix both, send upstream. Don't replace the stream half; the metadata half
   could become hum's own later.
3. **Google sign-in** (webview cookie login) to sync likes and playlists with the YouTube Music
   account, so hum (desktop) and MetroList (phone) share one library. Few days; Android would need
   it anyway.

## Later

- Song cache on disk (replays use no data), offline mode for cached songs.
- Crossfade, equalizer, skip silence, sleep timer.
- Discord status, Last.fm scrobbling.
- Mini player, tray menu.
- Code signing for the Windows installer (costs money; worth it once hum is public).
- **Android port, on hold.** Same `ui/` and `crates/core`, but no mpv there: needs a Media3 player
  plugin, phone layouts, background playback, lock-screen controls (weeks). Open question first:
  what would hum on Android do better than MetroList, which already works well on the phone?

## Decided against (and why)

- **Search field on Home**: Search is one click in the sidebar and opens with the field focused.
- **Keyboard-shortcut hints**: nobody drives a music player by shortcuts.
- **Stricter artist spread in radio**: hum's radio is YouTube Music's own and measured as varied
  (about 1 in 4 songs by the seed artist, same as YouTube continued). Easy to tighten if wanted.
- **Patching souvlaki** (100 idle wakeups/s from its 10 ms poll): left as is. It accepts outside
  PRs and has no AI policy, if that changes.
- **Memory trims that cost UX**: capping mpv's back buffer (slow seek-back), smaller tile covers
  (blurry on HiDPI), paging long lists, `MALLOC_ARENA_MAX` (−4 MB, noise). Most of the ~287 MB is
  WebKit's fixed cost; no unbounded growth found in our code.
- **Leaving the webview for a native UI (egui, like spotifast)**: would save ~150 MB but means
  rewriting the UI and losing cover-tinted design and the shared Android `ui/`.

## Known issues

- While a song plays the webview uses ~28 % of a CPU core on Windows (hum.exe ~2 %); idle and
  paused are ~0 %. Likely progress-bar or playing-indicator redraws; profile before fixing.
- libmpv-2.dll is 116 of the 133 MB installed (shinchiro's build has every codec, video too).

- Artist top songs have no durations until played (YouTube Music doesn't list them there).
- Only a song's first artist links to a page; YouTube Music's song lists carry one artist id.
- Home starts nearly empty (one search button); it fills in as history builds up.
- Skipping a song early costs up to ~0.3 MB at High: mpv reads 20 s ahead.
- Only two stream sizes are used (~130 and ~50 kbps AAC); YouTube's ~65 kbps Opus stream would
  need ytmusic-rs to stop preferring AAC.
- The dev app needs `~/.local/share/com.melihgurlek.hum/WebKitCache` cleared if the window
  ever shows unstyled after switching branches.
