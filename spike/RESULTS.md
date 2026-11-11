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
