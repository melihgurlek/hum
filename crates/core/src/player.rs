//! Audio playback through libmpv, on a dedicated thread (the mpv handle is not `Send`).
//!
//! mpv's own playlist holds the current track plus the prefetched next one, so advancing is
//! gapless. Our side tracks which video id sits at which mpv playlist position.

use std::sync::mpsc;
use std::thread;

use anyhow::{Result, anyhow};
use libmpv2::Mpv;
use libmpv2::events::{Event, PropertyData};
use libmpv2::Format;
use tokio::sync::mpsc::UnboundedSender;

use crate::source::Stream;

#[derive(Debug)]
pub enum Command {
    /// Replace everything and play this stream now.
    Play(Stream),
    /// Replace the current track's stream with another one from `secs` (a quality change).
    Reload(Stream, f64),
    /// Queue this stream to play after the current one (gapless).
    SetNext(Stream),
    SetPaused(bool),
    TogglePause,
    /// Seconds from the start.
    Seek(f64),
    /// 0-100.
    Volume(f64),
    Stop,
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    /// A track started producing audio. `video_id` is the one now playing.
    Started { video_id: String },
    Position(f64),
    Duration(f64),
    Paused(bool),
    /// Playback stopped to wait for the network, partway through a track.
    Stalled,
    /// Bytes downloaded since the last report, and the download speed right now in bytes per
    /// second (mpv's `cache-speed`).
    Downloaded { bytes: u64, rate: f64 },
    /// The last item in mpv's playlist ended and nothing follows.
    Idle,
    Error { video_id: Option<String>, message: String },
}

pub struct Player {
    tx: mpsc::Sender<Command>,
}

impl Player {
    pub fn spawn(events: UnboundedSender<PlayerEvent>) -> Result<Player> {
        let (tx, rx) = mpsc::channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();
        thread::Builder::new()
            .name("mpv".into())
            .spawn(move || run(rx, events, ready_tx))?;
        ready_rx.recv().map_err(|_| anyhow!("mpv thread died"))??;
        Ok(Player { tx })
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }
}

fn init_mpv() -> Result<Mpv> {
    // mpv refuses to start unless LC_NUMERIC is "C". GTK applies the user's locale, and a
    // Turkish locale uses a decimal comma.
    #[cfg(unix)]
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
    }
    let mpv = Mpv::with_initializer(|init| {
        init.set_option("vid", "no")?;
        init.set_option("video", "no")?;
        init.set_option("audio-display", "no")?;
        init.set_option("gapless-audio", "weak")?;
        init.set_option("prefetch-playlist", "yes")?;
        init.set_option("cache", "yes")?;
        init.set_option("cache-secs", 60i64)?;
        init.set_option("demuxer-max-bytes", "8MiB")?;
        init.set_option("demuxer-readahead-secs", 20i64)?;
        init.set_option("network-timeout", 15i64)?;
        init.set_option("ytdl", "no")?;
        init.set_option("idle", "yes")?;
        init.set_option("keep-open", "no")?;
        init.set_option("terminal", "no")?;
        if let Ok(path) = std::env::var("HUM_MPV_LOG") {
            init.set_option("log-file", path.as_str())?;
        }
        Ok(())
    })
    .map_err(|e| anyhow!("mpv init: {e}"))?;
    Ok(mpv)
}

/// `loadfile` per-file option value, escaped with mpv's `%len%` syntax so commas survive.
fn escaped(v: &str) -> String {
    format!("%{}%{}", v.len(), v)
}

fn load(mpv: &Mpv, stream: &Stream, mode: &str, start: Option<f64>) -> Result<()> {
    let mut opts = format!("user-agent={}", escaped(&stream.user_agent));
    if let Some(t) = start {
        opts.push_str(&format!(",start={t:.1}"));
    }
    // mpv >= 0.38: loadfile <url> <flags> <index> <options>
    mpv.command("loadfile", &[&stream.url, mode, "-1", &opts])
        .map_err(|e| anyhow!("loadfile: {e}"))
}

fn run(
    rx: mpsc::Receiver<Command>,
    events: UnboundedSender<PlayerEvent>,
    ready: mpsc::Sender<Result<()>>,
) {
    let mpv = match init_mpv() {
        Ok(m) => m,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let ev = &mpv;
    let _ = ev.disable_deprecated_events();
    let _ = ev.observe_property("time-pos", Format::Double, 1);
    let _ = ev.observe_property("duration", Format::Double, 2);
    let _ = ev.observe_property("pause", Format::Flag, 3);
    let _ = ev.observe_property("playlist-pos", Format::Int64, 4);
    let _ = ev.observe_property("idle-active", Format::Flag, 5);
    let _ = ev.observe_property("paused-for-cache", Format::Flag, 6);
    let _ = ready.send(Ok(()));

    // Video ids in mpv playlist order. `playlist-clear` keeps only the current entry.
    let mut ids: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    let mut last_pos_sent = -1.0f64;
    // Data used: mpv's `cache-speed` (bytes per second arriving now), summed once a second.
    let mut speed_at = std::time::Instant::now();
    let mut downloaded = 0f64;

    loop {
        // Commands first, so a click is acted on within one poll interval.
        loop {
            match rx.try_recv() {
                Ok(cmd) => {
                    let res = match cmd {
                        Command::Play(s) => {
                            ids = vec![s.video_id.clone()];
                            current = Some(s.video_id.clone());
                            let _ = mpv.set_property("pause", false);
                            load(&mpv, &s, "replace", None)
                        }
                        Command::Reload(s, at) => {
                            ids = vec![s.video_id.clone()];
                            current = Some(s.video_id.clone());
                            load(&mpv, &s, "replace", Some(at))
                        }
                        Command::SetNext(s) => {
                            // Drop anything queued after the current entry, then append.
                            let _ = mpv.command("playlist-clear", &[]);
                            ids.retain(|id| Some(id) == current.as_ref());
                            ids.truncate(1);
                            ids.push(s.video_id.clone());
                            load(&mpv, &s, "append", None)
                        }
                        Command::SetPaused(p) => mpv
                            .set_property("pause", p)
                            .map_err(|e| anyhow!("{e}")),
                        Command::TogglePause => {
                            let p: bool = mpv.get_property("pause").unwrap_or(false);
                            mpv.set_property("pause", !p).map_err(|e| anyhow!("{e}"))
                        }
                        Command::Seek(secs) => mpv
                            .command("seek", &[&format!("{secs}"), "absolute"])
                            .map_err(|e| anyhow!("{e}")),
                        Command::Volume(v) => mpv
                            .set_property("volume", v.clamp(0.0, 100.0))
                            .map_err(|e| anyhow!("{e}")),
                        Command::Stop => {
                            ids.clear();
                            current = None;
                            mpv.command("stop", &[]).map_err(|e| anyhow!("{e}"))
                        }
                    };
                    if let Err(e) = res {
                        let _ = events.send(PlayerEvent::Error {
                            video_id: current.clone(),
                            message: e.to_string(),
                        });
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }

        let since = speed_at.elapsed().as_secs_f64();
        if since >= 1.0 {
            speed_at = std::time::Instant::now();
            let speed = mpv.get_property::<i64>("cache-speed").unwrap_or(0).max(0) as f64;
            downloaded += speed * since;
            if downloaded >= 1.0 {
                let bytes = downloaded as u64;
                let _ = events.send(PlayerEvent::Downloaded { bytes, rate: speed });
                downloaded = downloaded.fract();
            }
        }

        // Then events, waiting a short while so commands stay responsive.
        let Some(event) = ev.wait_event(0.02) else {
            continue;
        };
        match event {
            Ok(Event::PropertyChange { name, change, .. }) => match (name, change) {
                ("time-pos", PropertyData::Double(t)) => {
                    // Whole-second updates are plenty: the UI interpolates in between.
                    if (t - last_pos_sent).abs() >= 1.0 || t < last_pos_sent {
                        last_pos_sent = t.floor();
                        let _ = events.send(PlayerEvent::Position(t));
                    }
                }
                ("duration", PropertyData::Double(d)) => {
                    let _ = events.send(PlayerEvent::Duration(d));
                }
                ("pause", PropertyData::Flag(p)) => {
                    let _ = events.send(PlayerEvent::Paused(p));
                }
                // A stall at the very start is just the first buffering, not a slow connection.
                ("paused-for-cache", PropertyData::Flag(true)) if last_pos_sent >= 1.0 => {
                    let _ = events.send(PlayerEvent::Stalled);
                }
                ("idle-active", PropertyData::Flag(true)) if !ids.is_empty() => {
                    let _ = events.send(PlayerEvent::Idle);
                }
                ("playlist-pos", PropertyData::Int64(pos)) if pos >= 0 => {
                    if let Some(id) = ids.get(pos as usize) {
                        current = Some(id.clone());
                    }
                }
                _ => {}
            },
            Ok(Event::PlaybackRestart) => {
                if let Some(id) = &current {
                    let _ = events.send(PlayerEvent::Started { video_id: id.clone() });
                }
            }
            Ok(Event::StartFile) => {
                last_pos_sent = -1.0;
            }
            Ok(Event::EndFile(reason)) => {
                // mpv reports errors via the end-file reason; a natural EOF advances by itself.
                let reason = format!("{reason:?}");
                if reason.to_lowercase().contains("error") {
                    let _ = events.send(PlayerEvent::Error {
                        video_id: current.clone(),
                        message: reason,
                    });
                }
            }
            Ok(Event::Shutdown) => return,
            Ok(_) => {}
            Err(e) => log::warn!("mpv event error: {e}"),
        }
    }
}
