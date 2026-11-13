//! What the engine tells a player and hears back, whichever player it is: libmpv on the
//! desktop ([`crate::player`]), the app's Media3 player on Android.

use crate::source::Stream;

#[derive(Debug)]
pub enum Command {
    /// Replace everything and play this stream now.
    Play(Stream),
    /// Replace the current track's stream with another one from `secs` (a quality change).
    Reload(Stream, f64),
    /// Queue this stream to play after the current one (gapless).
    SetNext(Stream),
    /// Forget the stream queued after the current one.
    ClearNext,
    SetPaused(bool),
    TogglePause,
    /// Seconds from the start.
    Seek(f64),
    /// 0-100.
    Volume(f64),
    Stop,
    /// Report the network (`PlayerEvent::Network`); only Android's player knows it.
    QueryNetwork,
    /// A song's big cover landed in the cache after it started: show it on the lock screen.
    Artwork { video_id: String, file: std::path::PathBuf },
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
    /// A recording for the song cache reached the end with no seek; `expected` is the
    /// stream's size.
    Recorded { path: std::path::PathBuf, expected: Option<u64> },
    Error { video_id: Option<String>, message: String },
    /// The connection is metered (mobile data) or not (Wi-Fi), on Android.
    Network { metered: bool },
}
