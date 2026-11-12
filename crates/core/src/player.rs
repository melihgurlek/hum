//! Audio playback through libmpv, on a dedicated thread that owns the event loop.
//!
//! The loop sleeps in `wait_event` until mpv has something to say; a command sent from another
//! thread interrupts that sleep with `mpv_wakeup`. So an idle player costs no wakeups at all,
//! and a playing one wakes for mpv's own events plus once a second for the data counter.
//!
//! mpv's own playlist holds the current track plus the prefetched next one, so advancing is
//! gapless. Our side tracks which video id sits at which mpv playlist position.

use std::sync::{Arc, mpsc};
use std::thread;

use anyhow::{Result, anyhow};
use libmpv2::Mpv;
use std::collections::{HashMap, HashSet};

use libmpv2::events::{Event, PropertyData};
use libmpv2::Format;
use tokio::sync::mpsc::UnboundedSender;

use crate::source::Stream;

pub use crate::playback::{Command, PlayerEvent};

pub struct Player {
    tx: mpsc::Sender<Command>,
    /// Shared only to call `mpv_wakeup`, which mpv documents as safe from any thread. The Arc
    /// keeps the handle alive for as long as either side can touch it.
    mpv: Arc<Mpv>,
}

impl Player {
    pub fn spawn(events: UnboundedSender<PlayerEvent>) -> Result<Player> {
        let (tx, rx) = mpsc::channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<Arc<Mpv>>>();
        thread::Builder::new()
            .name("mpv".into())
            .spawn(move || run(rx, events, ready_tx))?;
        let mpv = ready_rx.recv().map_err(|_| anyhow!("mpv thread died"))??;
        Ok(Player { tx, mpv })
    }

    pub fn send(&self, cmd: Command) {
        if self.tx.send(cmd).is_ok() {
            // SAFETY: the handle is alive (we hold an Arc) and mpv_wakeup is thread-safe.
            unsafe { libmpv2_sys::mpv_wakeup(self.mpv.ctx.as_ptr()) };
        }
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
        // 20 s ahead rides out a network hiccup; more just wastes data when a song is skipped
        // (60 s was ~1 MB per early skip at High). Auto quality handles a slow connection.
        init.set_option("cache-secs", 20i64)?;
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
    if let Some(g) = stream.gain_db {
        opts.push_str(&format!(",volume-gain={g:.2}"));
    }
    // Only a play from the start can record the whole song.
    if let (Some(path), None) = (&stream.record_to, start) {
        opts.push_str(&format!(",stream-record={}", escaped(&path.to_string_lossy())));
    }
    // mpv >= 0.38: loadfile <url> <flags> <index> <options>
    mpv.command("loadfile", &[&stream.url, mode, "-1", &opts])
        .map_err(|e| anyhow!("loadfile: {e}"))
}

/// A song-cache recording: path, the stream's size, and whether it's still whole (no seek).
type Recording = (std::path::PathBuf, Option<u64>, bool);

/// Note what a stream about to load means for the cache: on disk or not, recording or not.
fn track(s: &Stream, start: Option<f64>, recs: &mut HashMap<String, Recording>, local: &mut HashSet<String>) {
    if s.local {
        local.insert(s.video_id.clone());
    } else {
        local.remove(&s.video_id);
    }
    if let Some((old, _, _)) = recs.remove(&s.video_id) {
        let _ = std::fs::remove_file(old);
    }
    if let (Some(p), None) = (&s.record_to, start) {
        recs.insert(s.video_id.clone(), (p.clone(), s.content_length, true));
    }
}

fn run(
    rx: mpsc::Receiver<Command>,
    events: UnboundedSender<PlayerEvent>,
    ready: mpsc::Sender<Result<Arc<Mpv>>>,
) {
    let mpv = match init_mpv() {
        Ok(m) => Arc::new(m),
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let ev = &*mpv;
    let _ = ev.disable_deprecated_events();
    let _ = ev.observe_property("time-pos", Format::Double, 1);
    let _ = ev.observe_property("duration", Format::Double, 2);
    let _ = ev.observe_property("pause", Format::Flag, 3);
    let _ = ev.observe_property("playlist-pos", Format::Int64, 4);
    let _ = ev.observe_property("idle-active", Format::Flag, 5);
    let _ = ev.observe_property("paused-for-cache", Format::Flag, 6);
    let _ = ready.send(Ok(mpv.clone()));

    // Video ids in mpv playlist order. `playlist-clear` keeps only the current entry.
    let mut ids: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    let mut last_pos_sent = -1.0f64;
    // Data used: mpv's `cache-speed` (bytes per second arriving now), summed once a second.
    let mut speed_at = std::time::Instant::now();
    let mut downloaded = 0f64;
    // Whether audio is moving, so the loop knows to wake each second for the data counter.
    let mut paused = false;
    let mut idle = true;
    // Song-cache recordings by video id: path, expected size, and whether it's still whole.
    let mut recordings: HashMap<String, Recording> = HashMap::new();
    let mut local: HashSet<String> = HashSet::new();

    loop {
        // Commands first, so a click is acted on within one poll interval.
        loop {
            match rx.try_recv() {
                Ok(cmd) => {
                    let res = match cmd {
                        Command::Play(s) => {
                            track(&s, None, &mut recordings, &mut local);
                            ids = vec![s.video_id.clone()];
                            current = Some(s.video_id.clone());
                            let _ = mpv.set_property("pause", false);
                            load(&mpv, &s, "replace", None)
                        }
                        Command::Reload(s, at) => {
                            track(&s, Some(at), &mut recordings, &mut local);
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
                            track(&s, None, &mut recordings, &mut local);
                            load(&mpv, &s, "append", None)
                        }
                        Command::ClearNext => {
                            ids.retain(|id| Some(id) == current.as_ref());
                            ids.truncate(1);
                            mpv.command("playlist-clear", &[]).map_err(|e| anyhow!("{e}"))
                        }
                        Command::SetPaused(p) => mpv
                            .set_property("pause", p)
                            .map_err(|e| anyhow!("{e}")),
                        Command::TogglePause => {
                            let p: bool = mpv.get_property("pause").unwrap_or(false);
                            mpv.set_property("pause", !p).map_err(|e| anyhow!("{e}"))
                        }
                        Command::Seek(secs) => {
                            if let Some(r) = current.as_ref().and_then(|id| recordings.get_mut(id)) {
                                r.2 = false;
                            }
                            mpv
                                .command("seek", &[&format!("{secs}"), "absolute"])
                                .map_err(|e| anyhow!("{e}"))
                        }
                        Command::Volume(v) => mpv
                            .set_property("volume", v.clamp(0.0, 100.0))
                            .map_err(|e| anyhow!("{e}")),
                        Command::QueryNetwork => Ok(()),
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
            // A cached song reads from disk: no data used, and no speed worth learning from.
            let on_disk = current.as_ref().is_some_and(|id| local.contains(id));
            let speed = match on_disk {
                true => 0.0,
                false => mpv.get_property::<i64>("cache-speed").unwrap_or(0).max(0) as f64,
            };
            downloaded += speed * since;
            if downloaded >= 1.0 {
                let bytes = downloaded as u64;
                let _ = events.send(PlayerEvent::Downloaded { bytes, rate: speed });
                downloaded = downloaded.fract();
            }
        }

        // Then sleep until mpv has an event or a command arrives (`send` wakes us). While audio
        // is moving, also wake once a second to sample the download speed.
        let timeout = if idle || paused { -1.0 } else { 1.0 };
        let Some(event) = ev.wait_event(timeout) else {
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
                    paused = p;
                    let _ = events.send(PlayerEvent::Paused(p));
                }
                // A stall at the very start is just the first buffering, not a slow connection.
                ("paused-for-cache", PropertyData::Flag(true)) if last_pos_sent >= 1.0 => {
                    let _ = events.send(PlayerEvent::Stalled);
                }
                ("idle-active", PropertyData::Flag(active)) => {
                    idle = active;
                    if active && !ids.is_empty() {
                        let _ = events.send(PlayerEvent::Idle);
                    }
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
                let ended = current.as_ref().and_then(|id| recordings.remove(id));
                if let Some((path, expected, whole)) = ended {
                    // 0 is mpv's MPV_END_FILE_REASON_EOF: played to the end.
                    if whole && reason == 0 {
                        let _ = events.send(PlayerEvent::Recorded { path, expected });
                    } else {
                        let _ = std::fs::remove_file(path);
                    }
                }
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
