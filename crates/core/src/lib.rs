//! hum core: YouTube Music source, mpv playback, queue and radio. No UI dependencies, so the
//! same crate can back the desktop app and, later, the Android port.

pub mod account;
pub mod color;
pub mod engine;
pub mod library;
pub mod lyrics;
pub mod player;
pub mod source;

pub use account::Account;
pub use engine::{Engine, Update};
pub use library::Library;
pub use source::{Quality, Source, Stream, Track};
