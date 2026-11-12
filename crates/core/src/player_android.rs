//! Playback on Android: there is no mpv, so the player is the app's own (Media3, in a foreground
//! service). Commands go out as JSON to the backend the app registers with [`set_backend`];
//! the app reports back with [`event`]. Queue, radio, prefetch and Auto quality stay in the
//! engine, so this file is the whole platform difference on the Rust side.

use std::sync::{Mutex, OnceLock};

use anyhow::Result;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

pub use crate::playback::{Command, PlayerEvent};
use crate::source::Stream;

/// Receives each command as a JSON object, e.g. `{"cmd":"play","stream":{...}}`.
pub type Backend = Box<dyn Fn(String) + Send + Sync>;

static BACKEND: OnceLock<Backend> = OnceLock::new();
static EVENTS: Mutex<Option<UnboundedSender<PlayerEvent>>> = Mutex::new(None);

/// Called once by the app, when its native player is reachable.
pub fn set_backend(backend: Backend) {
    if BACKEND.set(backend).is_err() {
        log::warn!("player backend set twice; keeping the first");
    }
}

/// An event from the app's player, as JSON: `{"ev":"started","id":"..."}`, `{"ev":"position",
/// "secs":1.5}`, `{"ev":"duration","secs":200}`, `{"ev":"paused","paused":true}`,
/// `{"ev":"stalled"}`, `{"ev":"idle"}`, `{"ev":"error","id":"...","message":"..."}`.
pub fn event(json: &str) {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        log::warn!("player event is not json: {json}");
        return;
    };
    let id = || v["id"].as_str().map(str::to_string);
    let secs = || v["secs"].as_f64().unwrap_or(0.0);
    let e = match v["ev"].as_str().unwrap_or_default() {
        "started" => PlayerEvent::Started { video_id: id().unwrap_or_default() },
        "position" => PlayerEvent::Position(secs()),
        "duration" => PlayerEvent::Duration(secs()),
        "paused" => PlayerEvent::Paused(v["paused"].as_bool().unwrap_or(false)),
        "stalled" => PlayerEvent::Stalled,
        "idle" => PlayerEvent::Idle,
        "error" => PlayerEvent::Error {
            video_id: id(),
            message: v["message"].as_str().unwrap_or("playback failed").to_string(),
        },
        other => {
            log::warn!("unknown player event {other:?}");
            return;
        }
    };
    if let Some(tx) = EVENTS.lock().unwrap().as_ref() {
        let _ = tx.send(e);
    }
}

fn stream_json(s: &Stream) -> Value {
    json!({
        "id": s.video_id,
        "url": s.url,
        "userAgent": s.user_agent,
        "local": s.local,
        // Media3 takes a linear volume for the item; the engine decides it in dB.
        "gain": s.gain_db.map_or(1.0, |db| 10f64.powf(db / 20.0)),
    })
}

fn to_json(cmd: &Command) -> Value {
    match cmd {
        Command::Play(s) => json!({ "cmd": "play", "stream": stream_json(s) }),
        Command::Reload(s, at) => json!({ "cmd": "play", "stream": stream_json(s), "start": at }),
        Command::SetNext(s) => json!({ "cmd": "setNext", "stream": stream_json(s) }),
        Command::ClearNext => json!({ "cmd": "clearNext" }),
        Command::SetPaused(p) => json!({ "cmd": "setPaused", "paused": p }),
        Command::TogglePause => json!({ "cmd": "togglePause" }),
        Command::Seek(secs) => json!({ "cmd": "seek", "secs": secs }),
        Command::Volume(v) => json!({ "cmd": "volume", "volume": v / 100.0 }),
        Command::Stop => json!({ "cmd": "stop" }),
    }
}

pub struct Player;

impl Player {
    pub fn spawn(events: UnboundedSender<PlayerEvent>) -> Result<Player> {
        *EVENTS.lock().unwrap() = Some(events);
        Ok(Player)
    }

    pub fn send(&self, cmd: Command) {
        let json = to_json(&cmd).to_string();
        match BACKEND.get() {
            Some(backend) => backend(json),
            None => log::warn!("no player backend yet; dropped {json}"),
        }
    }
}
