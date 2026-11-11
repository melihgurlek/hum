//! The local library: every play with how it ended, likes, and the taste scores built from them.
//!
//! One SQLite file. Access goes through a `Mutex<Connection>` and every call is short, so the
//! async callers use `spawn_blocking` (see `Library::run`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::source::Track;

/// How a play ended. Drives the taste score.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Listened to at least 80%, or to the end.
    Completed,
    /// Skipped within the first 30 seconds.
    SkippedEarly,
    /// Skipped later.
    SkippedLate,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Completed => "completed",
            Outcome::SkippedEarly => "skipped_early",
            Outcome::SkippedLate => "skipped_late",
        }
    }

    /// Classify a play from how long it lasted.
    pub fn classify(listened_secs: f64, duration_secs: f64) -> Outcome {
        if duration_secs > 0.0 && listened_secs >= duration_secs * 0.8 {
            Outcome::Completed
        } else if listened_secs < 30.0 {
            Outcome::SkippedEarly
        } else {
            Outcome::SkippedLate
        }
    }

    fn weight(s: &str) -> f64 {
        match s {
            "completed" => 1.0,
            "skipped_early" => -1.0,
            "skipped_late" => -0.3,
            _ => 0.0,
        }
    }
}

/// Plays fade with a 30-day half-life.
const HALF_LIFE_DAYS: f64 = 30.0;
/// A like counts like two completed plays that never fade.
const LIKE_WEIGHT: f64 = 2.0;
const DAY: i64 = 86_400;

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// What the library knows about one track, for "why is this here" labels.
#[derive(Clone, Debug, Serialize, Default)]
pub struct TrackNote {
    pub liked: bool,
    pub plays: u32,
    pub last_played: Option<i64>,
    pub skipped_recently: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct HistoryEntry {
    pub track: Track,
    pub played_at: i64,
    pub outcome: Outcome,
}

#[derive(Clone)]
pub struct Library {
    db: Arc<Mutex<Connection>>,
}

impl Library {
    pub fn open(path: &Path) -> Result<Library> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        let db = Connection::open(path)?;
        db.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS tracks (
                 id TEXT PRIMARY KEY,
                 json TEXT NOT NULL,
                 artist TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS plays (
                 id INTEGER PRIMARY KEY,
                 track_id TEXT NOT NULL REFERENCES tracks(id),
                 artist TEXT NOT NULL,
                 played_at INTEGER NOT NULL,
                 listened_secs REAL NOT NULL,
                 duration_secs REAL NOT NULL,
                 outcome TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS plays_track ON plays(track_id, played_at);
             CREATE INDEX IF NOT EXISTS plays_time ON plays(played_at);
             CREATE TABLE IF NOT EXISTS lyrics (
                 track_id TEXT PRIMARY KEY,
                 json TEXT,
                 fetched_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS likes (
                 track_id TEXT PRIMARY KEY REFERENCES tracks(id),
                 liked_at INTEGER NOT NULL
             );",
        )?;
        Ok(Library {
            db: Arc::new(Mutex::new(db)),
        })
    }

    /// In-memory, for tests.
    pub fn memory() -> Result<Library> {
        Library::open(Path::new(":memory:"))
    }

    /// Run a short DB job off the async runtime.
    pub async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&Library) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let lib = self.clone();
        tokio::task::spawn_blocking(move || f(&lib)).await?
    }

    fn upsert_track(db: &Connection, t: &Track) -> Result<()> {
        db.execute(
            "INSERT INTO tracks (id, json, artist, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET json = ?2, artist = ?3, updated_at = ?4",
            params![t.id, serde_json::to_string(t)?, artist_key(t), now()],
        )?;
        Ok(())
    }

    pub fn record_play(
        &self,
        track: &Track,
        played_at: i64,
        listened_secs: f64,
        duration_secs: f64,
    ) -> Result<Outcome> {
        let outcome = Outcome::classify(listened_secs, duration_secs);
        let db = self.db.lock().unwrap();
        Self::upsert_track(&db, track)?;
        db.execute(
            "INSERT INTO plays (track_id, artist, played_at, listened_secs, duration_secs, outcome)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                track.id,
                artist_key(track),
                played_at,
                listened_secs,
                duration_secs,
                outcome.as_str()
            ],
        )?;
        Ok(outcome)
    }

    /// Cached lyrics: `Some(None)` means "looked recently, there are none".
    pub fn cached_lyrics(&self, id: &str) -> Result<Option<Option<crate::lyrics::Lyrics>>> {
        let db = self.db.lock().unwrap();
        let row: Option<(Option<String>, i64)> = db
            .query_row(
                "SELECT json, fetched_at FROM lyrics WHERE track_id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((Some(j), _)) => Some(serde_json::from_str(&j).ok()),
            // Lyrics get added to LRCLIB over time: look again after a week.
            Some((None, at)) if now() - at < 7 * DAY => Some(None),
            _ => None,
        })
    }

    pub fn store_lyrics(&self, id: &str, lyrics: Option<&crate::lyrics::Lyrics>) -> Result<()> {
        let json = lyrics.map(serde_json::to_string).transpose()?;
        self.db.lock().unwrap().execute(
            "INSERT OR REPLACE INTO lyrics (track_id, json, fetched_at) VALUES (?1, ?2, ?3)",
            params![id, json, now()],
        )?;
        Ok(())
    }

    pub fn set_liked(&self, track: &Track, liked: bool) -> Result<()> {
        let db = self.db.lock().unwrap();
        if liked {
            Self::upsert_track(&db, track)?;
            db.execute(
                "INSERT OR IGNORE INTO likes (track_id, liked_at) VALUES (?1, ?2)",
                params![track.id, now()],
            )?;
        } else {
            db.execute("DELETE FROM likes WHERE track_id = ?1", params![track.id])?;
        }
        Ok(())
    }

    pub fn is_liked(&self, id: &str) -> Result<bool> {
        let db = self.db.lock().unwrap();
        Ok(db
            .query_row("SELECT 1 FROM likes WHERE track_id = ?1", params![id], |_| Ok(()))
            .optional()?
            .is_some())
    }

    pub fn liked(&self) -> Result<Vec<Track>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare(
            "SELECT t.json FROM likes l JOIN tracks t ON t.id = l.track_id ORDER BY l.liked_at DESC",
        )?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows
            .filter_map(|j| j.ok().and_then(|j| serde_json::from_str(&j).ok()))
            .collect())
    }

    /// Plays, newest first.
    pub fn history(&self, limit: usize) -> Result<Vec<HistoryEntry>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare(
            "SELECT t.json, p.played_at, p.outcome FROM plays p JOIN tracks t ON t.id = p.track_id
             ORDER BY p.played_at DESC LIMIT ?1",
        )?;
        let rows = st.query_map(params![limit as i64], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?))
        })?;
        Ok(rows
            .filter_map(|r| r.ok())
            .filter_map(|(j, at, o)| {
                Some(HistoryEntry {
                    track: serde_json::from_str(&j).ok()?,
                    played_at: at,
                    outcome: match o.as_str() {
                        "completed" => Outcome::Completed,
                        "skipped_early" => Outcome::SkippedEarly,
                        _ => Outcome::SkippedLate,
                    },
                })
            })
            .collect())
    }

    /// Distinct recently played tracks (completed or skipped late), newest first.
    pub fn recent_tracks(&self, limit: usize) -> Result<Vec<Track>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare(
            "SELECT t.json FROM plays p JOIN tracks t ON t.id = p.track_id
             WHERE p.outcome != 'skipped_early'
             GROUP BY p.track_id ORDER BY MAX(p.played_at) DESC LIMIT ?1",
        )?;
        let rows = st.query_map(params![limit as i64], |r| r.get::<_, String>(0))?;
        Ok(rows
            .filter_map(|j| j.ok().and_then(|j| serde_json::from_str(&j).ok()))
            .collect())
    }

    pub fn notes(&self, ids: &[String]) -> Result<HashMap<String, TrackNote>> {
        let db = self.db.lock().unwrap();
        let cutoff = now() - 14 * DAY;
        let mut out = HashMap::new();
        let mut plays = db.prepare(
            "SELECT COUNT(*), MAX(played_at),
                    SUM(CASE WHEN outcome = 'skipped_early' AND played_at > ?2 THEN 1 ELSE 0 END)
             FROM plays WHERE track_id = ?1",
        )?;
        let mut like = db.prepare("SELECT 1 FROM likes WHERE track_id = ?1")?;
        for id in ids {
            let (n, last, skips): (u32, Option<i64>, Option<u32>) =
                plays.query_row(params![id, cutoff], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            let liked = like.exists(params![id])?;
            out.insert(
                id.clone(),
                TrackNote {
                    liked,
                    plays: n,
                    last_played: last,
                    skipped_recently: skips.unwrap_or(0) > 0,
                },
            );
        }
        Ok(out)
    }

    /// Decayed taste scores per track and per artist.
    fn scores(db: &Connection) -> Result<(HashMap<String, f64>, HashMap<String, f64>)> {
        let t = now() as f64;
        let mut tracks: HashMap<String, f64> = HashMap::new();
        let mut artists: HashMap<String, f64> = HashMap::new();
        let mut st = db.prepare("SELECT track_id, artist, played_at, outcome FROM plays")?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        for (id, artist, at, outcome) in rows.flatten() {
            let age_days = ((t - at as f64) / DAY as f64).max(0.0);
            let w = Outcome::weight(&outcome) * 0.5f64.powf(age_days / HALF_LIFE_DAYS);
            *tracks.entry(id).or_default() += w;
            *artists.entry(artist).or_default() += w;
        }
        let mut st = db.prepare(
            "SELECT l.track_id, t.artist FROM likes l JOIN tracks t ON t.id = l.track_id",
        )?;
        for (id, artist) in st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .flatten()
        {
            *tracks.entry(id).or_default() += LIKE_WEIGHT;
            *artists.entry(artist).or_default() += LIKE_WEIGHT;
        }
        Ok((tracks, artists))
    }

    /// Your best-liked tracks right now: seeds for radio and Home.
    pub fn top_tracks(&self, limit: usize) -> Result<Vec<Track>> {
        let db = self.db.lock().unwrap();
        let (tracks, _) = Self::scores(&db)?;
        let mut ranked: Vec<(String, f64)> = tracks.into_iter().filter(|(_, s)| *s > 0.5).collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        Self::load_tracks(&db, ranked.into_iter().take(limit).map(|(id, _)| id))
    }

    /// Songs you used to play a lot but haven't heard in at least `quiet_days`.
    pub fn forgotten_favorites(&self, quiet_days: i64, limit: usize) -> Result<Vec<(Track, i64)>> {
        let db = self.db.lock().unwrap();
        let cutoff = now() - quiet_days * DAY;
        let mut st = db.prepare(
            "SELECT p.track_id, MAX(p.played_at) AS last,
                    SUM(CASE WHEN p.outcome = 'completed' THEN 1 ELSE 0 END) AS done,
                    EXISTS(SELECT 1 FROM likes l WHERE l.track_id = p.track_id) AS liked
             FROM plays p GROUP BY p.track_id
             HAVING last < ?1 AND (done >= 3 OR liked)
             ORDER BY done DESC LIMIT ?2",
        )?;
        let rows: Vec<(String, i64)> = st
            .query_map(params![cutoff, limit as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
            .flatten()
            .collect();
        let mut out = Vec::new();
        for (id, last) in rows {
            if let Some(t) = Self::load_tracks(&db, std::iter::once(id))?.pop() {
                out.push((t, last));
            }
        }
        Ok(out)
    }

    fn load_tracks(db: &Connection, ids: impl Iterator<Item = String>) -> Result<Vec<Track>> {
        let mut st = db.prepare("SELECT json FROM tracks WHERE id = ?1")?;
        let mut out = Vec::new();
        for id in ids {
            if let Some(j) = st
                .query_row(params![id], |r| r.get::<_, String>(0))
                .optional()?
            {
                if let Ok(t) = serde_json::from_str(&j) {
                    out.push(t);
                }
            }
        }
        Ok(out)
    }

    /// Reorder radio candidates by taste and drop what you clearly don't want right now.
    ///
    /// - Skipped within 30 s in the last 14 days: dropped.
    /// - Played in the last 6 hours: dropped (you just heard it).
    /// - Otherwise keep YouTube's order (it already encodes "sounds like this"), nudged by your
    ///   artist affinity, and never more than 2 songs by one artist in any run of 10.
    pub fn rank_radio(&self, candidates: Vec<Track>) -> Result<Vec<Track>> {
        let db = self.db.lock().unwrap();
        let (_, artists) = Self::scores(&db)?;
        let skip_cutoff = now() - 14 * DAY;
        let recent_cutoff = now() - 6 * 3600;
        let mut skipped = db.prepare(
            "SELECT 1 FROM plays WHERE track_id = ?1 AND outcome = 'skipped_early' AND played_at > ?2",
        )?;
        let mut recent = db.prepare("SELECT 1 FROM plays WHERE track_id = ?1 AND played_at > ?2")?;
        let total: f64 = artists.values().map(|v| v.abs()).sum::<f64>().max(1.0);

        let mut scored = Vec::new();
        for (rank, t) in candidates.into_iter().enumerate() {
            if skipped.exists(params![t.id, skip_cutoff])? || recent.exists(params![t.id, recent_cutoff])? {
                continue;
            }
            let rank_score = 1.0 / (1.0 + rank as f64 / 5.0);
            let affinity = artists.get(&artist_key(&t)).copied().unwrap_or(0.0) / total;
            scored.push((0.75 * rank_score + 0.25 * affinity.clamp(-1.0, 1.0), t));
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        Ok(spread_artists(scored.into_iter().map(|(_, t)| t).collect(), 2, 10))
    }
}

fn artist_key(t: &Track) -> String {
    t.artist_id
        .clone()
        .unwrap_or_else(|| t.artists.first().cloned().unwrap_or_default().to_lowercase())
}

/// Reorder so no artist appears more than `max` times in any window of `window` tracks. Tracks
/// that can't be placed yet wait and go in as soon as they fit; leftovers go at the end.
fn spread_artists(tracks: Vec<Track>, max: usize, window: usize) -> Vec<Track> {
    let mut out: Vec<Track> = Vec::with_capacity(tracks.len());
    let mut waiting: std::collections::VecDeque<Track> = tracks.into();
    let mut stalled = 0;
    while let Some(t) = waiting.pop_front() {
        let key = artist_key(&t);
        let start = out.len().saturating_sub(window - 1);
        let n = out[start..].iter().filter(|o| artist_key(o) == key).count();
        if n < max || stalled > waiting.len() {
            out.push(t);
            stalled = 0;
        } else {
            waiting.push_back(t);
            stalled += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, artist: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            artists: vec![artist.into()],
            artist_id: Some(artist.into()),
            album: None,
            album_id: None,
            duration: Some(200),
            thumb: None,
            cover: None,
        }
    }

    #[test]
    fn classifies_outcomes() {
        assert_eq!(Outcome::classify(190.0, 200.0), Outcome::Completed);
        assert_eq!(Outcome::classify(10.0, 200.0), Outcome::SkippedEarly);
        assert_eq!(Outcome::classify(90.0, 200.0), Outcome::SkippedLate);
    }

    #[test]
    fn radio_drops_recent_skips_and_spreads_artists() {
        let lib = Library::memory().unwrap();
        let skipped = track("s", "x");
        lib.record_play(&skipped, now() - DAY, 5.0, 200.0).unwrap();
        let mut c = vec![skipped.clone()];
        for i in 0..5 {
            c.push(track(&format!("a{i}"), "a"));
        }
        c.push(track("b0", "b"));
        let ranked = lib.rank_radio(c).unwrap();
        assert!(ranked.iter().all(|t| t.id != "s"));
        let first_three: Vec<_> = ranked.iter().take(3).map(|t| artist_key(t)).collect();
        assert!(first_three.contains(&"b".to_string()), "{first_three:?}");
    }

    #[test]
    fn forgotten_favorites_need_old_repeated_plays() {
        let lib = Library::memory().unwrap();
        let fav = track("f", "a");
        for d in 0..4 {
            lib.record_play(&fav, now() - (90 + d) * DAY, 200.0, 200.0).unwrap();
        }
        let fresh = track("n", "a");
        for _ in 0..4 {
            lib.record_play(&fresh, now() - DAY, 200.0, 200.0).unwrap();
        }
        let ff = lib.forgotten_favorites(30, 10).unwrap();
        assert_eq!(ff.len(), 1);
        assert_eq!(ff[0].0.id, "f");
    }

    #[test]
    fn likes_round_trip_and_score() {
        let lib = Library::memory().unwrap();
        let t = track("l", "a");
        lib.set_liked(&t, true).unwrap();
        assert!(lib.is_liked("l").unwrap());
        assert_eq!(lib.liked().unwrap().len(), 1);
        assert_eq!(lib.top_tracks(5).unwrap()[0].id, "l");
        lib.set_liked(&t, false).unwrap();
        assert!(!lib.is_liked("l").unwrap());
    }
}
