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

#[derive(Clone, Debug, Serialize)]
pub struct PlaylistSummary {
    pub id: i64,
    pub name: String,
    pub count: u32,
    /// Up to four distinct covers, for the mosaic.
    pub covers: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct DataUsed {
    pub month: u64,
    pub total: u64,
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
             CREATE TABLE IF NOT EXISTS playlists (
                 id INTEGER PRIMARY KEY,
                 name TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS playlist_tracks (
                 playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                 pos INTEGER NOT NULL,
                 track_id TEXT NOT NULL REFERENCES tracks(id),
                 added_at INTEGER NOT NULL,
                 PRIMARY KEY (playlist_id, pos)
             );
             CREATE TABLE IF NOT EXISTS likes (
                 track_id TEXT PRIMARY KEY REFERENCES tracks(id),
                 liked_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS data_used (
                 month TEXT PRIMARY KEY,
                 bytes INTEGER NOT NULL
             );",
        )?;
        Ok(Library {
            db: Arc::new(Mutex::new(db)),
        })
    }

    /// Add downloaded audio bytes to this month's total (local time).
    pub fn add_data_used(&self, bytes: u64) -> Result<()> {
        self.db.lock().unwrap().execute(
            "INSERT INTO data_used (month, bytes) VALUES (strftime('%Y-%m', 'now', 'localtime'), ?1)
             ON CONFLICT(month) DO UPDATE SET bytes = bytes + ?1",
            params![bytes as i64],
        )?;
        Ok(())
    }

    /// Audio bytes downloaded this month and in all.
    pub fn data_used(&self) -> Result<DataUsed> {
        let db = self.db.lock().unwrap();
        let (month, total) = db.query_row(
            "SELECT COALESCE(SUM(CASE WHEN month = strftime('%Y-%m', 'now', 'localtime') THEN bytes END), 0),
                    COALESCE(SUM(bytes), 0)
             FROM data_used",
            [],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )?;
        Ok(DataUsed { month: month as u64, total: total as u64 })
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

    // ---- playlists -----------------------------------------------------------------------

    pub fn create_playlist(&self, name: &str) -> Result<i64> {
        let db = self.db.lock().unwrap();
        let t = now();
        db.execute(
            "INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
            params![clean_name(name), t],
        )?;
        Ok(db.last_insert_rowid())
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<()> {
        self.db.lock().unwrap().execute(
            "UPDATE playlists SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, clean_name(name), now()],
        )?;
        Ok(())
    }

    pub fn delete_playlist(&self, id: i64) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", params![id])?;
        db.execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Playlists, most recently changed first.
    pub fn playlists(&self) -> Result<Vec<PlaylistSummary>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare(
            "SELECT p.id, p.name, (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)
             FROM playlists p ORDER BY p.updated_at DESC",
        )?;
        let rows: Vec<(i64, String, u32)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .flatten()
            .collect();
        let mut covers = db.prepare(
            "SELECT t.json FROM playlist_tracks pt JOIN tracks t ON t.id = pt.track_id
             WHERE pt.playlist_id = ?1 ORDER BY pt.pos LIMIT 24",
        )?;
        let mut out = Vec::new();
        for (id, name, count) in rows {
            let mut seen = Vec::<String>::new();
            for j in covers.query_map(params![id], |r| r.get::<_, String>(0))?.flatten() {
                if let Ok(t) = serde_json::from_str::<Track>(&j) {
                    if let Some(c) = t.thumb.or(t.cover) {
                        if !seen.contains(&c) {
                            seen.push(c);
                        }
                    }
                }
                if seen.len() == 4 {
                    break;
                }
            }
            out.push(PlaylistSummary { id, name, count, covers: seen });
        }
        Ok(out)
    }

    pub fn playlist(&self, id: i64) -> Result<Option<Playlist>> {
        let db = self.db.lock().unwrap();
        let Some(name) = db
            .query_row("SELECT name FROM playlists WHERE id = ?1", params![id], |r| r.get::<_, String>(0))
            .optional()?
        else {
            return Ok(None);
        };
        let mut st = db.prepare(
            "SELECT t.json FROM playlist_tracks pt JOIN tracks t ON t.id = pt.track_id
             WHERE pt.playlist_id = ?1 ORDER BY pt.pos",
        )?;
        let tracks = st
            .query_map(params![id], |r| r.get::<_, String>(0))?
            .filter_map(|j| j.ok().and_then(|j| serde_json::from_str(&j).ok()))
            .collect();
        Ok(Some(Playlist { id, name, tracks }))
    }

    /// Append tracks, skipping ones already in the playlist. Returns how many were added.
    pub fn add_to_playlist(&self, id: i64, tracks: &[Track]) -> Result<usize> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let mut next: i64 = tx.query_row(
            "SELECT COALESCE(MAX(pos) + 1, 0) FROM playlist_tracks WHERE playlist_id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        let mut added = 0;
        for t in tracks {
            let exists: bool = tx
                .prepare_cached("SELECT 1 FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2")?
                .exists(params![id, t.id])?;
            if exists {
                continue;
            }
            Self::upsert_track(&tx, t)?;
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, pos, track_id, added_at) VALUES (?1, ?2, ?3, ?4)",
                params![id, next, t.id, now()],
            )?;
            next += 1;
            added += 1;
        }
        tx.execute("UPDATE playlists SET updated_at = ?2 WHERE id = ?1", params![id, now()])?;
        tx.commit()?;
        Ok(added)
    }

    /// Remove the song at `index` (0-based, in playlist order).
    pub fn remove_from_playlist(&self, id: i64, index: usize) -> Result<()> {
        let mut ids = self.track_ids(id)?;
        if index < ids.len() {
            ids.remove(index);
            self.rewrite_order(id, &ids)?;
        }
        Ok(())
    }

    /// Move the song at `from` to `to` (0-based, in playlist order).
    pub fn move_in_playlist(&self, id: i64, from: usize, to: usize) -> Result<()> {
        let mut ids = self.track_ids(id)?;
        if from < ids.len() && to < ids.len() && from != to {
            let t = ids.remove(from);
            ids.insert(to, t);
            self.rewrite_order(id, &ids)?;
        }
        Ok(())
    }

    fn track_ids(&self, id: i64) -> Result<Vec<(String, i64)>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare(
            "SELECT track_id, added_at FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY pos",
        )?;
        let v = st
            .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .flatten()
            .collect();
        Ok(v)
    }

    fn rewrite_order(&self, id: i64, ids: &[(String, i64)]) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        tx.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", params![id])?;
        for (pos, (track, added)) in ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, pos, track_id, added_at) VALUES (?1, ?2, ?3, ?4)",
                params![id, pos as i64, track, added],
            )?;
        }
        tx.execute("UPDATE playlists SET updated_at = ?2 WHERE id = ?1", params![id, now()])?;
        tx.commit()?;
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

fn clean_name(name: &str) -> String {
    let n = name.trim();
    if n.is_empty() { "New playlist".to_string() } else { n.chars().take(100).collect() }
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
    fn playlists_add_dedupe_move_remove() {
        let lib = Library::memory().unwrap();
        let id = lib.create_playlist("  Night drive ").unwrap();
        let (a, b, c) = (track("a", "x"), track("b", "y"), track("c", "z"));
        assert_eq!(lib.add_to_playlist(id, &[a.clone(), b.clone()]).unwrap(), 2);
        assert_eq!(lib.add_to_playlist(id, &[b.clone(), c.clone()]).unwrap(), 1);
        let ids = |l: &Library| -> Vec<String> {
            l.playlist(id).unwrap().unwrap().tracks.into_iter().map(|t| t.id).collect()
        };
        assert_eq!(ids(&lib), ["a", "b", "c"]);
        lib.move_in_playlist(id, 2, 0).unwrap();
        assert_eq!(ids(&lib), ["c", "a", "b"]);
        lib.remove_from_playlist(id, 1).unwrap();
        assert_eq!(ids(&lib), ["c", "b"]);
        let list = lib.playlists().unwrap();
        assert_eq!((list[0].name.as_str(), list[0].count), ("Night drive", 2));
        lib.rename_playlist(id, "").unwrap();
        assert_eq!(lib.playlist(id).unwrap().unwrap().name, "New playlist");
        lib.delete_playlist(id).unwrap();
        assert!(lib.playlist(id).unwrap().is_none());
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

    #[test]
    fn data_used_adds_up_per_month() {
        let l = Library::memory().unwrap();
        assert_eq!(l.data_used().unwrap(), DataUsed { month: 0, total: 0 });
        l.add_data_used(1_000).unwrap();
        l.add_data_used(500).unwrap();
        l.db.lock().unwrap().execute("INSERT INTO data_used VALUES ('2001-01', 7)", []).unwrap();
        assert_eq!(l.data_used().unwrap(), DataUsed { month: 1_500, total: 1_507 });
    }
}
