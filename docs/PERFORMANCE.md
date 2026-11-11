# Performance on Windows

Measured 2026-11-11 on the installer from `main` (commit 01e8084, *Windows build* run
43084181839). AMD Ryzen 5 5600H (12 threads), 15 GB RAM, Windows 11 Home, not signed in. The library database was backed up before and restored after, so the test plays
are not in history.

How: hum.exe started 5 times with WebView2's DevTools port open
(`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=…`). Times come from the page's
own `performance` entries and from `hum.log`. Response times are the app's own Tauri commands,
called from inside the webview, so they include the IPC round trip but not rendering.
Click-to-sound is the engine's own `Timing` update. Memory and CPU are summed over hum.exe and
every WebView2 process under it (7 processes).

## Launch (5 launches, OS caches warm)

| From process start to | Median | Range |
|---|---|---|
| window exists | 62 ms | 53–145 |
| webview starts loading | 624 ms | 563–696 |
| page loaded | 759 ms | 701–852 |
| first paint | 832 ms | 781–932 |
| engine ready (`source warm` in the log) | 1039 ms | 927–1130 |

The warm-up itself (resolving one stream) took 277–504 ms, median 311 ms, and runs alongside the
webview start. About 0.5 s of every launch is WebView2 starting up before any of hum's code runs.

## Response times

| Command | Time |
|---|---|
| search (5 different queries) | 470–657 ms, median 606 ms |
| the same search again | 532 ms (no cache in the backend) |
| artist page | 374, 395 ms |
| album page | 333, 785 ms |
| radio (50 songs) | 831 ms |
| lyrics | 195 ms (first), 90 ms |
| cover palette | 150 ms |

| Click to sound | Times |
|---|---|
| cold (song not prefetched) | 1088, 2388, 489 ms, median 1088 ms |
| prefetched next song | 296, 314, 22 ms |

The Linux spike measured ~460 ms cold and ~140 ms prefetched. Windows cold plays here were slower
and spread widely (one took 2.4 s). That points at stream resolution (see
[ytmusic-rs review](YTMUSIC-RS.md): an extra round trip per track) and the network, not the
player.

## Memory and CPU

| State | Memory (private) | Working set | hum.exe alone (WS / private) | CPU, % of one core |
|---|---|---|---|---|
| idle, 20 s after launch | 226 MB | 414 MB | 49 / 25 MB | 0.4 % (hum.exe 0 %) |
| playing | 255 MB | 463 MB | 82 / 43 MB | **29.8 %** (hum.exe 2 %) |
| paused, after browsing and 6 plays | 283 MB | 480 MB | 82 / 43 MB | 0 % |

- Idle is quiet: the #24 work holds on Windows.
- **Playing costs ~28 % of a core in the webview**, not in hum.exe (mpv decoding is the 2 %).
  Something in the UI redraws constantly while a song plays: most likely the position updates
  driving the progress bar, the playing-indicator animation, or both. Worth a profile in DevTools
  (Performance tab) before guessing; it matters for laptop battery.
- Working set counts shared WebView2 pages several times; private memory is the closer number.
  Most of it is WebView2 (365 MB of the 414 MB working set at idle).

## Disk

| What | Size |
|---|---|
| installer (`hum_*_x64-setup.exe`) | ~38 MB |
| installed, `%LOCALAPPDATA%\hum` | 133 MB: libmpv-2.dll 116 MB, hum.exe 17 MB |
| WebView2 profile, `%LOCALAPPDATA%\com.melihgurlek.hum\EBWebView` | 54 MB after light use; grows with use |
| cover cache, `…\images` | 1.3 MB for 61 covers |
| library, `%APPDATA%\com.melihgurlek.hum\hum.db` | 0.2 MB |

WebView2 itself ships with Windows and isn't counted. libmpv is 87 % of the install: shinchiro's
build includes every codec and filter (video, too); an audio-only libmpv build would likely be
much smaller, at the cost of building it in CI.
