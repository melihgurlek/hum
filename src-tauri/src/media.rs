//! OS media controls: MPRIS on Linux (KDE's media widget, keyboard media keys, playerctl) and
//! the System Media Transport Controls on Windows.
//!
//! souvlaki's handle stays on one dedicated thread. Engine updates come in through a broadcast
//! receiver; OS button presses go out to the engine on the async runtime.

use std::time::Duration;

use hum_core::source::images;
use hum_core::{Engine, Update};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use tauri::{AppHandle, Manager};

const SEEK_STEP: f64 = 10.0;

pub fn spawn(app: AppHandle, engine: Engine, hwnd: Option<usize>) {
    let updates = engine.subscribe();
    std::thread::Builder::new()
        .name("media-controls".into())
        .spawn(move || {
            if let Err(e) = run(app, engine, hwnd, updates) {
                log::warn!("media controls unavailable: {e}");
            }
        })
        .ok();
}

fn run(
    app: AppHandle,
    engine: Engine,
    hwnd: Option<usize>,
    mut updates: tokio::sync::broadcast::Receiver<Update>,
) -> Result<(), String> {
    let config = PlatformConfig {
        display_name: "hum",
        dbus_name: "hum",
        hwnd: hwnd.map(|h| h as *mut std::ffi::c_void),
    };
    let mut controls = MediaControls::new(config).map_err(|e| format!("{e:?}"))?;

    let e = engine.clone();
    controls
        .attach(move |event| on_event(&app, &e, event))
        .map_err(|e| format!("{e:?}"))?;
    let _ = controls.set_playback(MediaPlayback::Stopped);

    let mut paused = true;
    let mut position = 0.0f64;
    // mpv reports `pause=false` at startup with nothing loaded; stay Stopped until a track.
    let mut has_track = false;
    loop {
        let update = match updates.blocking_recv() {
            Ok(u) => u,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => return Ok(()),
        };
        match update {
            Update::NowPlaying { track, .. } => {
                has_track = true;
                position = 0.0;
                let artist = track.artists.join(", ");
                // The cached big cover when there is one, else a big URL that always exists.
                let cover = images::best_cached(track.cover.as_deref(), track.thumb.as_deref())
                    .and_then(|p| reqwest::Url::from_file_path(p).ok())
                    .map(|u| u.to_string())
                    .or_else(|| track.cover.as_deref().map(images::large_sure));
                let _ = controls.set_metadata(MediaMetadata {
                    title: Some(&track.title),
                    artist: Some(&artist),
                    album: track.album.as_deref(),
                    cover_url: cover.as_deref(),
                    duration: track.duration.map(|d| Duration::from_secs(d as u64)),
                });
                let _ = controls.set_playback(playback(paused, position));
            }
            Update::Paused { paused: p } => {
                paused = p;
                if has_track {
                    let _ = controls.set_playback(playback(paused, position));
                }
            }
            Update::Position { secs } => {
                // Clients extrapolate position while playing, so only report jumps (seeks),
                // not every tick.
                let jumped = (secs - position).abs() > 2.5;
                position = secs;
                if jumped && has_track {
                    let _ = controls.set_playback(playback(paused, position));
                }
            }
            _ => {}
        }
    }
}

fn playback(paused: bool, secs: f64) -> MediaPlayback {
    let progress = Some(MediaPosition(Duration::from_secs_f64(secs.max(0.0))));
    if paused {
        MediaPlayback::Paused { progress }
    } else {
        MediaPlayback::Playing { progress }
    }
}

fn on_event(app: &AppHandle, engine: &Engine, event: MediaControlEvent) {
    let e = engine.clone();
    match event {
        MediaControlEvent::Play => e.set_paused(false),
        MediaControlEvent::Pause | MediaControlEvent::Stop => e.set_paused(true),
        MediaControlEvent::Toggle => e.toggle_pause(),
        MediaControlEvent::Next => {
            tauri::async_runtime::spawn(async move {
                let _ = e.next().await;
            });
        }
        MediaControlEvent::Previous => {
            tauri::async_runtime::spawn(async move {
                let _ = e.previous().await;
            });
        }
        MediaControlEvent::SetPosition(MediaPosition(d)) => e.seek(d.as_secs_f64()),
        MediaControlEvent::Seek(dir) => e.seek(step(e.position(), dir, SEEK_STEP)),
        MediaControlEvent::SeekBy(dir, d) => e.seek(step(e.position(), dir, d.as_secs_f64())),
        MediaControlEvent::SetVolume(v) => e.set_volume(v * 100.0),
        MediaControlEvent::Raise => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        MediaControlEvent::Quit => app.exit(0),
        _ => {}
    }
}

fn step(pos: f64, dir: SeekDirection, by: f64) -> f64 {
    match dir {
        SeekDirection::Forward => pos + by,
        SeekDirection::Backward => (pos - by).max(0.0),
    }
}
