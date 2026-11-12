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
/// A like counts like two completed plays, fading from the last time it was liked or played.
const LIKE_WEIGHT: f64 = 2.0;
const LIKE_HALF_LIFE_DAYS: f64 = 90.0;
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
    /// On the YouTube Music account too.
    pub synced: bool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct DataUsed {
    pub month: u64,
    pub total: u64,
}

/// A playlist row: track id, when it was added, its id on the account playlist.
type Entry = (String, i64, Option<String>);

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
        // v1: YouTube Music account sync. `remote`: the like is on the account; `unliked` and
        // `deleted_playlists` hold removals not yet sent.
        if db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? < 1 {
            db.execute_batch(
                "ALTER TABLE likes ADD COLUMN remote INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE playlists ADD COLUMN remote_id TEXT;
                 ALTER TABLE playlist_tracks ADD COLUMN set_video_id TEXT;
                 CREATE TABLE IF NOT EXISTS unliked (track_id TEXT PRIMARY KEY);
                 CREATE TABLE IF NOT EXISTS deleted_playlists (remote_id TEXT PRIMARY KEY);
                 PRAGMA user_version = 1;",
            )?;
        }
        // v2: account playlists kept on the account but not shown in hum.
        if db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? < 2 {
            db.execute_batch(
                "CREATE TABLE IF NOT EXISTS hidden_playlists (remote_id TEXT PRIMARY KEY, name TEXT NOT NULL);
                 PRAGMA user_version = 2;",
            )?;
        }
        // v3: likes that came from the account, whose like date is unknown. Retro: synced likes
        // never played here can only have come from the account.
        if db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? < 3 {
            db.execute_batch(
                "ALTER TABLE likes ADD COLUMN imported INTEGER NOT NULL DEFAULT 0;
                 UPDATE likes SET imported = 1 WHERE remote = 1 AND track_id NOT IN (SELECT track_id FROM plays);
                 PRAGMA user_version = 3;",
            )?;
        }
        // v4: artists kept out of radio and quick picks.
        if db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? < 4 {
            db.execute_batch(
                "CREATE TABLE IF NOT EXISTS blocked_artists (key TEXT PRIMARY KEY, name TEXT NOT NULL, blocked_at INTEGER NOT NULL);
                 PRAGMA user_version = 4;",
            )?;
        }
        // v5: songs' loudness from YouTube, so a copy from the song cache plays at the same level.
        if db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? < 5 {
            db.execute_batch(
                "CREATE TABLE IF NOT EXISTS loudness (track_id TEXT PRIMARY KEY, db REAL NOT NULL);
                 PRAGMA user_version = 5;",
            )?;
        }
        Ok(Library {
            db: Arc::new(Mutex::new(db)),
        })
    }

    pub fn set_loudness(&self, track_id: &str, db: f32) -> Result<()> {
        self.db.lock().unwrap().execute(
            "INSERT OR REPLACE INTO loudness (track_id, db) VALUES (?1, ?2)",
            params![track_id, db as f64],
        )?;
        Ok(())
    }

    pub fn loudness(&self, track_id: &str) -> Result<Option<f32>> {
        let db = self.db.lock().unwrap();
        let v: Option<f64> = db
            .query_row("SELECT db FROM loudness WHERE track_id = ?1", params![track_id], |r| r.get(0))
            .optional()?;
        Ok(v.map(|v| v as f32))
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
        db.execute(
            "INSERT OR IGNORE INTO deleted_playlists SELECT remote_id FROM playlists WHERE id = ?1 AND remote_id IS NOT NULL",
            params![id],
        )?;
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
        let Some((name, synced)) = db
            .query_row("SELECT name, remote_id IS NOT NULL FROM playlists WHERE id = ?1", params![id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?))
            })
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
        Ok(Some(Playlist { id, name, tracks, synced }))
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

    /// Each playlist that holds the song, with the song's place in it (0-based), so the add
    /// menu can show where it already is and take it out again.
    pub fn playlists_with(&self, track_id: &str) -> Result<Vec<(i64, usize)>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare(
            "SELECT pt.playlist_id,
                    (SELECT COUNT(*) FROM playlist_tracks o WHERE o.playlist_id = pt.playlist_id AND o.pos < pt.pos)
             FROM playlist_tracks pt WHERE pt.track_id = ?1",
        )?;
        let v = st
            .query_map(params![track_id], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as usize)))?
            .flatten()
            .collect();
        Ok(v)
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

    fn track_ids(&self, id: i64) -> Result<Vec<Entry>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare(
            "SELECT track_id, added_at, set_video_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY pos",
        )?;
        let v = st
            .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .flatten()
            .collect();
        Ok(v)
    }

    fn rewrite_order(&self, id: i64, ids: &[Entry]) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        tx.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", params![id])?;
        for (pos, (track, added, set_id)) in ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, pos, track_id, added_at, set_video_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, pos as i64, track, added, set_id],
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
            db.execute(
                "INSERT OR IGNORE INTO unliked SELECT track_id FROM likes WHERE track_id = ?1 AND remote = 1",
                params![track.id],
            )?;
            db.execute("DELETE FROM likes WHERE track_id = ?1", params![track.id])?;
        }
        Ok(())
    }

    // ---- account sync --------------------------------------------------------------------

    /// Every like, and whether it is on the account.
    pub fn like_states(&self) -> Result<Vec<(String, bool)>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare("SELECT track_id, remote FROM likes")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.flatten().collect();
        Ok(v)
    }

    pub fn mark_liked_remote(&self, id: &str) -> Result<()> {
        self.db.lock().unwrap().execute("UPDATE likes SET remote = 1 WHERE track_id = ?1", params![id])?;
        Ok(())
    }

    /// Likes from the account that hum doesn't have, newest first.
    pub fn add_remote_likes(&self, tracks: &[Track]) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let t = now();
        for (i, track) in tracks.iter().enumerate() {
            Self::upsert_track(&tx, track)?;
            tx.execute(
                "INSERT INTO likes (track_id, liked_at, remote, imported) VALUES (?1, ?2, 1, 1)
                 ON CONFLICT(track_id) DO UPDATE SET remote = 1",
                params![track.id, t - i as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Unliked on another device.
    pub fn drop_like(&self, id: &str) -> Result<()> {
        self.db.lock().unwrap().execute("DELETE FROM likes WHERE track_id = ?1", params![id])?;
        Ok(())
    }

    pub fn pending_unlikes(&self) -> Result<Vec<String>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare("SELECT track_id FROM unliked")?;
        let v = st.query_map([], |r| r.get(0))?.flatten().collect();
        Ok(v)
    }

    pub fn clear_unlike(&self, id: &str) -> Result<()> {
        self.db.lock().unwrap().execute("DELETE FROM unliked WHERE track_id = ?1", params![id])?;
        Ok(())
    }

    /// Every playlist with its account playlist id, if it has one.
    pub fn playlist_links(&self) -> Result<Vec<(i64, Option<String>)>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare("SELECT id, remote_id FROM playlists")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.flatten().collect();
        Ok(v)
    }

    pub fn playlist_remote(&self, id: i64) -> Result<Option<String>> {
        let db = self.db.lock().unwrap();
        Ok(db
            .query_row("SELECT remote_id FROM playlists WHERE id = ?1", params![id], |r| r.get(0))
            .optional()?
            .flatten())
    }

    pub fn link_playlist(&self, id: i64, remote_id: &str) -> Result<()> {
        self.db.lock().unwrap().execute(
            "UPDATE playlists SET remote_id = ?2 WHERE id = ?1",
            params![id, remote_id],
        )?;
        Ok(())
    }

    /// Each entry's id on the account playlist, in order; `None` until synced.
    pub fn set_video_ids(&self, id: i64) -> Result<Vec<(String, Option<String>)>> {
        Ok(self.track_ids(id)?.into_iter().map(|(t, _, s)| (t, s)).collect())
    }

    /// Make playlist `id` (or a new one, when `None`) exactly the account's copy.
    pub fn replace_playlist(
        &self,
        id: Option<i64>,
        remote_id: &str,
        name: &str,
        entries: &[(Track, Option<String>)],
    ) -> Result<i64> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let t = now();
        let id = match id {
            Some(id) => {
                tx.execute("UPDATE playlists SET name = ?2 WHERE id = ?1", params![id, clean_name(name)])?;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO playlists (name, created_at, updated_at, remote_id) VALUES (?1, ?2, ?2, ?3)",
                    params![clean_name(name), t, remote_id],
                )?;
                tx.last_insert_rowid()
            }
        };
        let old: HashMap<String, i64> = tx
            .prepare("SELECT track_id, added_at FROM playlist_tracks WHERE playlist_id = ?1")?
            .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .flatten()
            .collect();
        tx.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", params![id])?;
        for (pos, (track, set_id)) in entries.iter().enumerate() {
            Self::upsert_track(&tx, track)?;
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, pos, track_id, added_at, set_video_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, pos as i64, track.id, old.get(&track.id).copied().unwrap_or(t), set_id],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    /// Deleted on the account: remove here without queueing a deletion.
    pub fn drop_playlist(&self, id: i64) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", params![id])?;
        db.execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Stop showing account playlist `remote_id` here; it stays on the account.
    pub fn hide_playlist(&self, remote_id: &str, name: &str) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO hidden_playlists (remote_id, name) VALUES (?1, ?2)",
            params![remote_id, name],
        )?;
        tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id IN (SELECT id FROM playlists WHERE remote_id = ?1)",
            params![remote_id],
        )?;
        tx.execute("DELETE FROM playlists WHERE remote_id = ?1", params![remote_id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn unhide_playlist(&self, remote_id: &str) -> Result<()> {
        self.db
            .lock()
            .unwrap()
            .execute("DELETE FROM hidden_playlists WHERE remote_id = ?1", params![remote_id])?;
        Ok(())
    }

    pub fn hidden_playlists(&self) -> Result<std::collections::HashSet<String>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare("SELECT remote_id FROM hidden_playlists")?;
        let v = st.query_map([], |r| r.get(0))?.flatten().collect();
        Ok(v)
    }

    pub fn pending_playlist_deletes(&self) -> Result<Vec<String>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare("SELECT remote_id FROM deleted_playlists")?;
        let v = st.query_map([], |r| r.get(0))?.flatten().collect();
        Ok(v)
    }

    pub fn clear_playlist_delete(&self, remote_id: &str) -> Result<()> {
        self.db
            .lock()
            .unwrap()
            .execute("DELETE FROM deleted_playlists WHERE remote_id = ?1", params![remote_id])?;
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
        let mut last_played: HashMap<String, i64> = HashMap::new();
        for (id, artist, at, outcome) in rows.flatten() {
            let last = last_played.entry(id.clone()).or_default();
            *last = (*last).max(at);
            let age_days = ((t - at as f64) / DAY as f64).max(0.0);
            let w = Outcome::weight(&outcome) * 0.5f64.powf(age_days / HALF_LIFE_DAYS);
            *tracks.entry(id).or_default() += w;
            *artists.entry(artist).or_default() += w;
        }
        let mut st = db.prepare(
            "SELECT l.track_id, t.artist, l.liked_at, l.imported FROM likes l JOIN tracks t ON t.id = l.track_id",
        )?;
        for (id, artist, liked_at, imported) in st
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, bool>(3)?))
            })?
            .flatten()
        {
            // An imported like's date is unknown (maybe years ago): it counts once played here.
            let since = last_played.get(&id).copied().max((!imported).then_some(liked_at));
            let Some(since) = since else { continue };
            let age_days = ((t - since as f64) / DAY as f64).max(0.0);
            let w = LIKE_WEIGHT * 0.5f64.powf(age_days / LIKE_HALF_LIFE_DAYS);
            *tracks.entry(id).or_default() += w;
            *artists.entry(artist).or_default() += w;
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
    /// - By a blocked artist, or skipped early in the last 14 days: dropped.
    /// - Played in the last 2 hours: dropped (you just heard it).
    /// - Otherwise YouTube's order (it already encodes "sounds like this"), weighed against your
    ///   taste for the artist (relative to your favorite artist) and pushed down when heard
    ///   recently: a lot for yesterday, half for a week ago, barely for a month ago.
    /// - Never more than 2 songs by one artist in any run of 10.
    pub fn rank_radio(&self, candidates: Vec<Track>) -> Result<Vec<Track>> {
        let db = self.db.lock().unwrap();
        let (_, artists) = Self::scores(&db)?;
        let blocked = Self::blocked_keys(&db)?;
        let t = now();
        let skip_cutoff = t - 14 * DAY;
        let just_heard = t - 2 * 3600;
        let mut skipped = db.prepare(
            "SELECT 1 FROM plays WHERE track_id = ?1 AND outcome = 'skipped_early' AND played_at > ?2",
        )?;
        let mut last = db.prepare("SELECT MAX(played_at) FROM plays WHERE track_id = ?1")?;
        let top = artists.values().fold(0.0f64, |m, v| m.max(*v)).max(1.0);

        let mut scored = Vec::new();
        for (rank, c) in candidates.into_iter().enumerate() {
            if blocked.contains(&artist_key(&c)) || skipped.exists(params![c.id, skip_cutoff])? {
                continue;
            }
            let last_played: Option<i64> = last.query_row(params![c.id], |r| r.get(0))?;
            if last_played.is_some_and(|l| l > just_heard) {
                continue;
            }
            let rank_score = 1.0 / (1.0 + rank as f64 / 5.0);
            let affinity = (artists.get(&artist_key(&c)).copied().unwrap_or(0.0) / top).clamp(-1.0, 1.0);
            let heard = last_played.map_or(0.0, |l| 0.5f64.powf((t - l).max(0) as f64 / (7 * DAY) as f64));
            scored.push((0.7 * rank_score + 0.3 * affinity - 0.6 * heard, c));
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        Ok(spread_artists(scored.into_iter().map(|(_, t)| t).collect(), 2, 10))
    }

    fn blocked_keys(db: &Connection) -> Result<std::collections::HashSet<String>> {
        let mut st = db.prepare("SELECT key FROM blocked_artists")?;
        let keys = st.query_map([], |r| r.get::<_, String>(0))?.flatten().collect();
        Ok(keys)
    }

    /// Keep this track's artist out of radio and quick picks. Returns the artist's key.
    pub fn block_artist(&self, track: &Track) -> Result<String> {
        let key = artist_key(track);
        let name = track.artists.first().cloned().unwrap_or_default();
        self.db.lock().unwrap().execute(
            "INSERT OR REPLACE INTO blocked_artists (key, name, blocked_at) VALUES (?1, ?2, ?3)",
            params![key, name, now()],
        )?;
        Ok(key)
    }

    pub fn unblock_artist(&self, key: &str) -> Result<()> {
        self.db.lock().unwrap().execute("DELETE FROM blocked_artists WHERE key = ?1", params![key])?;
        Ok(())
    }

    /// Blocked artists as (key, name), most recent first.
    pub fn blocked_artists(&self) -> Result<Vec<(String, String)>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare("SELECT key, name FROM blocked_artists ORDER BY blocked_at DESC")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.flatten().collect();
        Ok(rows)
    }
}

/// Several seeds' radios as one list. A song scores by where it sits in each radio, weighted
/// by its seed (the first seed counts most), summed over radios, so songs that several of your
/// songs lead to come first. Order within one radio is YouTube's "sounds like this".
pub fn fuse_radios(radios: Vec<Vec<Track>>) -> Vec<Track> {
    let mut scored: Vec<(f64, Track)> = Vec::new();
    for (seed, radio) in radios.into_iter().enumerate() {
        let weight = 1.0 / (1.0 + seed as f64 * 0.25);
        for (pos, t) in radio.into_iter().enumerate() {
            let s = weight / (1.0 + pos as f64 / 5.0);
            match scored.iter_mut().find(|(_, u)| u.id == t.id) {
                Some(e) => e.0 += s,
                None => scored.push((s, t)),
            }
        }
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.into_iter().map(|(_, t)| t).collect()
}

fn clean_name(name: &str) -> String {
    let n = name.trim();
    if n.is_empty() { "New playlist".to_string() } else { n.chars().take(100).collect() }
}

/// The key an artist is scored and blocked by: their channel id, or the lowercased name.
pub fn artist_key(t: &Track) -> String {
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
            artist_ids: vec![],
            album: None,
            album_id: None,
            duration: Some(200),
            thumb: None,
            cover: None,
        }
    }

    #[test]
    fn unliking_a_synced_like_is_remembered() {
        let lib = Library::memory().unwrap();
        let (a, b) = (track("a", "x"), track("b", "x"));
        lib.set_liked(&a, true).unwrap();
        lib.set_liked(&b, true).unwrap();
        lib.mark_liked_remote("a").unwrap();
        lib.set_liked(&a, false).unwrap();
        lib.set_liked(&b, false).unwrap();
        assert_eq!(lib.pending_unlikes().unwrap(), vec!["a".to_string()]);
    }

    #[test]
    fn account_copy_replaces_a_playlist_and_keeps_entry_ids() {
        let lib = Library::memory().unwrap();
        let id = lib.replace_playlist(None, "PL1", "Mix", &[(track("a", "x"), Some("s1".into())), (track("b", "x"), Some("s2".into()))]).unwrap();
        lib.move_in_playlist(id, 0, 1).unwrap();
        let ids = lib.set_video_ids(id).unwrap();
        assert_eq!(ids, vec![("b".into(), Some("s2".into())), ("a".into(), Some("s1".into()))]);
        assert_eq!(lib.playlist_remote(id).unwrap().as_deref(), Some("PL1"));
        lib.replace_playlist(Some(id), "PL1", "Mix 2", &[(track("c", "x"), None)]).unwrap();
        let p = lib.playlist(id).unwrap().unwrap();
        assert_eq!((p.name.as_str(), p.tracks.len()), ("Mix 2", 1));
        lib.delete_playlist(id).unwrap();
        assert_eq!(lib.pending_playlist_deletes().unwrap(), vec!["PL1".to_string()]);
    }

    #[test]
    fn hiding_an_account_playlist_drops_it_here_only() {
        let lib = Library::memory().unwrap();
        lib.replace_playlist(None, "PL1", "Old", &[(track("a", "x"), Some("s1".into()))]).unwrap();
        lib.hide_playlist("PL1", "Old").unwrap();
        assert!(lib.playlists().unwrap().is_empty());
        assert!(lib.pending_playlist_deletes().unwrap().is_empty(), "hiding must not delete on the account");
        assert!(lib.hidden_playlists().unwrap().contains("PL1"));
        lib.unhide_playlist("PL1").unwrap();
        assert!(lib.hidden_playlists().unwrap().is_empty());
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
    fn radio_drops_blocked_artists_and_just_heard_songs() {
        let lib = Library::memory().unwrap();
        let (a, b, c) = (track("a", "x"), track("b", "blocked"), track("c", "y"));
        lib.block_artist(&b).unwrap();
        lib.record_play(&c, now() - 3600, 200.0, 200.0).unwrap();
        let ids: Vec<_> = lib.rank_radio(vec![a, b.clone(), c]).unwrap().into_iter().map(|t| t.id).collect();
        assert_eq!(ids, ["a"]);
        lib.unblock_artist(&artist_key(&b)).unwrap();
        assert_eq!(lib.rank_radio(vec![b]).unwrap().len(), 1);
    }

    #[test]
    fn radio_pushes_down_recently_heard_songs() {
        let lib = Library::memory().unwrap();
        let (heard, fresh, old) = (track("heard", "x"), track("fresh", "y"), track("old", "z"));
        lib.record_play(&heard, now() - DAY, 200.0, 200.0).unwrap();
        lib.record_play(&old, now() - 60 * DAY, 200.0, 200.0).unwrap();
        let ids: Vec<_> =
            lib.rank_radio(vec![heard, old, fresh]).unwrap().into_iter().map(|t| t.id).collect();
        assert_eq!(ids, ["old", "fresh", "heard"], "yesterday's song goes last; two months ago barely counts");
    }

    #[test]
    fn your_favorite_artist_outranks_youtubes_order() {
        let lib = Library::memory().unwrap();
        for i in 0..5 {
            lib.record_play(&track(&format!("fav{i}"), "fav"), now() - (10 + i) * DAY, 200.0, 200.0).unwrap();
        }
        // A long tail of artists heard once each, which used to dilute the favorite to nothing.
        for i in 0..40 {
            lib.record_play(&track(&format!("t{i}"), &format!("tail{i}")), now() - 20 * DAY, 200.0, 200.0).unwrap();
        }
        let mut cands: Vec<Track> = (0..3).map(|i| track(&format!("new{i}"), &format!("other{i}"))).collect();
        cands.push(track("favnew", "fav"));
        let ids: Vec<_> = lib.rank_radio(cands).unwrap().into_iter().map(|t| t.id).collect();
        assert!(ids.iter().position(|i| i == "favnew") < Some(3), "{ids:?}");
    }

    #[test]
    fn fused_radios_favor_songs_several_seeds_share() {
        let r = |ids: &[&str]| ids.iter().map(|i| track(i, i)).collect::<Vec<_>>();
        let fused = fuse_radios(vec![r(&["a", "b", "c", "shared"]), r(&["d", "shared", "e"])]);
        let ids: Vec<_> = fused.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids[0], "shared", "{ids:?}");
        assert_eq!(ids.len(), 6, "no duplicates");
        assert!(ids.iter().position(|&i| i == "a") < ids.iter().position(|&i| i == "d"), "first seed counts more");
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
        assert_eq!(lib.playlists_with("b").unwrap(), [(id, 1)]);
        assert!(lib.playlists_with("a").unwrap().is_empty());
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
    fn imported_likes_count_only_once_played() {
        let lib = Library::memory().unwrap();
        lib.add_remote_likes(&[track("old", "a")]).unwrap();
        assert!(lib.top_tracks(5).unwrap().is_empty(), "a like from the account alone is no favorite");
        lib.record_play(&track("old", "a"), now() - 400 * DAY, 200.0, 200.0).unwrap();
        lib.record_play(&track("new", "b"), now() - DAY, 200.0, 200.0).unwrap();
        lib.set_liked(&track("new", "b"), true).unwrap();
        let top = lib.top_tracks(5).unwrap();
        assert_eq!(top.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), vec!["new"], "played 400 days ago fades out");
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
