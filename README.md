# hum

An ad-free YouTube Music player for Linux, Windows and Android, made for listening, not
tinkering. No account needed, no subscription, no ads.

<p>
  <img src="docs/screenshots/phone-np.webp" alt="Now Playing on Android, colored from the album cover" width="270" />
  <img src="docs/screenshots/phone-home.webp" alt="Home on Android: quick picks and recently played" width="270" />
</p>

## What it does

- **Search and play** any song from YouTube Music, usually within half a second.
- **Endless radio**: every song keeps going with songs like it, ranked by what you actually
  listen to and skip.
- **Now Playing** that takes its colors from the album cover, with **synced lyrics**.
- **Liked songs, playlists and history**, stored locally. Sign in to sync your YouTube Music
  playlists and likes (optional).
- **Saved songs**: songs you play to the end are kept, so playing them again costs no data and
  works offline. Download whole playlists, and cap the cache size.
- **Auto sound quality** that steps down when the connection stalls, **Data saver**, and how
  much data hum has used.
- **Media keys** and the system's media controls (MPRIS on Linux, SMTC on Windows, the lock
  screen and notification on Android).

## How light it is

Android, Galaxy S23 FE, same Wi-Fi, apps alternated in one session
([docs/MOBILE.md](docs/MOBILE.md) has the method and the raw numbers):

| | hum | SimpMusic 2.5.0 | Metrolist 13.7.0 |
|---|---|---|---|
| APK | **16 MB** | 27 MB | 26 MB |
| Tap to sound | **~0.4–0.7 s** | 2.4 s | 2.1 s |
| CPU, screen off, playing (incl. the system's audio) | **2.8 %** of a core | 71 % | |
| Memory, screen off, playing | 161 MB | 158 MB | |
| Memory, open and idle | 208 MB | ~150 MB | |

hum's interface is a web page (Tauri), and the phone's WebView costs about 50 MB more than a
native interface while the app is open. Playback itself is native (ExoPlayer with audio
offload on Android, libmpv on desktop), so with the screen off hum is level on memory and uses
a small fraction of the CPU, which is what costs battery. Desktop numbers are in
[docs/PERFORMANCE.md](docs/PERFORMANCE.md).

## Built with

Rust + [Tauri 2](https://tauri.app) + Svelte 5. Playback through libmpv on desktop and Media3
ExoPlayer on Android. YouTube Music access through Sonora's
[ytmusic-rs](https://github.com/sonorahq/ytmusic-rs) (a fork with fixes offered back). Lyrics
from [LRCLIB](https://lrclib.net). Library in SQLite.

```
crates/core   source, player, queue, radio, library, lyrics (no UI code; shared by desktop and Android)
src-tauri     app shell: commands, media controls, cover-art cache, Android bridge
ui            Svelte interface (desktop and phone layouts)
docs          DESIGN.md (design spec), ROADMAP.md (done and next), MOBILE.md, PERFORMANCE.md
```

## Run it (Fedora)

```sh
sudo dnf install webkit2gtk4.1-devel libsoup3-devel gtk3-devel librsvg2-devel mpv-libs mpv-devel
cd ui && npm install && cd ..
./ui/node_modules/.bin/tauri dev
```

Tests: `cargo test -p hum-core`. Headless playback check:
`cargo run -p hum-core --example play -- "duman bu akşam"`.

Android: `./ui/node_modules/.bin/tauri android build --apk --target aarch64` with the Android
SDK and NDK; the build notes are in [docs/MOBILE.md](docs/MOBILE.md). There are no published
builds yet.

## Status

Personal project, under active development. See [docs/ROADMAP.md](docs/ROADMAP.md).
Licensed GPL-3.0-or-later (ytmusic-rs is GPL-3).
