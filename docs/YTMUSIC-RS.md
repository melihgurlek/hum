# ytmusic-rs: what hum uses, and whether to replace it

Reviewed 2026-11-11 at the pinned rev `db4ab87` of
[sonorahq/ytmusic-rs](https://github.com/sonorahq/ytmusic-rs). The numbers are from reading the
code, not from measuring it; see [PERFORMANCE.md](PERFORMANCE.md) for what was measured.

## What it is

About 175 KB of Rust plus 175 KB of bundled JavaScript (meriyah, astring and a signature
solver) run in QuickJS through `rquickjs`. 67 commits, last push 2026-11-06, GPL-3.0.

| hum uses | hum doesn't use |
|---|---|
| `search_songs`, `artist`, `album`, `track_radio` | cookie sign-in, cookie rotation, library sync |
| raw `execute` (search, browse, next) and `parse::{find_renderers, panel_track}` | playlist edits, thumbnail upload |
| `best_audio`, `open_audio_within` (VisionOS guest client, embedded player fallback) | Premium 256k streams, PO token minting, whole-file `download` |
| the JS deciphering, only for the embedded fallback | |

hum is anonymous (`YtMusic::anonymous()`), so everything behind a sign-in is dead weight today
and becomes relevant with Google sign-in (ROADMAP Next #2).

## Should hum have its own?

Two halves, two answers.

**Metadata (search, browse, radio, lyrics): yes, if wanted.** It is JSON in, JSON out against
InnerTube. `source.rs` already does half of it by hand (top result, artist radio, lyrics). A
hum-only version is maybe 1–2k lines. It breaks when YouTube changes a layout, as any client does,
but the fix would be ours to make the same day.

**Streams: no.** This is the arms race: client tricks (VisionOS guest today), signature and `n`
deciphering by running YouTube's player JS, proof-of-origin tokens that need BotGuard in a real
browser, client versions hard-coded to July 2026. rustypipe lost it the day of the spike. Only
yt-dlp keeps up week to week; a solo rewrite would be less reliable, not more.

## Inefficiencies found

1. **An extra round trip per track.** `best_audio()` probes each URL with a `Range: bytes=0-0`
   request (`serves()`) before returning it, and mpv then opens the same URL. Likely part of why
   cold click-to-sound measured 509 ms warm in the spike vs rustypipe's 280 ms, and 0.5–2.4 s on
   Windows. Needs an API that returns the URL unprobed (mpv reports a refusal anyway, and hum
   already retries a song that won't play).
2. **Data saver opens a stream and throws it away.** `Source::resolve` calls
   `open_audio_within(…, Some(SAVER_CEILING))` only to pick by bitrate, then drops the body. There
   is no public way to choose by bitrate without opening it.
3. The startup warm-up resolves a real stream for a hard-coded video (`dQw4w9WgXcQ`); it warms the
   visitor session and connections, but a lighter request would do.
4. Search sends two `search` requests per query (`search_songs` and the unfiltered one for the
   top result).
5. QuickJS (C, compiled from source) plus 175 KB of JS are in every build for a fallback that
   only runs when the guest client fails: build time and binary size. Removing it loses the
   fallback, so keep it unless it becomes optional upstream.
6. AAC is preferred over Opus, so YouTube's ~65 kbps Opus stream is never used (already in
   ROADMAP Known issues).

## Options

- **Fork and fix 1–2**, pin hum to the fork, and send the changes upstream so the fork stays
  small. Least work, fixes the measured slowness.
- **Own metadata layer**, keep ytmusic-rs only for stream URLs. Search, browse, radio and lyrics
  become hum's code; the fragile part stays with people tracking YouTube.

Both can be done; the fork first.

**Decided 2026-11-11: no fork.** Measured, the probe is ~50 ms (median) of a ~750 ms cold click;
see ROADMAP, Decided against. Hovering a play button pre-resolves the song instead (#28).
