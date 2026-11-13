# A phone version of hum

Updated 2026-11-12 after reading Metrolist, SimpMusic, ArchiveTune, Harmony Music and limusic on
GitHub (details in [COMPETITORS.md](COMPETITORS.md)).

## Verdict: worth building; the spike passed

Why it's worth it:
- That's where people listen. 86 % of SimpMusic's downloads are Android. limusic's top-voted
  issue (#214) asks for Android.
- hum's differences matter *more* on a phone. Auto quality on a shaky mobile connection, Data
  used on a metered plan, a cache that costs no extra data, radio that learns from skips. No
  Android client has any of these. Metrolist's "Auto" is just Low on metered connections.
- The field is weak where it counts. Every Android client's top complaint is playback breaking.
  InnerTune and RiMusic died of it. Metrolist ships a playback megathread per release.
- hum's structure suits it. Stream extraction, queue, prefetch, radio and Auto quality are
  already Rust in `crates/core`, and Rust runs on Android. That means one stream layer to keep
  working on both platforms.

What it costs:
- A Media3 player, a foreground service, a media session (lock screen, notification, Bluetooth),
  phone layouts, and keeping background playback alive. Weeks of work.

What not to copy from SimpMusic: it wrote the player logic twice. There are two ~3,000-line
handlers for queue, prefetch, crossfade and sleep timer, one per platform, and bugs get fixed
twice. hum should keep all of that in Rust and make Media3 a thin backend behind the same trait
as mpv: load, play, pause, seek, and report position and errors.

## The spike: passed (2026-11-12, Galaxy S23 FE, Android 16)

**Question:** can the Rust engine keep playing and moving through the queue after the WebView
is gone? **Yes.** Played a song with its radio, swiped hum away from Recents, locked the phone:
the next song started on its own, then the engine queued and started the one after (ExoPlayer
said "started", Rust moved its queue and handed over the next stream, no UI involved).

How it's built (branch `spike/android`):
- `crates/core/src/player_android.rs`: on Android the player sends each command out as JSON and
  takes events back. Queue, radio, prefetch, Auto quality and loudness stay in the engine, shared
  with desktop. Desktop keeps mpv; its dependency tree is unchanged.
- `src-tauri/src/android.rs`: the JNI bridge (~50 lines).
- `gen/android/.../HumPlayer.kt` + `PlaybackService.kt`: Media3 ExoPlayer in a
  `MediaSessionService` (foreground service, notification, lock screen, headset buttons), with
  each YouTube client's User-Agent per stream url.

Build notes: rquickjs needs `bindgen` on Android (`LIBCLANG_PATH` = the NDK's libclang, plus its
clang include dir in `BINDGEN_EXTRA_CLANG_ARGS`); R8 strips `HumPlayer.command` unless kept
(`proguard-rules.pro`), since only Rust calls it.

### Measured against Metrolist and SimpMusic, same phone, same Wi-Fi, same scripts

| | hum (release) | Metrolist 13.7.0 | SimpMusic 2.5.0 (FOSS) |
|---|---|---|---|
| APK | **16 MB** | 26 MB | 27 MB |
| Cold start, first frame | **281–395 ms** | 301–372 ms | 479–627 ms |
| Tap to sound, median of 3 | **1.34 s** (now ~0.5 s, below) | 2.1 s | 2.4 s |
| Memory, idle after launch | 86 MB, app process only (see below) | 188 MB | 132 MB |
| Memory, screen off, playing | 131 MB, app process only (see below) | 299 MB | 156 MB |
| CPU, screen off, playing | **16.9 %** | 34.8 % | 25.9 % |

Tap to sound is timed from the finger lifting (getevent) to the media session turning PLAYING,
for all three; hum's own log agreed within 50 ms. All anonymous: SimpMusic warns that without
sign-in it goes through Piped and may be slower. Data per hour wasn't measurable in 5-minute
windows (ExoPlayer fetches a whole song ahead), so it's left out.

What the numbers say:
- hum is already the lightest and fastest of the three, before any phone-specific tuning.
- Screen-off CPU is ExoPlayer's playback and decoder threads, not Rust (~0 %). Media3's audio
  offload (decoding on the DSP while the screen is off) is the next thing to try for battery.
- Tap to sound had room, and most of it was ExoPlayer: see below.

### Audio offload (2026-11-12)

Screen off, playing, 5 minutes each, % of one core from `/proc/<pid>/stat`. The earlier 16.9 %
counted hum's process only; the system's decoder and audio services were doing most of the
work:

| | Before | Offload |
|---|---|---|
| hum | 18.1 % | 2.9 % |
| media.swcodec (AAC decoder) | 10.9 % | 0.0 % |
| audioserver | 27.4 % | 1.0 % |
| audio HAL service | 7.0 % | 0.4 % |
| Total | 63.4 % | 4.4 % |

Media3 offload with gapless required; `dumpsys media.audio_flinger` shows an OFFLOAD output
(`COMPRESS_OFFLOAD`, AAC), and a song change stayed on it.

### Tap to sound, after tuning (2026-11-12)

hum's own logs split the wait (stream lookup, then the player from its play command to
PLAYING). Same phone, Wi-Fi, an app already running:

| | Before | After |
|---|---|---|
| Stream lookup | 115-280 ms | 45-280 ms (touch-down prefetch) |
| ExoPlayer, command to sound | 887-947 ms | 326-390 ms |
| Tap to sound, URL not resolved | 1106-1227 ms | 431-659 ms |
| Tap to sound, URL resolved | 900 ms | 331-396 ms |

- ExoPlayer started only after 2.5 s of buffer, a video default; hum starts at 0.5 s (2 s
  after a stall). At 130 kbps that's ~8 KB instead of ~40 KB, and the connection set-up left.
- Rows resolve their song on touch-down (a tap holds ~100 ms), and the engine shares a lookup
  in progress, so the play that follows waits for it rather than asking YouTube again.
- Still slow: YouTube's own stream lookup, 143-961 ms on any tap whose URL isn't resolved yet.
  The first tap after launch is not slower (re-measured 2026-11-12, five cold starts driven over
  adb: first songs 623-1170 ms, median 726; second songs 619-1499, median 1053; songs with the
  URL already resolved 457-485). ExoPlayer takes 400-580 ms either way, about 300 of it waiting
  for the first bytes; starting its service at launch saved nothing (it cost ~80 ms).
- The Activity and WebView stay in memory after a swipe-away (Samsung keeps them); fine at
  131 MB, but the renderer could be released when the UI is gone.

### Memory, measured again with the WebView renderer counted (2026-11-12, evening)

The memory rows above counted hum's app process only. hum's UI runs in the WebView, whose
renderer is a separate process (`com.google.android.webview:sandboxed_process…`, started with
the app); SimpMusic draws natively and has none. Same phone, same session, apps alternated,
fresh launch:

| | hum (app + renderer) | SimpMusic 2.5.0 FOSS |
|---|---|---|
| Idle after launch, 15 s, three runs | 150-153 + 58 = **~208 MB** | 146-163 MB |
| Screen off, playing, after 2 min | 100 + 61 = **161 MB** | 158 MB |
| CPU, screen off, playing (app, renderer, audio path) | 2.4 + 1.8 + 0.5 = **4.7 %** | 31.6 + 39.8 = **71.4 %** |

The app process alone grew from 86 MB to ~150 MB at launch without a code change: the same
build (main at a34f34a) measured 107 MB in the morning and 153 MB now, with the WebView
unchanged since 2026-10-24. What changed was the phone's state (library, saved queue, covers).

So: on memory, a WebView UI costs ~50 MB more than a native one while the app is open, and
it's level with the screen off. On CPU, which is what costs battery, hum uses ~15x less.

Releasing the renderer while hidden would take ~60 MB off the screen-off figure, but it isn't
worth doing: a playing app keeps a foreground service, so Android wouldn't kill hum sooner
either way; 60 MB is under 1 % of this phone's RAM; and reopening would cost a UI cold start
(~0.4 s) and the page's state.

The renderer's CPU was worth cutting: the page was sent the playing position every second
while nobody could see it. Now it isn't while the app is in the background or the screen is
off, and it gets the current position when it shows again. Screen off, playing: renderer
1.8 % to 0.6 %, app 2.4 % to 1.7 %, **4.7 % to 2.8 %** with the audio path.

### Still to do for a real port
- Phone layouts: today it's the desktop design squeezed onto a phone.
- Track title, artist and cover in the notification and lock screen (the stream has none yet).
- (Audio offload, the shorter start buffer and touch prefetch are done.)
- Song cache and Data used on Android (mpv's recording and byte counter have no ExoPlayer
  equivalent yet), saved queue across process death, battery measured over an hour unplugged.

## Must work on day one

Taken from what breaks in the other apps:
- **Background playback that survives.** SimpMusic #2233: Media3 drops the foreground service 10
  minutes after a track change. Handle dismissing the notification and clearing the task
  correctly. Tell users how to turn off battery optimization.
- **Queue and position saved**, so playback resumes after the system kills the process.
- **Recovery on each kind of error**: retry on network errors, refresh the link on a 403, try
  another client, retry at most 3 times. SimpMusic #2239: a failed prefetch leaves a dead player
  that stops playback at the next song.
- **Prefetch on visible search results.** Hovering doesn't exist on touch.
- **Auto quality and Data used**, plus an "only on Wi-Fi" setting for prefetch.
- **A screen-off battery budget.** Metrolist #3949: battery drain and garbage-collection storms.
- **Long lists loaded in pages.** Metrolist crashes out of memory at 900+ liked songs.
- **A log export** for bug reports.
- Android Auto can come second.
