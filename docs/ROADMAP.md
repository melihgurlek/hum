# hum roadmap

Updated in every PR. Newest done items at the top of each section.

## Done

| PR | What |
|---|---|
| #38 | Home on a first launch: YouTube Music's own home page (the account's when signed in) until something is played; quick picks fuse up to four seeds' radios (latest plays, favorites, then account likes), so likes from the account fill Home at once |
| #33 | Song cache: a song played to the end without a seek is kept on disk (mpv records it as it plays, no extra data), up to 1 GB, oldest first. Replays start in ~20 ms instead of ~900 ms and use no data. Settings shows saved songs, with Clear |
| #32 | Settings → Playback: "Radio from my YouTube Music account" (off by default, shown when signed in). Measured: it replaces about half of each radio (13–30 of 50 shared with the anonymous one, which repeats itself 48–50 of 50) |
| #29 | Google sign-in (Settings), and likes and playlists synced both ways with the YouTube Music account: changes go up at once, the account's come down at startup and when Liked songs or a playlist opens. Search and playback stay anonymous |
| #28 | Every play button resolves its song while the pointer is on it, not just track rows: Home tiles and buttons, Play on album, artist and playlist pages, Up next, recent searches. A hovered click skips the 100–350 ms resolve (click→sound ~750 ms cold → ~400 ms) |
| #27 | Playing no longer costs a fifth of a CPU core: the progress bar steps once a second instead of gliding (a CSS transition restarted every second kept the webview drawing 60 fps), and the playing mark in lists is static. Linux webview 19 % → 1.8 % of a core while playing. Windows installer tested on a real machine: sound, covers, media keys work ([PERFORMANCE.md](PERFORMANCE.md)) |
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

Status 2026-11-12 (Linux): sign-in, account sync and the song cache (#33) work against a real
account. Order below comes from [COMPETITORS.md](COMPETITORS.md): every dead client died of
playback breaking, so reliability comes first.

1. **Playback that recovers on its own**: on an error, drop that song's link and cache entry,
   skip the failing client for that song for 5 minutes, retry (at most 3), and as a last resort
   try every client at low quality. Needs ytmusic-rs to accept clients to skip.
2. **Cipher configs from the community registries** (`MetrolistGroup/faraday`,
   `ZemerTeam/zemer-cipher`), refreshed when an unknown player hash appears, so YouTube
   changes are fixed without a hum release.
3. **Errors that say what broke** (network, link expired, client blocked, bot check), with a
   "Copy playback report" button (client, itag, error).
4. **Recommendations**: demote songs heard in the last 30 days instead of dropping the last 6
   hours; follow radio continuations; block an artist.
5. **Cheap desktop wins**: restore queue and position on launch, loudness normalization from
   `loudness_db`, sleep timer with fade.
6. **Test on Windows** (WebView2): sign-in, account sync, the song cache, account radio.
7. **Test the song cache by hand**: seeking or skipping mid-song must discard the recording; a
   song played to the end shows up under Settings → Saved songs.
8. **Android spike** ([MOBILE.md](MOBILE.md)): the Rust engine playing from a Media3 foreground
   service after the WebView is gone.

## Later

- Send plays to the account's history, as an off-by-default setting (as Stash does). Code parked
  on `feat/play-history`: YouTube accepts the pings, but it couldn't be confirmed they count,
  since the test account's history has been paused since 2026-02. Paused history also means
  YouTube ignores listening for recommendations, so it only matters to people who keep it on.
- Offline mode: play saved songs with no connection (the cache is there since #33).
- Tray and mini player.
- Scrobbling (ListenBrainz, Last.fm), Discord status.
- Skip silence, equalizer; gapless before crossfade (crossfade is buggy in every client that has it).
- Cookie fallback for users whose IP gets YouTube's bot check (the guest VisionOS client fails there, SimpMusic #2861).
- Year in review from local history (SimpMusic's analytics are the template).
- Flatpak / COPR packaging.
- Code signing for the Windows installer (costs money; worth it once hum is public).
- **Android port**: worth building once the spike passes; plan in [MOBILE.md](MOBILE.md).

## Decided against (and why)

- **Forking ytmusic-rs to drop its URL probe**: measured on 12 songs, the probe is 4–250 ms
  (median ~50 ms) of a ~750 ms cold click; mpv's own connection (~400 ms) and the player request
  (100–320 ms) are the rest. Not worth a fork, and the probe catches refused URLs. Hovering
  pre-resolves instead (#28).
- **Fluid progress bar**: 60 fps costs ~19 % of a core, 4 updates a second ~4.6 %, once a second
  ~1.8 %. Steps of 2–3 px a second are barely visible.

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

- Playlists saved from other people don't show; only your own sync.
- Edits to a synced playlist made while signed out (other than deleting it) are overwritten by the
  account's copy at the next sync.

- libmpv-2.dll is 116 of the 133 MB installed (shinchiro's build has every codec, video too).

- Artist top songs have no durations until played (YouTube Music doesn't list them there).
- Only a song's first artist links to a page; YouTube Music's song lists carry one artist id.
- Skipping a song early costs up to ~0.3 MB at High: mpv reads 20 s ahead.
- Only two stream sizes are used (~130 and ~50 kbps AAC); YouTube's ~65 kbps Opus stream would
  need ytmusic-rs to stop preferring AAC.
- The dev app needs `~/.local/share/com.melihgurlek.hum/WebKitCache` cleared if the window
  ever shows unstyled after switching branches.
