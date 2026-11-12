//! hum core: YouTube Music source, mpv playback, queue and radio. No UI dependencies, so the
//! same crate can back the desktop app and, later, the Android port.

pub mod account;
pub mod cache;
pub mod color;
pub mod downloads;
pub mod engine;
pub mod library;
pub mod lyrics;
pub mod playback;
#[cfg(not(target_os = "android"))]
pub mod player;
#[cfg(target_os = "android")]
#[path = "player_android.rs"]
pub mod player;
pub mod source;
pub mod sync;

pub use account::Account;
pub use engine::{Engine, Repeat, Update};
pub use library::Library;
pub use source::{Quality, Source, Stream, Track};
