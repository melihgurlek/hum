# hum

An ad-free YouTube Music player for Linux and Windows, made for listening, not tinkering.
No account, no subscription, no ads.

## What it does

- **Search and play** any song from YouTube Music, usually within half a second.
- **Endless radio**: every song keeps going with songs like it, ranked by what you actually
  listen to and skip.
- **Now Playing** that takes its colors from the album cover, with **synced lyrics**.
- **Liked songs, playlists and history**, stored locally.
- **Media keys** and the desktop's media controls (MPRIS on Linux, SMTC on Windows).
- **Data saver** and Auto sound quality; cover art is cached so it downloads once.

## Built with

Rust + [Tauri 2](https://tauri.app) + Svelte 5. Playback through libmpv. YouTube Music access
through Sonora's [ytmusic-rs](https://github.com/sonorahq/ytmusic-rs). Lyrics from
[LRCLIB](https://lrclib.net). Library in SQLite.

```
crates/core   source, player, queue, library, lyrics (no UI code; reused for Android later)
src-tauri     desktop shell: commands, media controls, cover-art cache
ui            Svelte interface
docs          DESIGN.md (design spec), ROADMAP.md (done and next)
```

## Run it (Fedora)

```sh
sudo dnf install webkit2gtk4.1-devel libsoup3-devel gtk3-devel librsvg2-devel mpv-libs mpv-devel
cd ui && npm install && cd ..
./ui/node_modules/.bin/tauri dev
```

Tests: `cargo test -p hum-core`. Headless playback check:
`cargo run -p hum-core --example play -- "duman bu akşam"`.

## Status

Personal project, under active development. See [docs/ROADMAP.md](docs/ROADMAP.md).
Licensed GPL-3.0-or-later (ytmusic-rs is GPL-3).
