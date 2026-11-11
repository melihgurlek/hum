# Would a phone version of hum help?

First pass 2026-11-11, too brief to decide on. Feature lists come from project pages and listings, not from
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

## Undecided: the first pass was too thin

The first draft concluded "don't port" from one search, and asked the wrong question: what the
phone would add to the desktop. The real question is whether hum would be the better app where
people actually listen, which is mostly the phone. Several of hum's features matter *more* there:
auto quality on a shaky mobile connection, Data used on a metered plan, a cache that costs no
extra data. MetroList being in maintenance mode is an opening.

## What their users fight with (second pass, from release notes and reviews)

- **MetroList** went into maintenance mode (v13.4.0), followed by a "future of Metrolist"
  release (v13.6.1). Recent releases keep fixing the same kinds of thing: playback freezing,
  recovering from YouTube player failures, YouTube changes that broke playback (official apps
  too), the app lingering in the background after closing, memory and background work in large
  libraries, startup crashes. A listing showed ~623 open issues. Its data control is a manual
  "Low" quality; its docs ask you to turn off battery optimization for reliable playback.
- **SimpMusic** gets praised for its UI. A review summary says the complaints are reliability
  (buffering, crashes, cache) and discovery (odd recommendations).
- **OuterTune** lists personalized quick picks too, so hum's Home isn't unique on that.

What that suggests:
- **Openings for hum:** quality that adapts on its own instead of a manual Low, radio ranked by
  your plays and skips against "odd recommendations", and a maintained app where the
  most-used one is in maintenance mode.
- **The shared risk:** their top pain, streams breaking when YouTube changes, would be hum's
  too. On Android it's a different player and possibly different client tricks. Whoever keeps
  streams working wins; hum would depend on ytmusic-rs keeping up on Android as well.

## Research to do before deciding

1. Use MetroList and SimpMusic for a few days on the phone. Note what's annoying: start time,
   data use, battery, how good radio is, design.
2. Read their open issues and reviews for recurring complaints (playback dying in the
   background, data use, sync bugs, broken streams after YouTube changes).
3. Measure on the phone: click to sound, data per hour at each quality, battery per hour.
4. Price the port: Media3 player plugin, phone layouts, background service, lock screen, and
   keeping streams working on Android.
5. Then decide, from what those apps do badly and hum could do well.

Phone as a remote stays a cheap idea on its own, whatever the decision.

Sources for the second pass: [MetroList v13.4.0, maintenance mode](https://newreleases.io/project/github/MetrolistGroup/Metrolist/release/v13.4.0),
[v13.6.1](https://newreleases.io/project/github/MetrolistGroup/Metrolist/release/v13.6.1),
[v13.6.2](https://newreleases.io/project/github/MetrolistGroup/Metrolist/release/v13.6.2),
[v13.7.0](https://newreleases.io/project/github/MetrolistGroup/Metrolist/release/v13.7.0),
[MetroList playback guide](https://mintlify.wiki/MetrolistGroup/Metrolist/guide/playback),
[SimpMusic reviews](https://www.producthunt.com/products/simpmusic/reviews),
[OuterTune](https://alternativeto.net/software/outertune/).

Sources: [alternativeto: Metrolist](https://alternativeto.net/software/metrolist/),
[alternativeto: RiMusic](https://alternativeto.net/software/rimusic/),
[SimpMusic](https://www.simpmusic.org/blogs/open-source-youtube-music-client),
[ArchiveTune](https://awesome.ecosyste.ms/projects/github.com%2Fkoiverse%2FArchiveTune),
[r/fossdroid, March 2025](https://redlib.hackliberty.org/r/fossdroid/comments/1jbwfan/other_youtube_music_apps).
