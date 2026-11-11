//! The engine: queue, stream cache, prefetch and radio autoplay on top of `Source` and `Player`.
//!
//! Rules from the plan:
//! - `play` returns as soon as mpv has a URL; radio and prefetch run in background tasks.
//! - The next track is resolved and handed to mpv ahead of time, so advancing is gapless.
//! - No lock is held across an `.await`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::Serialize;
use tokio::sync::{broadcast, mpsc};

use crate::player::{Command, Player, PlayerEvent};
use crate::source::{Quality, Source, Stream, Track};

/// Fetch more radio when fewer than this many tracks are left after the current one.
const RADIO_LOOKAHEAD: usize = 10;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Update {
    NowPlaying { track: Track, index: usize },
    Queue { tracks: Vec<Track>, index: usize },
    Position { secs: f64 },
    Duration { secs: f64 },
    Paused { paused: bool },
    /// Click to first audio, in ms. The number the whole app is judged by.
    Timing { video_id: String, ms: u64, cached: bool },
    Error { message: String },
}

struct State {
    queue: Vec<Track>,
    index: usize,
    autoplay: bool,
    /// When the user asked for the current track, for the timing log.
    requested_at: Option<(String, Instant, bool)>,
    /// Which video id mpv has queued after the current one.
    next_loaded: Option<String>,
    /// Radio already fetched for this seed.
    radio_seed: Option<String>,
    position: f64,
    retried: Option<String>,
}

struct Inner {
    source: Source,
    player: Player,
    state: Mutex<State>,
    streams: Mutex<HashMap<String, Stream>>,
    quality: AtomicU8,
    updates: broadcast::Sender<Update>,
    /// The runtime `start` ran in. Callers may be on threads without one (sync Tauri commands).
    rt: tokio::runtime::Handle,
}

#[derive(Clone)]
pub struct Engine(Arc<Inner>);

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn quality_from(v: u8) -> Quality {
    match v {
        0 => Quality::DataSaver,
        1 => Quality::Normal,
        2 => Quality::High,
        _ => Quality::Auto,
    }
}

impl Engine {
    /// Must be called inside a tokio runtime.
    pub fn start(source: Source) -> Result<Engine> {
        let (ptx, prx) = mpsc::unbounded_channel();
        let player = Player::spawn(ptx)?;
        let (updates, _) = broadcast::channel(256);
        let engine = Engine(Arc::new(Inner {
            source,
            player,
            state: Mutex::new(State {
                queue: vec![],
                index: 0,
                autoplay: true,
                requested_at: None,
                next_loaded: None,
                radio_seed: None,
                position: 0.0,
                retried: None,
            }),
            streams: Mutex::new(HashMap::new()),
            quality: AtomicU8::new(3),
            updates,
            rt: tokio::runtime::Handle::current(),
        }));
        let e = engine.clone();
        engine.0.rt.spawn(async move { e.player_events(prx).await });
        let e = engine.clone();
        engine.0.rt.spawn(async move { e.0.source.warm().await });
        Ok(engine)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Update> {
        self.0.updates.subscribe()
    }

    pub fn source(&self) -> &Source {
        &self.0.source
    }

    fn emit(&self, u: Update) {
        let _ = self.0.updates.send(u);
    }

    pub fn quality(&self) -> Quality {
        quality_from(self.0.quality.load(Ordering::Relaxed))
    }

    pub fn set_quality(&self, q: Quality) {
        let v = match q {
            Quality::DataSaver => 0,
            Quality::Normal => 1,
            Quality::High => 2,
            Quality::Auto => 3,
        };
        self.0.quality.store(v, Ordering::Relaxed);
        // Cached URLs were picked for the old quality.
        self.0.streams.lock().unwrap().clear();
    }

    pub fn set_autoplay(&self, on: bool) {
        self.0.state.lock().unwrap().autoplay = on;
    }

    /// A cached, unexpired stream URL, or resolve one now.
    async fn stream(&self, id: &str) -> Result<(Stream, bool)> {
        if let Some(s) = self.0.streams.lock().unwrap().get(id) {
            if s.expires_at - 120 > now_unix() {
                return Ok((s.clone(), true));
            }
        }
        let s = self.0.source.resolve(id, self.quality()).await?;
        self.0
            .streams
            .lock()
            .unwrap()
            .insert(id.to_string(), s.clone());
        Ok((s, false))
    }

    /// Resolve ahead of a likely click (hover). Fire and forget.
    pub fn prefetch(&self, id: String) {
        let e = self.clone();
        self.0.rt.spawn(async move {
            let _ = e.stream(&id).await;
        });
    }

    /// Replace the queue with `tracks` and play `start`.
    pub async fn play(&self, tracks: Vec<Track>, start: usize) -> Result<()> {
        if tracks.is_empty() {
            return Ok(());
        }
        let start = start.min(tracks.len() - 1);
        {
            let mut st = self.0.state.lock().unwrap();
            st.queue = tracks;
            st.index = start;
            st.radio_seed = None;
            st.next_loaded = None;
        }
        self.play_index(start).await
    }

    async fn play_index(&self, index: usize) -> Result<()> {
        let requested = Instant::now();
        let track = {
            let mut st = self.0.state.lock().unwrap();
            let Some(track) = st.queue.get(index).cloned() else {
                return Ok(());
            };
            st.index = index;
            st.next_loaded = None;
            st.position = 0.0;
            track
        };
        self.emit(Update::NowPlaying { track: track.clone(), index });
        let (stream, cached) = match self.stream(&track.id).await {
            Ok(s) => s,
            Err(e) => {
                self.emit(Update::Error {
                    message: format!("Couldn't play {}: {e}", track.title),
                });
                return Err(e);
            }
        };
        self.0.state.lock().unwrap().requested_at = Some((track.id.clone(), requested, cached));
        self.0.player.send(Command::Play(stream));
        self.after_track_change();
        Ok(())
    }

    /// Background work once a track is (about to be) playing: radio top-up and prefetch.
    fn after_track_change(&self) {
        let e = self.clone();
        self.0.rt.spawn(async move {
            e.top_up_radio().await;
            e.prefetch_next().await;
        });
    }

    async fn top_up_radio(&self) {
        let seed = {
            let st = self.0.state.lock().unwrap();
            let remaining = st.queue.len().saturating_sub(st.index + 1);
            let current = st.queue.get(st.index).map(|t| t.id.clone());
            if !st.autoplay || remaining >= RADIO_LOOKAHEAD || current == st.radio_seed {
                return;
            }
            current
        };
        let Some(seed) = seed else { return };
        match self.0.source.radio(&seed).await {
            Ok(tracks) => {
                let snapshot = {
                    let mut st = self.0.state.lock().unwrap();
                    st.radio_seed = Some(seed);
                    let known: std::collections::HashSet<String> =
                        st.queue.iter().map(|t| t.id.clone()).collect();
                    st.queue
                        .extend(tracks.into_iter().filter(|t| !known.contains(&t.id)));
                    (st.queue.clone(), st.index)
                };
                self.emit(Update::Queue { tracks: snapshot.0, index: snapshot.1 });
            }
            Err(e) => log::warn!("radio for {seed}: {e}"),
        }
    }

    async fn prefetch_next(&self) {
        let next = {
            let st = self.0.state.lock().unwrap();
            st.queue.get(st.index + 1).cloned()
        };
        let Some(next) = next else { return };
        match self.stream(&next.id).await {
            Ok((s, _)) => {
                let mut st = self.0.state.lock().unwrap();
                // Still the right next track?
                if st.queue.get(st.index + 1).map(|t| &t.id) == Some(&next.id) {
                    st.next_loaded = Some(next.id.clone());
                    self.0.player.send(Command::SetNext(s));
                }
            }
            Err(e) => log::warn!("prefetch {}: {e}", next.id),
        }
    }

    pub async fn next(&self) -> Result<()> {
        let idx = {
            let st = self.0.state.lock().unwrap();
            if st.index + 1 >= st.queue.len() {
                return Ok(());
            }
            st.index + 1
        };
        self.play_index(idx).await
    }

    pub async fn previous(&self) -> Result<()> {
        let (idx, restart) = {
            let st = self.0.state.lock().unwrap();
            (st.index.saturating_sub(1), st.position > 3.0 || st.index == 0)
        };
        if restart {
            self.0.player.send(Command::Seek(0.0));
            return Ok(());
        }
        self.play_index(idx).await
    }

    pub async fn jump(&self, index: usize) -> Result<()> {
        self.play_index(index).await
    }

    pub fn toggle_pause(&self) {
        self.0.player.send(Command::TogglePause);
    }

    pub fn seek(&self, secs: f64) {
        self.0.player.send(Command::Seek(secs));
    }

    pub fn set_volume(&self, v: f64) {
        self.0.player.send(Command::Volume(v));
    }

    pub fn queue(&self) -> (Vec<Track>, usize) {
        let st = self.0.state.lock().unwrap();
        (st.queue.clone(), st.index)
    }

    async fn player_events(&self, mut rx: mpsc::UnboundedReceiver<PlayerEvent>) {
        while let Some(ev) = rx.recv().await {
            match ev {
                PlayerEvent::Started { video_id } => self.on_started(video_id),
                PlayerEvent::Position(secs) => {
                    self.0.state.lock().unwrap().position = secs;
                    self.emit(Update::Position { secs });
                }
                PlayerEvent::Duration(secs) => self.emit(Update::Duration { secs }),
                PlayerEvent::Paused(paused) => self.emit(Update::Paused { paused }),
                PlayerEvent::Idle => {
                    // Reached the end with nothing prefetched: try the next item.
                    let has_next = {
                        let st = self.0.state.lock().unwrap();
                        st.index + 1 < st.queue.len() && st.requested_at.is_none()
                    };
                    if has_next {
                        let _ = self.next().await;
                    }
                }
                PlayerEvent::Error { video_id, message } => {
                    log::warn!("playback error {video_id:?}: {message}");
                    self.on_error(video_id).await;
                }
            }
        }
    }

    fn on_started(&self, video_id: String) {
        let (timing, advanced) = {
            let mut st = self.0.state.lock().unwrap();
            let timing = match st.requested_at.take() {
                Some((id, at, cached)) if id == video_id => {
                    Some((id, at.elapsed().as_millis() as u64, cached))
                }
                other => {
                    st.requested_at = other;
                    None
                }
            };
            // A gapless advance: mpv moved on to the prefetched entry by itself.
            let advanced = if st.queue.get(st.index).map(|t| &t.id) != Some(&video_id) {
                if let Some(i) = st.queue.iter().position(|t| t.id == video_id) {
                    st.index = i;
                    st.next_loaded = None;
                    Some((st.queue[i].clone(), i))
                } else {
                    None
                }
            } else {
                None
            };
            st.retried = None;
            (timing, advanced)
        };
        if let Some((video_id, ms, cached)) = timing {
            log::info!("click→sound {video_id}: {ms} ms (url cached: {cached})");
            self.emit(Update::Timing { video_id, ms, cached });
        }
        if let Some((track, index)) = advanced {
            self.emit(Update::NowPlaying { track, index });
            self.after_track_change();
        }
    }

    /// A stream failed: drop its cached URL and retry once, then skip.
    async fn on_error(&self, video_id: Option<String>) {
        let Some(id) = video_id else { return };
        self.0.streams.lock().unwrap().remove(&id);
        let (retry, index) = {
            let mut st = self.0.state.lock().unwrap();
            let retry = st.retried.as_deref() != Some(&id);
            st.retried = Some(id.clone());
            (retry, st.index)
        };
        if retry {
            let _ = self.play_index(index).await;
        } else {
            self.emit(Update::Error { message: "That song wouldn't play, skipping it.".into() });
            let _ = self.next().await;
        }
    }
}
