//! Songs kept on disk after a full play, so a replay costs no data and starts at once.
//!
//! mpv records the stream as it plays (`stream-record`) to `<id>.<itag>.part.m4a`. A play that
//! reached the end without a seek leaves a whole copy, which becomes `<id>.<itag>.m4a`; anything
//! else is thrown away. Oldest-played songs go first once the cache passes its size cap. A cap of
//! 0 turns recording off; what is already kept still plays until it is cleared.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use anyhow::Result;

use crate::source::{Quality, Stream};

/// AAC ~130 kbps (High) and ~50 kbps (Data saver).
const ITAG_HIGH: u32 = 140;
const ITAG_SAVER: u32 = 139;

pub struct SongCache {
    dir: PathBuf,
    /// Bytes to keep; 0 is off. Changeable while the app runs (a setting).
    cap: AtomicU64,
}

impl SongCache {
    pub fn new(dir: PathBuf, cap: u64) -> SongCache {
        std::fs::create_dir_all(&dir).ok();
        // Recordings cut short by a quit.
        for (path, _) in files(&dir) {
            if is_part(&path) {
                let _ = std::fs::remove_file(path);
            }
        }
        SongCache { dir, cap: AtomicU64::new(cap) }
    }

    /// A kept copy good enough for `quality`: High wants the High stream; Data saver takes any.
    pub fn lookup(&self, id: &str, quality: Quality) -> Option<Stream> {
        let mut itags = vec![ITAG_HIGH];
        if quality == Quality::DataSaver {
            itags.push(ITAG_SAVER);
        }
        itags.into_iter().find_map(|itag| {
            let path = self.path(id, itag, false);
            let len = std::fs::metadata(&path).ok()?.len();
            // Mark it played, for the oldest-first trim.
            if let Ok(f) = std::fs::File::options().write(true).open(&path) {
                let _ = f.set_modified(SystemTime::now());
            }
            Some(local_stream(id, &path, itag, len))
        })
    }

    /// Where a play of this stream records to. Nowhere when the cache is off.
    pub fn record_path(&self, s: &Stream) -> Option<PathBuf> {
        let on = self.cap.load(Ordering::Relaxed) > 0;
        (on && matches!(s.itag, ITAG_HIGH | ITAG_SAVER)).then(|| self.path(&s.video_id, s.itag, true))
    }

    /// Change the size cap (0 turns recording off) and trim to it at once.
    pub fn set_cap(&self, bytes: u64) {
        self.cap.store(bytes, Ordering::Relaxed);
        self.trim();
    }

    /// A recording finished cleanly: keep it if it holds the whole stream, then trim.
    pub fn keep(&self, part: &Path, expected: Option<u64>) -> Result<bool> {
        let len = std::fs::metadata(part)?.len();
        // mpv rewrites the container, so a whole copy comes out a few KB smaller.
        let whole = expected.is_some_and(|e| len * 100 >= e * 97);
        // Turned off while it recorded.
        if !whole || self.cap.load(Ordering::Relaxed) == 0 {
            log::debug!("song cache: {} incomplete ({len} of {expected:?} bytes)", part.display());
            std::fs::remove_file(part)?;
            return Ok(false);
        }
        let name = part.file_name().and_then(|n| n.to_str()).unwrap_or_default().replace(".part.m4a", ".m4a");
        std::fs::rename(part, self.dir.join(name))?;
        self.trim();
        Ok(true)
    }

    /// Songs kept, and bytes they take.
    pub fn usage(&self) -> (usize, u64) {
        let kept: Vec<_> = files(&self.dir).into_iter().filter(|(p, _)| !is_part(p)).collect();
        (kept.len(), kept.iter().map(|(_, m)| m.len()).sum())
    }

    /// Delete every kept song (recordings in progress finish normally).
    pub fn clear(&self) {
        for (path, _) in files(&self.dir) {
            if !is_part(&path) {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    pub fn size(&self) -> u64 {
        files(&self.dir).iter().filter(|(p, _)| !is_part(p)).map(|(_, m)| m.len()).sum()
    }

    fn trim(&self) {
        let cap = self.cap.load(Ordering::Relaxed);
        // Off keeps what is there; clearing is the user's call.
        if cap == 0 {
            return;
        }
        let mut kept: Vec<_> = files(&self.dir).into_iter().filter(|(p, _)| !is_part(p)).collect();
        let mut total: u64 = kept.iter().map(|(_, m)| m.len()).sum();
        kept.sort_by_key(|(_, m)| m.modified().unwrap_or(SystemTime::UNIX_EPOCH));
        for (path, meta) in kept {
            if total <= cap {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total -= meta.len();
            }
        }
    }

    fn path(&self, id: &str, itag: u32, part: bool) -> PathBuf {
        self.dir.join(format!("{id}.{itag}{}.m4a", if part { ".part" } else { "" }))
    }
}

/// A copy on disk as a stream to play: no network, no recording.
pub(crate) fn local_stream(id: &str, path: &Path, itag: u32, len: u64) -> Stream {
    Stream {
        video_id: id.to_string(),
        url: path.to_string_lossy().into_owned(),
        itag,
        bitrate: if itag == ITAG_SAVER { 50_000 } else { 130_000 },
        codec: "mp4a.40.2".into(),
        user_agent: String::new(),
        content_length: Some(len),
        expires_at: i64::MAX,
        record_to: None,
        local: true,
        loudness_db: None,
        gain_db: None,
        meta: None,
    }
}

fn is_part(p: &Path) -> bool {
    p.to_string_lossy().ends_with(".part.m4a")
}

fn files(dir: &Path) -> Vec<(PathBuf, std::fs::Metadata)> {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().filter_map(|e| Some((e.path(), e.metadata().ok()?))).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(id: &str, itag: u32) -> Stream {
        Stream {
            video_id: id.into(),
            url: String::new(),
            itag,
            bitrate: 0,
            codec: String::new(),
            user_agent: String::new(),
            content_length: Some(1000),
            expires_at: 0,
            record_to: None,
            local: false,
            loudness_db: None,
            gain_db: None,
            meta: None,
        }
    }

    #[test]
    fn keeps_whole_recordings_and_trims_oldest() {
        let dir = std::env::temp_dir().join(format!("hum-cache-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let c = SongCache::new(dir.clone(), 2500);
        for (id, len) in [("a", 990), ("b", 500), ("c", 990), ("d", 990)] {
            let part = c.record_path(&stream(id, ITAG_HIGH)).unwrap();
            std::fs::write(&part, vec![0u8; len]).unwrap();
            c.keep(&part, Some(1000)).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(c.lookup("b", Quality::High).is_none(), "half a song is not kept");
        assert!(c.lookup("a", Quality::High).is_none(), "oldest goes past the cap");
        assert!(c.lookup("d", Quality::High).is_some_and(|s| s.local && s.url.ends_with("d.140.m4a")));
        assert!(c.lookup("d", Quality::DataSaver).is_some(), "data saver takes a better copy");
        assert!(c.size() <= 2500);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn limit_zero_stops_recording_and_lowering_trims() {
        let dir = std::env::temp_dir().join(format!("hum-cache-limit-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let c = SongCache::new(dir.clone(), 10_000);
        for id in ["a", "b", "c"] {
            let part = c.record_path(&stream(id, ITAG_HIGH)).unwrap();
            std::fs::write(&part, vec![0u8; 990]).unwrap();
            c.keep(&part, Some(1000)).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        c.set_cap(2000);
        assert!(c.size() <= 2000 && c.lookup("a", Quality::High).is_none(), "lowering trims the oldest");
        assert!(c.lookup("c", Quality::High).is_some());
        c.set_cap(0);
        assert!(c.record_path(&stream("d", ITAG_HIGH)).is_none(), "off records nothing");
        assert!(c.lookup("c", Quality::High).is_some(), "off still plays what is kept");
        assert_eq!(c.usage().0, 2, "off does not trim");
        c.set_cap(5000);
        assert!(c.record_path(&stream("d", ITAG_HIGH)).is_some());
        let _ = std::fs::remove_dir_all(dir);
    }
}
