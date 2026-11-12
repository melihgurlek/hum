# A phone version of hum

Updated 2026-11-12 after reading Metrolist, SimpMusic, ArchiveTune, Harmony Music and limusic on
GitHub (details in [COMPETITORS.md](COMPETITORS.md)).

## Verdict: worth building, after desktop reliability, starting with one spike

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

## The spike that decides it

Before any UI work: **can the Rust engine keep playing and prefetching after the WebView is
gone?**
- Run a Media3 foreground service that drives `crates/core` through the player trait.
- Kill the activity, turn the screen off for 30 minutes, and switch networks. It must keep
  playing and moving to the next song.
- Check what Tauri does when Android kills the WebView renderer (`onRenderProcessGone`), and how
  long the WebView takes to start on a 3–4 GB phone.

If that fails, the port means a native Android shell around `crates/core` instead of Tauri's
WebView, and it costs more.

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

## Before the spike

The desktop reliability work in [ROADMAP.md](ROADMAP.md) comes first: per-song client recovery,
cipher configs from the registries, clear error messages. A phone on mobile data hits every one
of those failures more often than a desktop on Wi-Fi.

The phone-as-remote idea is dropped. Taking the phone out to scan a QR code is more hassle than
reaching for the laptop.
