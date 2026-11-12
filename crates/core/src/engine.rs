//! The engine: queue, stream cache, prefetch and radio autoplay on top of `Source` and `Player`.
//!
//! Rules from the plan:
//! - `play` returns as soon as mpv has a URL; radio and prefetch run in background tasks.
//! - The next track is resolved and handed to mpv ahead of time, so advancing is gapless.
//! - No lock is held across an `.await`.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};

use crate::cache::SongCache;
use crate::downloads::{DownloadStatus, Downloads};
use crate::library::{self, Library};
use crate::player::{Command, Player, PlayerEvent};
use crate::source::{Quality, Source, Stream, Track};

/// How long a download pass waits after the network fails.
const DOWNLOAD_RETRY: Duration = Duration::from_secs(60);

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
    /// `message` says what broke in plain words; `details` is a report to copy into a bug
    /// report (song, quality, the full error), when there is one.
    Error { message: String, details: Option<String> },
    /// Auto quality stepped down (`low`) because the connection stalled, or back up.
    AutoQuality { low: bool },
}

/// What plays after the current song: the next one (`Off`), the next one and the first again
/// after the last (`All`), or this one again (`One`). Skipping still moves on under `One`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

struct State {
    queue: Vec<Track>,
    index: usize,
    autoplay: bool,
    repeat: Repeat,
    shuffle: bool,
    /// The queue's order before shuffle mixed it, so turning shuffle off puts it back.
    unshuffled: Option<Vec<Track>>,
    /// mpv has the current song queued again after itself (repeat one), so its next start is a
    /// new play rather than the same one.
    repeat_queued: bool,
    /// When the user asked for the current track, for the timing log.
    requested_at: Option<(String, Instant, bool)>,
    /// Which video id mpv has queued after the current one.
    next_loaded: Option<String>,
    /// Radio already fetched for this seed.
    radio_seed: Option<String>,
    position: f64,
    retried: Option<String>,
    /// Restored from the last session and not loaded yet: play starts the current track here.
    resume_at: Option<f64>,
    /// The play in progress, recorded to the library when the track changes.
    playing: Option<PlayRecord>,
}

impl State {
    /// Mix the songs after the current one, remembering the order they had.
    fn shuffle_upcoming(&mut self) {
        self.unshuffled = Some(self.queue.clone());
        if self.index + 1 < self.queue.len() {
            fastrand::shuffle(&mut self.queue[self.index + 1..]);
        }
    }

    /// Put the remembered order back, keeping the current song current. Songs dropped since
    /// (a blocked artist's) stay out; songs added since (radio) follow in their order.
    fn unshuffle(&mut self) {
        let Some(original) = self.unshuffled.take() else { return };
        let current = self.queue.get(self.index).map(|t| t.id.clone());
        let now: std::collections::HashSet<&str> = self.queue.iter().map(|t| t.id.as_str()).collect();
        let was: std::collections::HashSet<&str> = original.iter().map(|t| t.id.as_str()).collect();
        let added: Vec<Track> = self.queue.iter().filter(|t| !was.contains(t.id.as_str())).cloned().collect();
        let mut queue: Vec<Track> = original.into_iter().filter(|t| now.contains(t.id.as_str())).collect();
        queue.extend(added);
        self.index = current.and_then(|id| queue.iter().position(|t| t.id == id)).unwrap_or(0);
        self.queue = queue;
    }

    /// The index that plays when the current song ends, under the repeat mode.
    fn upcoming(&self) -> Option<usize> {
        match self.repeat {
            Repeat::One if self.index < self.queue.len() => Some(self.index),
            _ => self.after_current(),
        }
    }

    /// The index Next goes to: the following song, or the first after the last on repeat all.
    fn after_current(&self) -> Option<usize> {
        if self.index + 1 < self.queue.len() {
            Some(self.index + 1)
        } else if self.repeat != Repeat::Off && !self.queue.is_empty() {
            Some(0)
        } else {
            None
        }
    }
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
    /// Lookups in progress, shared: a tap that lands while the touch-down's prefetch is still
    /// resolving the same song waits for it instead of asking YouTube again.
    resolving: Mutex<HashMap<String, Arc<tokio::sync::OnceCell<Result<Stream, Arc<anyhow::Error>>>>>>,
    quality: AtomicU8,
    auto: Mutex<AutoState>,
    /// Downloaded bytes not yet written to the library.
    data_pending: AtomicU64,
    updates: broadcast::Sender<Update>,
    /// The runtime `start` ran in. Callers may be on threads without one (sync Tauri commands).
    rt: tokio::runtime::Handle,
    library: Option<Library>,
    songs: std::sync::RwLock<Option<Arc<crate::cache::SongCache>>>,
    downloads: std::sync::RwLock<Option<Arc<Downloads>>>,
    /// The download pass: whether one runs, and whether to go round again when it ends.
    pass: Mutex<Pass>,
    /// Wakes a pass waiting to retry the network.
    pass_wake: tokio::sync::Notify,
    /// On mobile data (Android reports it; assumed until it does, so nothing downloads on
    /// mobile data by surprise). Never set on the desktop.
    metered: std::sync::atomic::AtomicBool,
    /// Downloads may use mobile data (off unless turned on in Settings).
    metered_downloads: std::sync::atomic::AtomicBool,
    /// Loudness normalization (on unless turned off in Settings).
    normalize: std::sync::atomic::AtomicBool,
}

#[derive(Default)]
struct Pass {
    running: bool,
    again: bool,
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

/// A copy of the song on disk: kept for offline first, then the song cache.
fn local_copy(downloads: Option<&Downloads>, songs: Option<&SongCache>, id: &str, quality: Quality) -> Option<Stream> {
    downloads.and_then(|d| d.lookup(id)).or_else(|| songs.and_then(|c| c.lookup(id, quality)))
}

/// A failure of the connection, as opposed to the song (unavailable, refused).
fn is_network_error(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        c.is::<reqwest::Error>()
            || c.is::<tokio::time::error::Elapsed>()
            || c.to_string().contains("did not answer")
    })
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
                repeat: Repeat::Off,
                shuffle: false,
                unshuffled: None,
                repeat_queued: false,
                requested_at: None,
                next_loaded: None,
                resume_at: None,
                radio_seed: None,
                position: 0.0,
                retried: None,
                playing: None,
            }),
            streams: Mutex::new(HashMap::new()),
            resolving: Mutex::new(HashMap::new()),
            quality: AtomicU8::new(3),
            auto: Mutex::new(AutoState::default()),
            normalize: std::sync::atomic::AtomicBool::new(true),
            data_pending: AtomicU64::new(0),
            updates,
            rt: tokio::runtime::Handle::current(),
            library,
            songs: Default::default(),
            downloads: Default::default(),
            pass: Default::default(),
            pass_wake: Default::default(),
            metered: std::sync::atomic::AtomicBool::new(cfg!(target_os = "android")),
            metered_downloads: std::sync::atomic::AtomicBool::new(false),
        }));
        let e = engine.clone();
        engine.0.rt.spawn(async move { e.player_events(prx).await });
        let e = engine.clone();
        engine.0.rt.spawn(async move { e.0.source.warm().await });
        Ok(engine)
    }

    /// Keep fully played songs on disk and play them from there.
    pub fn set_song_cache(&self, cache: crate::cache::SongCache) {
        *self.0.songs.write().unwrap() = Some(Arc::new(cache));
    }

    pub fn song_cache(&self) -> Option<Arc<crate::cache::SongCache>> {
        self.0.songs.read().unwrap().clone()
    }

    /// How many bytes of songs to keep after playing them; 0 stops keeping new ones (what is
    /// kept still plays). A lower limit trims at once.
    pub fn set_song_cache_limit(&self, bytes: u64) {
        let Some(cache) = self.song_cache() else { return };
        self.0.rt.spawn_blocking(move || cache.set_cap(bytes));
    }

    /// Where songs kept for offline live. Starts a download pass.
    pub fn set_downloads(&self, downloads: Downloads) {
        *self.0.downloads.write().unwrap() = Some(Arc::new(downloads));
        self.sync_downloads();
    }

    pub fn downloads(&self) -> Option<Arc<Downloads>> {
        self.0.downloads.read().unwrap().clone()
    }

    /// Keep (or stop keeping) a playlist (`playlist:<id>`) or `liked` for offline.
    pub async fn set_download(&self, target: String, on: bool) -> Result<()> {
        let lib = self.library().ok_or_else(|| anyhow::anyhow!("library unavailable"))?;
        lib.run(move |l| l.set_download(&target, on)).await?;
        self.sync_downloads();
        Ok(())
    }

    /// Stop keeping anything for offline and delete the files.
    pub async fn remove_all_downloads(&self) -> Result<()> {
        let lib = self.library().ok_or_else(|| anyhow::anyhow!("library unavailable"))?;
        lib.run(|l| l.clear_downloads()).await?;
        if let Some(d) = self.downloads() {
            tokio::task::spawn_blocking(move || d.clear()).await?;
        }
        // A song in flight lands first; the pass's next sweep removes it.
        self.sync_downloads();
        Ok(())
    }

    pub async fn download_status(&self) -> Result<DownloadStatus> {
        let lib = self.library().ok_or_else(|| anyhow::anyhow!("library unavailable"))?;
        let sets = lib.run(|l| l.download_sets()).await?;
        let d = self.downloads().ok_or_else(|| anyhow::anyhow!("downloads unavailable"))?;
        let active = self.0.pass.lock().unwrap().running;
        let held = self.held_for_wifi();
        let mut s = tokio::task::spawn_blocking(move || DownloadStatus::new(sets, &d, active)).await?;
        s.waiting_for_wifi = held && s.targets.iter().any(|t| t.done < t.total);
        Ok(s)
    }

    /// Downloads may use mobile data. Turning it on starts any that were waiting.
    pub fn set_metered_downloads(&self, on: bool) {
        self.0.metered_downloads.store(on, Ordering::Relaxed);
        self.sync_downloads();
    }

    fn held_for_wifi(&self) -> bool {
        self.0.metered.load(Ordering::Relaxed) && !self.0.metered_downloads.load(Ordering::Relaxed)
    }

    /// Bring the downloads in line with what is marked: delete what no collection holds, fetch
    /// what is missing. Cheap to call; one pass runs at a time and a call during a pass makes it
    /// go round once more.
    pub fn sync_downloads(&self) {
        // Ask the player what network this is (Android answers with a Network event, which
        // calls back here when it changes).
        self.0.player.send(Command::QueryNetwork);
        {
            let mut pass = self.0.pass.lock().unwrap();
            if pass.running {
                pass.again = true;
                self.0.pass_wake.notify_one();
                return;
            }
            pass.running = true;
        }
        let e = self.clone();
        self.0.rt.spawn(async move { e.download_passes().await });
    }

    async fn download_passes(&self) {
        loop {
            self.0.pass.lock().unwrap().again = false;
            let offline = self.download_pass().await;
            {
                // Under the lock, so a trigger can't slip in between the check and the exit.
                let mut pass = self.0.pass.lock().unwrap();
                if pass.again {
                    continue;
                }
                if !offline {
                    pass.running = false;
                    return;
                }
            }
            // No network: try again in a minute, or sooner if something changed.
            let _ = tokio::time::timeout(DOWNLOAD_RETRY, self.0.pass_wake.notified()).await;
        }
    }

    /// One go over the wanted songs. True if it stopped because the network failed.
    async fn download_pass(&self) -> bool {
        let (Some(lib), Some(dl)) = (self.library().cloned(), self.downloads()) else { return false };
        let mut failures: HashMap<String, u8> = HashMap::new();
        loop {
            // Asked again each round, so a collection unmarked meanwhile stops the pass.
            let wanted = match lib.run(|l| l.download_wanted()).await {
                Ok(w) => w,
                Err(e) => {
                    log::warn!("downloads: {e:#}");
                    return false;
                }
            };
            let set: HashSet<String> = wanted.iter().cloned().collect();
            let d = dl.clone();
            let Ok(have) = tokio::task::spawn_blocking(move || {
                d.sweep(&set);
                d.have()
            })
            .await
            else {
                return false;
            };
            let next = wanted.iter().find(|id| !have.contains(*id) && failures.get(*id).copied().unwrap_or(0) < 2);
            let Some(id) = next else { return false };
            // On mobile data without leave: wait (a switch to Wi-Fi wakes the pass).
            if self.held_for_wifi() {
                return true;
            }
            match self.download_song(&dl, id).await {
                Ok(()) => {
                    // Its covers too, so Now Playing and the lock screen show them offline.
                    if let Ok(Some(t)) = lib.run({
                        let id = id.clone();
                        move |l| l.track(&id)
                    })
                    .await
                    {
                        for url in [&t.cover, &t.thumb].into_iter().flatten() {
                            crate::source::images::fetch(url).await;
                        }
                    }
                }
                Err(e) if is_network_error(&e) => {
                    log::warn!("downloads: {id}: {e:#}; trying again in a minute");
                    return true;
                }
                Err(e) => {
                    log::warn!("downloads: {id}: {e:#}");
                    *failures.entry(id.clone()).or_default() += 1;
                }
            }
        }
    }

    /// A copy from the song cache, else a fetch. Whole file or none.
    async fn download_song(&self, dl: &Arc<Downloads>, id: &str) -> Result<()> {
        if let Some(s) = self.song_cache().and_then(|c| c.lookup(id, Quality::High)) {
            let (d, id, from) = (dl.clone(), id.to_string(), s.url);
            tokio::task::spawn_blocking(move || d.adopt(Path::new(&from), &id, s.itag)).await??;
            return Ok(());
        }
        let (format, mut body) = self.0.source.open_audio(id).await?;
        let part = dl.part_path(id, format.itag);
        let res = self.write_body(&part, &mut body, format.content_length).await;
        self.flush_data(1);
        match res {
            Ok(()) => {
                let (d, part) = (dl.clone(), part.clone());
                tokio::task::spawn_blocking(move || d.finish(&part)).await?
            }
            Err(e) => {
                let _ = tokio::fs::remove_file(&part).await;
                Err(e)
            }
        }
    }

    async fn write_body(&self, part: &Path, body: &mut ytmusic::AudioStream, expected: Option<u64>) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::File::create(part).await?;
        let mut got = 0u64;
        while let Some(chunk) = body.chunk().await? {
            file.write_all(&chunk).await?;
            got += chunk.len() as u64;
            // Counted as it comes, so a song that fails partway still shows in Data used.
            let pending = self.0.data_pending.fetch_add(chunk.len() as u64, Ordering::Relaxed) + chunk.len() as u64;
            if pending >= DATA_FLUSH_BYTES {
                self.flush_data(DATA_FLUSH_BYTES);
            }
        }
        file.flush().await?;
        anyhow::ensure!(expected.is_none_or(|e| e == got), "got {got} of {expected:?} bytes");
        Ok(())
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
                Ok((mut s, _)) => {
                    s.meta = Some((&track).into());
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
        let was = std::mem::replace(&mut self.0.state.lock().unwrap().autoplay, on);
        // Turned on mid-song: fetch the radio now, not at the next song (with the queue at its
        // end there may be no next song to wait for).
        if on && !was {
            let e = self.clone();
            self.0.rt.spawn(async move {
                e.top_up_radio().await;
                e.prefetch_next().await;
            });
        }
    }

    /// Shuffle on mixes the songs after this one (and those of every new queue); off puts
    /// the order back.
    pub fn set_shuffle(&self, on: bool) {
        let (tracks, index) = {
            let mut st = self.0.state.lock().unwrap();
            if st.shuffle == on {
                return;
            }
            st.shuffle = on;
            if on {
                st.shuffle_upcoming();
            } else {
                st.unshuffle();
            }
            st.repeat_queued = false;
            if st.next_loaded.take().is_some() {
                self.0.player.send(Command::ClearNext);
            }
            (st.queue.clone(), st.index)
        };
        self.emit(Update::Queue { tracks, index });
        let e = self.clone();
        self.0.rt.spawn(async move { e.prefetch_next().await });
    }

    /// Change what plays after this song, and re-queue mpv's next entry to match.
    pub fn set_repeat(&self, repeat: Repeat) {
        {
            let mut st = self.0.state.lock().unwrap();
            if st.repeat == repeat {
                return;
            }
            st.repeat = repeat;
            st.repeat_queued = false;
            if st.next_loaded.take().is_some() {
                self.0.player.send(Command::ClearNext);
            }
        }
        let e = self.clone();
        self.0.rt.spawn(async move { e.prefetch_next().await });
    }

    /// The song from disk (downloads, then the song cache), a cached unexpired stream URL, or one resolved now. A network
    /// stream records itself for the song cache.
    async fn stream(&self, id: &str) -> Result<(Stream, bool)> {
        let songs = self.song_cache();
        let local = local_copy(self.downloads().as_deref(), songs.as_deref(), id, self.effective_quality());
        let (mut s, cached) = match local {
            Some(s) => (s, true),
            None => {
                let (mut s, cached) = self.url(id).await?;
                s.record_to = songs.and_then(|c| c.record_path(&s));
                (s, cached)
            }
        };
        if let Some(lib) = self.library() {
            match s.loudness_db {
                Some(db) if !s.local => {
                    let _ = lib.set_loudness(id, db);
                }
                None => s.loudness_db = lib.loudness(id).ok().flatten(),
                _ => {}
            }
        }
        s.gain_db = self.gain_for(s.loudness_db);
        Ok((s, cached))
    }

    /// Loudness normalization as YouTube does it: a song louder than the reference is turned
    /// down by the difference; a quieter one is left alone (turning it up could clip).
    fn gain_for(&self, loudness_db: Option<f32>) -> Option<f64> {
        if !self.0.normalize.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        loudness_db.filter(|db| *db > 0.0).map(|db| -(db as f64))
    }

    /// Even out loudness between songs, from the next song on.
    pub fn set_normalize(&self, on: bool) {
        self.0.normalize.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    async fn url(&self, id: &str) -> Result<(Stream, bool)> {
        if let Some(s) = self.0.streams.lock().unwrap().get(id) {
            if s.expires_at - 120 > now_unix() {
                return Ok((s.clone(), true));
            }
        }
        let cell = self.0.resolving.lock().unwrap().entry(id.to_string()).or_default().clone();
        let quality = self.effective_quality();
        let res = cell
            .get_or_init(|| async { self.0.source.resolve(id, quality).await.map_err(Arc::new) })
            .await
            .clone();
        {
            let mut resolving = self.0.resolving.lock().unwrap();
            if resolving.get(id).is_some_and(|c| Arc::ptr_eq(c, &cell)) {
                resolving.remove(id);
            }
        }
        let s = res.map_err(|e| anyhow::anyhow!("{e:#}"))?;
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
        let mut start = start.min(tracks.len() - 1);
        {
            let mut st = self.0.state.lock().unwrap();
            st.radio_seed = None;
            st.next_loaded = None;
            st.unshuffled = None;
            if st.shuffle {
                // The picked song first, then all the others mixed, the ones above it too.
                let mut queue = tracks.clone();
                let picked = queue.remove(start);
                queue.insert(0, picked);
                st.queue = queue;
                st.index = 0;
                st.shuffle_upcoming();
                st.unshuffled = Some(tracks);
                start = 0;
            } else {
                st.queue = tracks;
                st.index = start;
            }
        }
        if self.0.state.lock().unwrap().shuffle {
            let (tracks, index) = self.queue();
            self.emit(Update::Queue { tracks, index });
        }
        self.play_index(start).await
    }

    async fn play_index(&self, index: usize) -> Result<()> {
        self.play_from(index, None).await
    }

    /// Put back the last session's queue, paused at `position` in the current track. Nothing
    /// loads until play is pressed, so a launch costs no requests.
    pub fn restore(&self, tracks: Vec<Track>, index: usize, position: f64) {
        if tracks.is_empty() {
            return;
        }
        let mut st = self.0.state.lock().unwrap();
        st.index = index.min(tracks.len() - 1);
        st.queue = tracks;
        st.unshuffled = None;
        st.position = position;
        st.resume_at = Some(position);
        st.radio_seed = None;
        st.next_loaded = None;
    }

    /// Play a restored track from where it was left. True when there was one to resume.
    fn resume(&self) -> bool {
        let (index, at) = {
            let st = self.0.state.lock().unwrap();
            let Some(at) = st.resume_at else { return false };
            (st.index, at)
        };
        let e = self.clone();
        self.0.rt.spawn(async move {
            let _ = e.play_from(index, Some(at)).await;
        });
        true
    }

    async fn play_from(&self, index: usize, start: Option<f64>) -> Result<()> {
        let requested = Instant::now();
        self.finish_play();
        let track = {
            let mut st = self.0.state.lock().unwrap();
            let Some(track) = st.queue.get(index).cloned() else {
                return Ok(());
            };
            st.index = index;
            st.next_loaded = None;
            st.position = start.unwrap_or(0.0);
            st.resume_at = None;
            track
        };
        self.emit(Update::NowPlaying { track: track.clone(), index });
        let resolving = Instant::now();
        let (mut stream, cached) = match self.stream(&track.id).await {
            Ok(s) => s,
            Err(e) => {
                self.emit(Update::Error {
                    message: explain(&track.title, &e),
                    details: Some(self.report(&track.id, &track.title, &format!("{e:#}"))),
                });
                return Err(e);
            }
        };
        stream.meta = Some((&track).into());
        log::info!("stream for {} in {} ms (url cached: {cached})", track.id, resolving.elapsed().as_millis());
        self.0.state.lock().unwrap().requested_at = Some((track.id.clone(), requested, cached));
        // A resume a few seconds in starts there; otherwise from the top.
        match start.filter(|s| *s > 2.0) {
            Some(at) => self.0.player.send(Command::Reload(stream, at)),
            None => self.0.player.send(Command::Play(stream)),
        }
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

    /// Keep the current song playing and replace what follows it with its radio.
    pub async fn radio_from_current(&self) {
        let (tracks, index) = {
            let mut st = self.0.state.lock().unwrap();
            let keep = (st.index + 1).min(st.queue.len());
            st.queue.truncate(keep);
            st.radio_seed = None;
            st.unshuffled = None;
            st.repeat_queued = false;
            if st.next_loaded.take().is_some() {
                self.0.player.send(Command::ClearNext);
            }
            (st.queue.clone(), st.index)
        };
        self.emit(Update::Queue { tracks, index });
        self.fill_radio(true).await;
        self.prefetch_next().await;
    }

    async fn top_up_radio(&self) {
        self.fill_radio(false).await
    }

    /// Add the current song's radio after the queue: when it's running low under autoplay,
    /// or now (`force`).
    async fn fill_radio(&self, force: bool) {
        let seed = {
            let st = self.0.state.lock().unwrap();
            let remaining = st.queue.len().saturating_sub(st.index + 1);
            let current = st.queue.get(st.index).map(|t| t.id.clone());
            if (!force && (!st.autoplay || remaining >= RADIO_LOOKAHEAD)) || current == st.radio_seed {
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
            st.upcoming().and_then(|i| st.queue.get(i).cloned())
        };
        let Some(next) = next else { return };
        match self.stream(&next.id).await {
            Ok((mut s, _)) => {
                s.meta = Some((&next).into());
                let mut st = self.0.state.lock().unwrap();
                // Still the right next track?
                if st.upcoming().and_then(|i| st.queue.get(i)).map(|t| &t.id) == Some(&next.id) {
                    st.next_loaded = Some(next.id.clone());
                    st.repeat_queued = st.upcoming() == Some(st.index);
                    self.0.player.send(Command::SetNext(s));
                }
            }
            Err(e) => log::warn!("prefetch {}: {e}", next.id),
        }
    }

    /// Drop the upcoming tracks `keep` rejects (a blocked artist's); what has played and what is
    /// playing stay. Refills radio and the gapless next track if they changed.
    pub fn retain_upcoming(&self, keep: impl Fn(&Track) -> bool) {
        let (tracks, index) = {
            let mut st = self.0.state.lock().unwrap();
            let i = st.index;
            let queue = std::mem::take(&mut st.queue);
            st.queue = queue.into_iter().enumerate().filter(|(j, t)| *j <= i || keep(t)).map(|(_, t)| t).collect();
            let next = st.upcoming().and_then(|j| st.queue.get(j)).map(|t| t.id.clone());
            if st.next_loaded.is_some() && st.next_loaded != next {
                st.next_loaded = None;
                st.repeat_queued = false;
                self.0.player.send(Command::ClearNext);
            }
            (st.queue.clone(), st.index)
        };
        self.emit(Update::Queue { tracks, index });
        let e = self.clone();
        self.0.rt.spawn(async move {
            e.top_up_radio().await;
            e.prefetch_next().await;
        });
    }

    pub async fn next(&self) -> Result<()> {
        let idx = {
            let st = self.0.state.lock().unwrap();
            match st.after_current() {
                Some(i) => i,
                None => return Ok(()),
            }
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
        if !paused && self.resume() {
            return;
        }
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
        if self.resume() {
            return;
        }
        self.0.player.send(Command::TogglePause);
    }

    pub fn seek(&self, secs: f64) {
        {
            let mut st = self.0.state.lock().unwrap();
            if st.resume_at.is_some() {
                st.resume_at = Some(secs);
                st.position = secs;
                drop(st);
                self.emit(Update::Position { secs });
                return;
            }
        }
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
                PlayerEvent::Recorded { path, expected } => {
                    if let Some(c) = self.song_cache() {
                        self.0.rt.spawn(async move {
                            // mpv writes the file's index as it closes it; give it a moment.
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            match tokio::task::spawn_blocking(move || c.keep(&path, expected)).await {
                                Ok(Ok(true)) => log::info!("song cache: kept a song"),
                                Ok(Ok(false)) => {}
                                Ok(Err(e)) => log::warn!("song cache: {e:#}"),
                                Err(e) => log::warn!("song cache: {e}"),
                            }
                        });
                    }
                }
                PlayerEvent::Idle => {
                    self.finish_play();
                    // Reached the end with nothing prefetched: try the next item.
                    let upcoming = {
                        let st = self.0.state.lock().unwrap();
                        st.upcoming().filter(|_| st.requested_at.is_none())
                    };
                    if let Some(i) = upcoming {
                        let _ = self.play_index(i).await;
                    }
                }
                PlayerEvent::Network { metered } => {
                    let was = self.0.metered.swap(metered, Ordering::Relaxed);
                    if was != metered {
                        log::info!("network: {}", if metered { "mobile data" } else { "Wi-Fi" });
                        self.sync_downloads();
                    }
                }
                PlayerEvent::Error { video_id, message } => {
                    log::warn!("playback error {video_id:?}: {message}");
                    self.on_error(video_id, message).await;
                }
            }
        }
    }

    fn on_started(&self, video_id: String) {
        // mpv reached the copy of this song queued for repeat one: a new play of it.
        let repeated = {
            let mut st = self.0.state.lock().unwrap();
            let r = st.repeat_queued
                && st.next_loaded.as_deref() == Some(video_id.as_str())
                && st.queue.get(st.index).is_some_and(|t| t.id == video_id)
                && st.requested_at.is_none()
                // It ended rather than reloaded mid-song (a quality switch also restarts it).
                && st.playing.as_ref().is_some_and(|p| p.duration > 0.0 && p.position >= p.duration - 3.0);
            if r {
                st.repeat_queued = false;
                st.next_loaded = None;
            }
            r
        };
        let same = !repeated
            && self
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
        } else if repeated {
            // Queue it again for the next time round.
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

    /// What to paste into a bug report about a song that wouldn't play.
    fn report(&self, video_id: &str, title: &str, error: &str) -> String {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        format!(
            "hum {} on {}\nunix time: {secs}\nsong: {title} (https://music.youtube.com/watch?v={video_id})\nquality: {:?}{}\nerror: {error}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            self.quality(),
            if self.auto_low() { " (stepped down)" } else { "" },
        )
    }

    /// A stream failed: drop its cached URL and retry once, then skip.
    async fn on_error(&self, video_id: Option<String>, message: String) {
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
            let title = self.current().map(|t| t.title).unwrap_or_default();
            self.emit(Update::Error {
                message: "That song stopped playing twice, so hum skipped it.".into(),
                details: Some(self.report(&id, &title, &format!("playback: {message}"))),
            });
            let _ = self.next().await;
        }
    }
}

/// What broke, in words a listener can act on. The error text comes from ytmusic-rs and
/// reqwest; anything unrecognised falls back to a plain "couldn't play".
fn explain(title: &str, e: &anyhow::Error) -> String {
    let offline = e.chain().any(|c| {
        c.downcast_ref::<reqwest::Error>().is_some_and(|r| r.is_connect() || r.is_timeout())
    });
    let text = format!("{e:#}");
    let has = |s: &str| text.contains(s);
    if offline || has("dns error") || has("tcp connect error") || has("timed out") {
        "Can't reach YouTube. Check your internet connection.".into()
    } else if has("not a bot") {
        "YouTube is asking this network to prove it isn't a bot. Signing in usually helps; otherwise try again later.".into()
    } else if has("confirm your age") {
        format!("\u{201c}{title}\u{201d} is age-restricted. Sign in to play it.")
    } else if has("is not playable") {
        let reason = text
            .split("is not playable: ")
            .nth(1)
            .and_then(|r| r.split_once('(')?.1.split_once(')'))
            .map(|(r, _)| r.trim().trim_end_matches('.'))
            .filter(|r| !r.is_empty() && *r != "no reason");
        match reason {
            Some(r) => format!("\u{201c}{title}\u{201d} isn't available. YouTube says: {r}."),
            None => format!("\u{201c}{title}\u{201d} isn't available on YouTube Music."),
        }
    } else if has("refused") || has("is not being served") {
        format!("YouTube refused every stream for \u{201c}{title}\u{201d}. Copy the details if it keeps happening.")
    } else {
        format!("Couldn't play \u{201c}{title}\u{201d}.")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn plays_downloads_before_the_song_cache() {
        use crate::cache::SongCache;
        use crate::downloads::Downloads;
        let dir = std::env::temp_dir().join(format!("hum-engine-local-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let songs = SongCache::new(dir.join("songs"), 1 << 20);
        let downloads = Downloads::new(dir.join("downloads"));
        std::fs::write(dir.join("songs/a.140.m4a"), b"cached").unwrap();
        std::fs::write(dir.join("songs/b.140.m4a"), b"cached").unwrap();
        std::fs::write(dir.join("downloads/a.140.m4a"), b"downloaded").unwrap();
        let q = super::Quality::High;
        let a = super::local_copy(Some(&downloads), Some(&songs), "a", q).unwrap();
        assert!(a.local && a.url.contains("downloads"), "a download wins over the cache");
        let b = super::local_copy(Some(&downloads), Some(&songs), "b", q).unwrap();
        assert!(b.url.contains("songs"), "the cache covers what is not downloaded");
        assert!(super::local_copy(Some(&downloads), Some(&songs), "c", q).is_none());
        assert!(super::local_copy(None, None, "a", q).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unshuffle_puts_the_order_back() {
        let t = |id: &str| super::Track {
            id: id.into(),
            title: id.into(),
            artists: vec![],
            artist_id: None,
            artist_ids: vec![],
            album: None,
            album_id: None,
            duration: None,
            thumb: None,
            cover: None,
        };
        let ids = |q: &[super::Track]| q.iter().map(|t| t.id.clone()).collect::<Vec<_>>().join("");
        let mut st = super::State {
            queue: "abcdefgh".chars().map(|c| t(&c.to_string())).collect(),
            index: 1,
            autoplay: false,
            repeat: super::Repeat::Off,
            shuffle: true,
            unshuffled: None,
            repeat_queued: false,
            requested_at: None,
            next_loaded: None,
            radio_seed: None,
            position: 0.0,
            retried: None,
            resume_at: None,
            playing: None,
        };
        st.shuffle_upcoming();
        assert_eq!(&ids(&st.queue)[..2], "ab");
        let mut rest: Vec<char> = ids(&st.queue)[2..].chars().collect();
        rest.sort();
        assert_eq!(rest.into_iter().collect::<String>(), "cdefgh");
        // Play on to a later song, radio adds one, a blocked artist's song is dropped.
        st.index = 4;
        let playing = st.queue[4].id.clone();
        st.queue.push(t("z"));
        // Drop the song right after the current one (upcoming songs are what retain_upcoming drops).
        let dropped = st.queue.remove(5).id;
        st.unshuffle();
        let expected: String = "abcdefghz".chars().filter(|c| c.to_string() != dropped).collect();
        assert_eq!(ids(&st.queue), expected);
        assert_eq!(st.queue[st.index].id, playing);
    }

    #[test]
    fn repeat_picks_what_plays_next() {
        let t = |id: &str| super::Track {
            id: id.into(),
            title: id.into(),
            artists: vec![],
            artist_id: None,
            artist_ids: vec![],
            album: None,
            album_id: None,
            duration: None,
            thumb: None,
            cover: None,
        };
        let mut st = super::State {
            queue: vec![t("a"), t("b")],
            index: 1,
            autoplay: false,
            repeat: super::Repeat::Off,
            shuffle: false,
            unshuffled: None,
            repeat_queued: false,
            requested_at: None,
            next_loaded: None,
            radio_seed: None,
            position: 0.0,
            retried: None,
            resume_at: None,
            playing: None,
        };
        assert_eq!((st.upcoming(), st.after_current()), (None, None));
        st.repeat = super::Repeat::All;
        assert_eq!((st.upcoming(), st.after_current()), (Some(0), Some(0)));
        st.repeat = super::Repeat::One;
        assert_eq!((st.upcoming(), st.after_current()), (Some(1), Some(0)));
        st.index = 0;
        assert_eq!((st.upcoming(), st.after_current()), (Some(0), Some(1)));
    }

    #[test]
    fn errors_say_what_broke() {
        let e = |m: &str| anyhow::anyhow!(m.to_string());
        let unavailable = e("the tokenless streams failed too (x is not playable: ERROR (This video is unavailable), x is not playable: ERROR (Video unavailable))");
        assert_eq!(explain("Song", &unavailable), "\u{201c}Song\u{201d} isn't available. YouTube says: This video is unavailable.");
        let offline = e("the tokenless streams failed too (cannot reach player: error sending request for url (https://www.youtube.com/youtubei/v1/player): client error (Connect): tcp connect error: Connection refused (os error 111))");
        assert!(explain("Song", &offline).starts_with("Can't reach YouTube"));
        let bot = e("x is not playable: LOGIN_REQUIRED (Sign in to confirm you\u{2019}re not a bot)");
        assert!(explain("Song", &bot).contains("isn't a bot"));
        let age = e("x is not playable: LOGIN_REQUIRED (Sign in to confirm your age)");
        assert!(explain("Song", &age).contains("age-restricted"));
        let refused = e("stream host refused the download with status 403 Forbidden");
        assert!(explain("Song", &refused).starts_with("YouTube refused"));
        assert_eq!(explain("Song", &e("something new")), "Couldn't play \u{201c}Song\u{201d}.");
    }

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
