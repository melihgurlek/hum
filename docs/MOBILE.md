# Would a phone version of hum help?

Looked into on 2026-11-11, briefly. Feature lists come from project pages and listings, not from
using each app, so treat the "they don't" column as unverified.

## The field on Android

Open-source YouTube Music clients are crowded and mature:
- **MetroList**: account sync (likes, playlists, albums, artists), offline caching, live lyrics,
  background play. The most-mentioned one, now in maintenance mode.
- **SimpMusic**: MIT, downloads for offline, actively maintained.
- **OuterTune, ArchiveTune**: InnerTune forks, Material 3.
- **InnerTune, RiMusic**: discontinued.

All of them already do what a hum port would start with: background play, lock-screen controls,
offline, lyrics, account sync.

## What hum does that they (as far as listed) don't

- Radio ranked by *your* listening: skips in the last 14 days dropped, songs heard in the last
  6 hours dropped, no more than 2 by one artist in any 10. Plays count, likes are a fading bonus.
- "Haven't played these in a while", and the reason each song is in Up next.
- Auto quality that learns from stalls (step down after 2 in 2 minutes; step up only when
  measured fast), and a Data used count.
- A cache that costs no extra data: the first play records itself.
- A design tinted by the cover.

None of that needs a phone to be useful, and all of it could be built into any of those apps.

## What a port would add

- **The same taste on both devices**: but plays and skips live in hum's local database. The
  phone and the desktop would each learn separately unless hum syncs its own history, which
  needs a server or a sync service hum doesn't have. YouTube's history can't carry it (it's paused
  on the test account anyway, see ROADMAP Later).
- **Shared library**: already done. Likes and playlists sync through the account (#29), so hum
  on the desktop and MetroList on the phone see the same library today.

Cost: Media3 player plugin instead of mpv, phone layouts, background service, lock screen,
battery work. Weeks, and then upkeep against apps that already have years on it.

## Verdict

Don't port. The phone is the crowded side; the desktop isn't. Desktop YouTube Music apps are
mostly Electron wrappers around the website, and hum's edge (low CPU and memory, a fast start,
its own taste ranking) shows most there.

## Things that would help instead

1. **Phone as a remote** (days, not weeks): hum serves a small page on the home network; scan a QR
   code from Settings and control playback, the queue and search from the phone. It gives the
   phone something MetroList can't do (drive the desktop), with no second player to maintain.
2. **Sync hum's own taste between devices**, if a port ever happens: export and import the
   plays table, or a sync folder. Without it, a port is just one more client.
3. **Offline mode** on the desktop, now that the cache exists (#33).

Sources: [alternativeto: Metrolist](https://alternativeto.net/software/metrolist/),
[alternativeto: RiMusic](https://alternativeto.net/software/rimusic/),
[SimpMusic](https://www.simpmusic.org/blogs/open-source-youtube-music-client),
[ArchiveTune](https://awesome.ecosyste.ms/projects/github.com%2Fkoiverse%2FArchiveTune),
[r/fossdroid, March 2025](https://redlib.hackliberty.org/r/fossdroid/comments/1jbwfan/other_youtube_music_apps).
