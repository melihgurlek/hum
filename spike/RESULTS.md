# Spike results — 2026-11-11 (mobile data, Istanbul, no login)

8 search → resolve → first-bytes runs per backend (`cargo run --release` / `--no-default-features --features rp`).

| | ytmusic-rs (Sonora, rev db4ab87) | rustypipe 0.11.4 |
|---|---|---|
| playable | 8/8 | 8/8 |
| click→first bytes, warm median | 509 ms | **280 ms** |
| cold (first track) | 806 ms | 1612 ms |
| best stream | itag 140 AAC ~131k | **itag 251 Opus ~150k** |
| data-saver stream | itag 139 ~50k | itag 139 ~50k |
| radio (RDAMVM) | 50 tracks, ~1.0 s | 50 tracks, ~1.0 s |
| search median | 614 ms | 716 ms |

Decision: **rustypipe** for metadata and streams. Warm it at startup to hide the cold cost.
The two crates bundle QuickJS (rquickjs) and cannot link into one binary, so the fallback is
yt-dlp (desktop), not ytmusic-rs. rustypipe logs `Ios` client errors and radio parse errors
but still returns results; check what they mean before relying on them.

## Correction, same day: first bytes are not enough

Playing for real in mpv returned 403. Full-download test (`FULL=1 ./target/.../hum-spike`):

- rustypipe 0.11.4: the iOS client's URL serves only the first chunk (requests ≥ 1 MiB and every
  later chunk get 403, i.e. a PO token is required after the first megabyte). Every other client
  fails to decipher ("could not get deobf data"). rustypipe is broken against current YouTube.
- ytmusic-rs: 3/3 tracks downloaded completely (VisionOS guest client, AAC itag 140 ~131k), about
  1.5-2 s per whole track. Its URLs accept open-ended ranges and seeks, so mpv plays them directly.

Decision reversed: **ytmusic-rs** for metadata and streams. First real playback through libmpv:
click→sound 754 ms (unprefetched), 219 ms (prefetched next track).
