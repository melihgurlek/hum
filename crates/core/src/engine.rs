//! The engine: queue, stream cache, prefetch and radio autoplay on top of `Source` and `Player`.
//!
//! Rules from the plan:
//! - `play` returns as soon as mpv has a URL; radio and prefetch run in background tasks.
//! - The next track is resolved and handed to mpv ahead of time, so advancing is gapless.
//! - No lock is held across an `.await`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::Serialize;
use tokio::sync::{broadcast, mpsc};

use crate::library::{self, Library};
use crate::player::{Command, Player, PlayerEvent};
use crate::source::{Quality, Source, Stream, Track};

/// Fetch more radio when fewer than this many tracks are left after the current one.
const RADIO_LOOKAHEAD: usize = 10;

/// Auto steps down after this many stalls within `STALL_WINDOW`. One stall can be a blip.
const STALLS_TO_STEP_DOWN: usize = 2;
const STALL_WINDOW: Duration = Duration::from_secs(120);
/// Auto tries the best quality again after this long without a stall, doubled after every
/// step-down this session (so a connection that keeps failing High settles on Data saver),
/// up to `STEP_UP_MAX_WAIT`.
const STEP_UP_AFTER: Duration = Duration::from_secs(15 * 60);
const STEP_UP_MAX_WAIT: Duration = Duration::from_secs(2 * 60 * 60);
/// ... and only if a download recently ran at least this fast, in bytes per second: three times
/// what High needs (~130 kbps, ~16 KB/s). mpv fills its buffer in a burst at the connection's
/// full speed whenever a song starts, so even Data saver streams show what the line can do.
const STEP_UP_MIN_RATE: f64 = 48_000.0;
/// A speed measurement older than this says nothing about the connection now.
const RATE_FRESH: Duration = Duration::from_secs(5 * 60);
/// Downloaded bytes are written to the library in batches of at least this much.
const DATA_FLUSH_BYTES: u64 = 1 << 20;

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
    /// Auto quality stepped down (`low`) because the connection stalled, or back up.
    AutoQuality { low: bool },
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
    /// The play in progress, recorded to the library when the track changes.
    playing: Option<PlayRecord>,
}

struct PlayRecord {
    track: Track,
    started_at: i64,
    position: f64,
    duration: f64,
}

struct Inner {
    source: Source,
    player: Player,
    state: Mutex<State>,
    streams: Mutex<HashMap<String, Stream>>,
    quality: AtomicU8,
    auto: Mutex<AutoState>,
    /// Downloaded bytes not yet written to the library.
    data_pending: AtomicU64,
    updates: broadcast::Sender<Update>,
    /// The runtime `start` ran in. Callers may be on threads without one (sync Tauri commands).
    rt: tokio::runtime::Handle,
    library: Option<Library>,
}

/// What Auto quality has learned about the connection this session.
#[derive(Default)]
struct AutoState {
    /// When Auto stepped down to data-saver streams, if it has.
    low_since: Option<Instant>,
    stalls: Vec<Instant>,
    /// Step-downs this session, for the doubling wait.
    downs: u32,
    /// The fastest recent download speed and when it was seen.
    peak: Option<(Instant, f64)>,
}

impl AutoState {
    /// Record a stall; true when Auto should step down now.
    fn stall(&mut self, now: Instant, auto: bool) -> bool {
        self.stalls.retain(|t| now.duration_since(*t) < STALL_WINDOW);
        self.stalls.push(now);
        let step = auto && self.low_since.is_none() && self.stalls.len() >= STALLS_TO_STEP_DOWN;
        if step {
            self.low_since = Some(now);
            self.downs += 1;
            // Speeds seen before were evidently not enough.
            self.peak = None;
        }
        step
    }

    fn sample(&mut self, now: Instant, rate: f64) {
        let keep = self
            .peak
            .is_some_and(|(at, r)| r >= rate && now.duration_since(at) < RATE_FRESH);
        if !keep {
            self.peak = Some((now, rate));
        }
    }

    fn wait(&self) -> Duration {
        STEP_UP_AFTER
            .saturating_mul(1 << self.downs.saturating_sub(1).min(8))
            .min(STEP_UP_MAX_WAIT)
    }

    /// Calm for long enough, and the connection was recently measured fast enough for High.
    fn step_up_ready(&self, now: Instant) -> bool {
        let wait = self.wait();
        let calm = |t: &Instant| now.duration_since(*t) >= wait;
        let fast = self.peak.is_some_and(|(at, r)| {
            now.duration_since(at) < RATE_FRESH && r >= STEP_UP_MIN_RATE
        });
        self.low_since.as_ref().is_some_and(calm) && self.stalls.iter().all(calm) && fast
    }

    fn step_up(&mut self) {
        self.low_since = None;
        self.stalls.clear();
        self.peak = None;
    }
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
    pub fn start(source: Source, library: Option<Library>) -> Result<Engine> {
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
                playing: None,
            }),
            streams: Mutex::new(HashMap::new()),
            quality: AtomicU8::new(3),
            auto: Mutex::new(AutoState::default()),
            data_pending: AtomicU64::new(0),
            updates,
            rt: tokio::runtime::Handle::current(),
            library,
        }));
        let e = engine.clone();
        engine.0.rt.spawn(async move { e.player_events(prx).await });
        let e = engine.clone();
        engine.0.rt.spawn(async move { e.0.source.warm().await });
        Ok(engine)
    }

    pub fn library(&self) -> Option<&Library> {
        self.0.library.as_ref()
    }

    /// Record the play in progress, if any.
    fn finish_play(&self) {
        self.flush_data(0);
        let rec = self.0.state.lock().unwrap().playing.take();
        let (Some(rec), Some(lib)) = (rec, self.0.library.clone()) else { return };
        if rec.position < 1.0 {
            return; // never really started
        }
        self.0.rt.spawn(async move {
            let res = lib
                .run(move |l| l.record_play(&rec.track, rec.started_at, rec.position, rec.duration))
                .await;
            if let Err(e) = res {
                log::warn!("recording play: {e:#}");
            }
        });
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

    /// The quality streams are resolved at: Auto is High until the connection stalls.
    fn effective_quality(&self) -> Quality {
        match self.quality() {
            Quality::Auto if self.0.auto.lock().unwrap().low_since.is_some() => Quality::DataSaver,
            Quality::Auto | Quality::Normal => Quality::High,
            q => q,
        }
    }

    /// Whether Auto is currently on data-saver streams because of stalls.
    pub fn auto_low(&self) -> bool {
        self.0.auto.lock().unwrap().low_since.is_some()
    }

    pub fn set_quality(&self, q: Quality) {
        let v = match q {
            Quality::DataSaver => 0,
            Quality::Normal => 1,
            Quality::High => 2,
            Quality::Auto => 3,
        };
        let before = self.effective_quality();
        self.0.quality.store(v, Ordering::Relaxed);
        if self.effective_quality() != before {
            self.requality_current();
        }
    }

    /// The stream size changed: drop cached URLs and switch the current song over at the same
    /// spot, so a stuttering song gets better now rather than from the next one.
    fn requality_current(&self) {
        self.0.streams.lock().unwrap().clear();
        let (track, at) = {
            let mut st = self.0.state.lock().unwrap();
            if st.playing.is_none() {
                return;
            }
            st.next_loaded = None;
            (st.queue.get(st.index).cloned(), st.position)
        };
        let Some(track) = track else { return };
        let e = self.clone();
        self.0.rt.spawn(async move {
            match e.stream(&track.id).await {
                Ok((s, _)) => {
                    // Only if the user hasn't moved on while this resolved.
                    let still = e.current().is_some_and(|t| t.id == track.id);
                    if still {
                        let at = e.position().max(at);
                        e.0.player.send(Command::Reload(s, at));
                        e.prefetch_next().await;
                    }
                }
                Err(err) => log::warn!("switching {} to the new quality: {err:#}", track.id),
            }
        });
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
        let s = self.0.source.resolve(id, self.effective_quality()).await?;
        {
            // Every play and hover adds a URL (~2 KB); drop the ones that have expired.
            let mut streams = self.0.streams.lock().unwrap();
            let now = now_unix();
            streams.retain(|_, s| s.expires_at - 120 > now);
            streams.insert(id.to_string(), s.clone());
        }
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
        self.finish_play();
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
        self.maybe_step_up();
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
        let ranked = match (self.0.source.radio(&seed).await, self.0.library.clone()) {
            (Ok(tracks), Some(lib)) => lib.run(move |l| l.rank_radio(tracks)).await,
            (res, _) => res,
        };
        match ranked {
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

    pub fn set_paused(&self, paused: bool) {
        self.0.player.send(Command::SetPaused(paused));
    }

    /// Last reported position, in seconds.
    pub fn position(&self) -> f64 {
        self.0.state.lock().unwrap().position
    }

    pub fn current(&self) -> Option<Track> {
        let st = self.0.state.lock().unwrap();
        st.queue.get(st.index).cloned()
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
                    {
                        let mut st = self.0.state.lock().unwrap();
                        st.position = secs;
                        if let Some(rec) = st.playing.as_mut() {
                            rec.position = secs;
                        }
                    }
                    self.emit(Update::Position { secs });
                }
                PlayerEvent::Duration(secs) => {
                    if let Some(rec) = self.0.state.lock().unwrap().playing.as_mut() {
                        rec.duration = secs;
                    }
                    self.emit(Update::Duration { secs });
                }
                PlayerEvent::Paused(paused) => self.emit(Update::Paused { paused }),
                PlayerEvent::Stalled => self.on_stall().await,
                PlayerEvent::Downloaded { bytes, rate } => {
                    self.0.auto.lock().unwrap().sample(Instant::now(), rate);
                    let pending = self.0.data_pending.fetch_add(bytes, Ordering::Relaxed) + bytes;
                    if pending >= DATA_FLUSH_BYTES {
                        self.flush_data(DATA_FLUSH_BYTES);
                    }
                }
                PlayerEvent::Idle => {
                    self.finish_play();
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
        let same = self
            .0
            .state
            .lock()
            .unwrap()
            .playing
            .as_ref()
            .is_some_and(|r| r.track.id == video_id);
        if !same {
            self.finish_play();
        }
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
            if !same {
                if let Some(track) = st.queue.iter().find(|t| t.id == video_id).cloned() {
                    st.playing = Some(PlayRecord {
                        duration: track.duration.unwrap_or(0) as f64,
                        track,
                        started_at: library::now(),
                        position: 0.0,
                    });
                }
            }
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

    /// Write downloaded bytes to the library once at least `min` are pending.
    fn flush_data(&self, min: u64) {
        let Some(lib) = self.0.library.clone() else { return };
        if self.0.data_pending.load(Ordering::Relaxed) < min.max(1) {
            return;
        }
        let bytes = self.0.data_pending.swap(0, Ordering::Relaxed);
        self.0.rt.spawn(async move {
            if let Err(e) = lib.run(move |l| l.add_data_used(bytes)).await {
                log::warn!("recording data used: {e:#}");
            }
        });
    }

    /// Playback waited for the network. On Auto, the second stall in a short while switches to
    /// data-saver streams, starting with the current song at the same spot.
    async fn on_stall(&self) {
        let is_auto = self.quality() == Quality::Auto;
        let (step_down, wait) = {
            let mut auto = self.0.auto.lock().unwrap();
            let step = auto.stall(Instant::now(), is_auto);
            log::info!("stalled for cache ({} recently)", auto.stalls.len());
            (step, auto.wait())
        };
        if !step_down {
            return;
        }
        log::info!("auto quality: stepping down to data saver, next try at High in {wait:?} at the earliest");
        self.emit(Update::AutoQuality { low: true });
        self.requality_current();
    }

    /// After a long enough calm on data-saver streams, and with the connection measured fast
    /// enough, Auto tries the best quality again from the next song (never mid-song).
    fn maybe_step_up(&self) {
        let mut auto = self.0.auto.lock().unwrap();
        if auto.step_up_ready(Instant::now()) {
            auto.step_up();
            drop(auto);
            log::info!("auto quality: trying the best quality again");
            self.0.streams.lock().unwrap().clear();
            self.emit(Update::AutoQuality { low: false });
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

#[cfg(test)]
mod tests {
    use super::*;

    const S: Duration = Duration::from_secs(1);
    const MIN: Duration = Duration::from_secs(60);

    #[test]
    fn steps_down_on_second_stall_in_window_only_on_auto() {
        let t = Instant::now();
        let mut a = AutoState::default();
        assert!(!a.stall(t, true));
        assert!(!a.stall(t + 3 * MIN, true), "first stall aged out of the window");
        assert!(a.stall(t + 4 * MIN, true));
        let mut manual = AutoState::default();
        assert!(!manual.stall(t, false) && !manual.stall(t + S, false));
    }

    #[test]
    fn steps_up_only_when_calm_and_measured_fast() {
        let t = Instant::now();
        let mut a = AutoState::default();
        a.stall(t, true);
        a.stall(t + S, true);
        let later = t + 16 * MIN;
        assert!(!a.step_up_ready(later), "no speed measured yet");
        a.sample(later - MIN, 20_000.0);
        assert!(!a.step_up_ready(later), "too slow for High");
        a.sample(later - MIN, 300_000.0);
        a.sample(later - S, 16_000.0); // a slower reading doesn't hide the recent burst
        assert!(a.step_up_ready(later));
        assert!(!a.step_up_ready(t + 10 * MIN), "not calm long enough");
        a.sample(later + 10 * MIN, 16_000.0);
        assert!(!a.step_up_ready(later + 10 * MIN), "the fast reading went stale");
    }

    #[test]
    fn wait_doubles_with_each_step_down_and_caps() {
        let t = Instant::now();
        let mut a = AutoState::default();
        let mut waits = vec![];
        for i in 0..5u32 {
            let at = t + Duration::from_secs(i as u64 * 10_000);
            a.stall(at, true);
            a.stall(at + S, true);
            waits.push(a.wait().as_secs() / 60);
            a.step_up();
        }
        assert_eq!(waits, [15, 30, 60, 120, 120]);
    }
}
