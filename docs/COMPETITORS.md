# What other YouTube Music clients teach hum

Read from their GitHub repos on 2026-11-12: source, issues sorted by reactions, release notes.
Nothing was run, so features come from code and READMEs. Issue counts are keyword matches on
titles, good for ranking only.

## Who's out there

| App | Stack | State | Notes |
|---|---|---|---|
| **limusic** (SimoHypers/limusic) | Rust, Tauri 2, Svelte, libmpv | Active, ~2k stars since 2026-07 | Same stack as hum, desktop only. Already ships tray, mini player, Discord, Last.fm, ListenBrainz, word-by-word lyrics, multi-account, COPR/AUR/AppImage. Its top issue (#214) asks for Android. hum's closest competitor. |
| **Metrolist** | Kotlin, Media3 | 14.2k stars, "maintenance mode" banner but 13.7.0 in Sep 2026 | 411 open issues. Rewriting in Kotlin Multiplatform with a desktop target (alpha). |
| **SimpMusic** | Kotlin + Compose Multiplatform | Active | Desktop since v1.0.0 (Nov 2025). 86 % of downloads are Android, 1.3 % Linux. |
| **ArchiveTune** (rukamori/ArchiveTune) | Kotlin, Media3 | Active | Best stream recovery and best recommendation code of the lot. |
| **Pear Desktop** (pear-devs/pear-desktop, ex th-ch/youtube-music) | Electron wrapping the web app | Active, 34k stars | 37 plugins. Memory leaks, breaks when YouTube changes the web UI. |
| OuterTune, InnerTune, RiMusic, ViMusic, Harmony Music | | Dead or abandoned | All died the same way: playback broke when YouTube changed something and nobody kept up. |

## The one lesson: whoever keeps streams playing wins

Every app's top complaint is "won't play": Metrolist ran five playback megathreads in three
months, ~25 % of SimpMusic's open issues, 43 of RiMusic's last 81. What the survivors do:

- **Many clients, health-tracked.** Metrolist keeps ~35 YouTube clients, each with a priority,
  PoToken rule and dated benchmark; a client is marked broken when a *second* range request
  403s (a whole-song check, as in hum's spike). Its benchmark also picks VisionOS as the fastest
  client, hum's primary.
- **Per-song recovery.** On a playback error Metrolist drops the URL and cached entry, blacklists
  that client for that song for 5 minutes, reseeks and retries (3 times); if every client is
  out, it retries all of them at low quality. ArchiveTune does the same with 6 clients and
  per-client back-off, plus a retry that wipes tokens and visitorData.
- **Cipher fixes without an app release.** Regex cipher extraction is dead. Metrolist,
  SimpMusic and limusic fetch per-player-hash configs from community registries
  (`MetrolistGroup/faraday`, `ZemerTeam/zemer-cipher`) with a few hours' TTL, refreshed
  immediately when an unknown player hash shows up.
- **Signed-in playback attracts bot checks.** Metrolist users get "confirm you're not a bot"
  when signed in; logging out fixes it. hum playing anonymously is the right default. But
  SimpMusic #2861 shows the VisionOS guest client failing on bot-flagged IPs, where `tv_samsung`
  and `mweb` with cookies worked 12/12, so hum needs a cookie fallback for those users.
- **Honest errors.** SimpMusic reports most failures as "Time out, check internet" (~30 issues
  about it). Users can't tell a network problem from a broken client.

Checked and dropped: limusic's code says googlevideo throttles open-ended range requests to ~2x
realtime. Measured here: ffmpeg (what mpv uses) sends `Range: bytes=0-` and got a whole song
in 0.3 s (~14 MB/s). Only a request without any Range header was throttled (32 KB/s). hum
isn't affected.

## Recommendations: where hum stands

Ahead of every one of them:
- Radio ranked by your own plays and skips. Everyone else plays YouTube's radio as-is.
  Metrolist doesn't even record skips (it stores plays of 30 s or more).
- Auto quality from measured stalls. Metrolist's Auto is `metered ? LOW : AUTO`; the rest are
  manual.
- A cache that costs no extra data (SimpMusic users ask for one, #2658), and a Data used count.
- Click to sound ~0.4–0.75 s. SimpMusic desktop waits 7–14 s for uncached songs (#2840).

Behind:
- **Seeding.** Quick picks use one seed (the last song). ArchiveTune fuses up to 6 seeds,
  scores songs by how many seeds point to them, and shuffles the top of the pool so the shelf
  rotates (`home/HomeUseCases.kt`).
- **Cold start.** Likes imported from the account give no taste until played in hum
  (`library.rs` `scores`, by design since #30). So a fresh hum with 80 account likes has an
  almost empty Home. ArchiveTune seeds from the account's YTM history when it has fewer than 6
  local seeds.
- **Radio depth.** hum fetches one page of YouTube's radio per seed. limusic and Harmony follow
  continuations.
- **Freshness.** hum drops anything heard in the last 6 hours. limusic demotes songs heard in
  the last 30 days, least recent first, which keeps radio fresh without starving it.
- **The affinity term is weak**: at most ~0.05 of the score for someone with a long tail of
  artists, so radio is in practice YouTube's order plus hum's filters.
- **No artist block.** Asked for in Metrolist (#294), RiMusic, ViMusic; limusic and ArchiveTune
  have it.

Nobody does time-of-day picks, smart shuffle or "not interested" feedback to YouTube. Not
worth leading on.

## Desktop features users ask for, by demand

Across Pear, limusic, Metrolist and RiMusic issues:
1. Restore queue and position on launch (Pear #3004, #858).
2. Tray and mini player (Pear #2557; limusic has both).
3. Scrobbling: ListenBrainz (Metrolist #2217, 19 reactions), Last.fm.
4. Loudness normalization (Pear #3032). ytmusic-rs already returns `loudness_db`; mpv can apply
   it per song.
5. Sleep timer (with fade), skip silence, EQ: cheap in mpv.
6. Discord status.
7. Offline mode on the existing cache; "keep for offline".
8. Flatpak / COPR packaging (Pear #737, the top-reacted issue). Signed Windows installer:
   SimpMusic has five antivirus-false-positive issues.
9. Year-in-review from local history. SimpMusic's analytics are the template (new artists via
   first-play time, plays by hour, a 30-active-day gate).

Avoid: settings sprawl (RiMusic has 60+ settings enums and 599 open feature requests),
crossfade before gapless (buggy in every client that has it), a downloader built on scraping.

## Linux notes for this machine

limusic users on Fedora report PipeWire stutter (#655, worked around with `ao=pulse` and
`audio-buffer=0.5`) and a WebKitGTK black window on NVIDIA (#523). Worth a pass on Wayland with
the RTX 3050.
