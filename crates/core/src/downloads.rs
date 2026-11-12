//! Songs kept for offline: every song of a playlist (or Liked songs) the user marked.
//!
//! One file per song, `<id>.<itag>.m4a`, written as `<id>.<itag>.m4a.part` and renamed once whole.
//! Unlike the song cache these have no size cap and live in the data dir, where the system
//! won't wipe them. A file whose song no collection wants any more is deleted (`sweep`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::cache::local_stream;
use crate::source::Stream;

pub struct Downloads {
    dir: PathBuf,
}

/// One marked collection: how many songs it has and how many of them are on disk.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct DownloadTarget {
    pub target: String,
    pub total: usize,
    pub done: usize,
}

#[derive(Debug, Serialize)]
pub struct DownloadStatus {
    pub targets: Vec<DownloadTarget>,
    /// Files on disk, and their bytes.
    pub count: usize,
    pub bytes: u64,
    /// A download pass is running (or waiting to retry).
    pub active: bool,
    /// Songs are left to fetch but the phone is on mobile data, and that's not allowed.
    pub waiting_for_wifi: bool,
}

impl Downloads {
    pub fn new(dir: PathBuf) -> Downloads {
        std::fs::create_dir_all(&dir).ok();
        // Downloads cut short by a quit.
        for (path, _) in files(&dir) {
            if is_part(&path) {
                let _ = std::fs::remove_file(path);
            }
        }
        Downloads { dir }
    }

    /// The song's file as a stream to play, whatever its itag.
    pub fn lookup(&self, id: &str) -> Option<Stream> {
        files(&self.dir).into_iter().find_map(|(path, meta)| {
            let (file_id, itag) = parse(&path)?;
            (file_id == id).then(|| local_stream(id, &path, itag, meta.len()))
        })
    }

    /// Ids of the songs on disk.
    pub fn have(&self) -> HashSet<String> {
        self.done().map(|(id, _, _)| id).collect()
    }

    /// Songs on disk, and bytes they take.
    pub fn usage(&self) -> (usize, u64) {
        self.done().fold((0, 0), |(n, b), (_, _, len)| (n + 1, b + len))
    }

    /// Delete the songs no collection wants. Returns how many went.
    pub fn sweep(&self, wanted: &HashSet<String>) -> usize {
        let unwanted = self.done_files().filter(|(path, id, _)| !wanted.contains(id) && std::fs::remove_file(path).is_ok());
        unwanted.count()
    }

    /// Delete every song (a download in progress ends up swept on the next pass).
    pub fn clear(&self) {
        self.sweep(&HashSet::new());
    }

    /// Where a download of this song is written until it is whole.
    pub fn part_path(&self, id: &str, itag: u32) -> PathBuf {
        self.dir.join(format!("{id}.{itag}.m4a.part"))
    }

    /// The download in `part` is whole: give it its real name.
    pub fn finish(&self, part: &Path) -> Result<()> {
        let name = part.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        std::fs::rename(part, self.dir.join(name.trim_end_matches(".part")))?;
        Ok(())
    }

    /// Take a copy of a song already on disk elsewhere (the song cache) instead of fetching it.
    pub fn adopt(&self, from: &Path, id: &str, itag: u32) -> Result<()> {
        let part = self.part_path(id, itag);
        if let Err(e) = std::fs::copy(from, &part) {
            let _ = std::fs::remove_file(&part);
            return Err(e.into());
        }
        self.finish(&part)
    }

    fn done_files(&self) -> impl Iterator<Item = (PathBuf, String, u64)> {
        files(&self.dir).into_iter().filter_map(|(path, meta)| {
            let (id, _) = parse(&path)?;
            Some((path, id, meta.len()))
        })
    }

    fn done(&self) -> impl Iterator<Item = (String, PathBuf, u64)> {
        self.done_files().map(|(path, id, len)| (id, path, len))
    }
}

impl DownloadStatus {
    /// `sets` are the marked collections with their song ids.
    pub fn new(sets: Vec<(String, Vec<String>)>, downloads: &Downloads, active: bool) -> DownloadStatus {
        let have = downloads.have();
        let targets = sets
            .into_iter()
            .map(|(target, ids)| DownloadTarget {
                total: ids.len(),
                done: ids.iter().filter(|id| have.contains(*id)).count(),
                target,
            })
            .collect();
        let (count, bytes) = downloads.usage();
        DownloadStatus { targets, count, bytes, active, waiting_for_wifi: false }
    }
}

/// `<id>.<itag>.m4a` -> (id, itag). Ids hold no dots.
fn parse(path: &Path) -> Option<(String, u32)> {
    let name = path.file_name()?.to_str()?.strip_suffix(".m4a")?;
    let (id, itag) = name.split_once('.')?;
    Some((id.to_string(), itag.parse().ok()?))
}

fn is_part(p: &Path) -> bool {
    p.to_string_lossy().ends_with(".part")
}

fn files(dir: &Path) -> Vec<(PathBuf, std::fs::Metadata)> {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().filter_map(|e| Some((e.path(), e.metadata().ok()?))).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hum-{name}-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn put(d: &Downloads, id: &str, itag: u32, len: usize) {
        let part = d.part_path(id, itag);
        std::fs::write(&part, vec![0u8; len]).unwrap();
        d.finish(&part).unwrap();
    }

    fn ids(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn sweep_deletes_only_unwanted_songs() {
        let dir = temp("sweep");
        let d = Downloads::new(dir.clone());
        put(&d, "a", 140, 100);
        put(&d, "b", 140, 200);
        put(&d, "c_-x", 139, 300);
        std::fs::write(d.part_path("e", 140), b"half").unwrap();
        assert_eq!(d.usage(), (3, 600), "a part is not a song");
        assert_eq!(d.sweep(&ids(&["a", "c_-x", "zzz"])), 1);
        assert_eq!(d.have(), ids(&["a", "c_-x"]));
        assert!(d.part_path("e", 140).exists(), "a download in progress is left alone");
        d.clear();
        assert_eq!(d.usage(), (0, 0));
        // Parts from a run that quit are gone on the next start.
        let d = Downloads::new(dir.clone());
        assert!(!d.part_path("e", 140).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn lookup_finds_any_itag_and_status_counts_done() {
        let dir = temp("lookup");
        let d = Downloads::new(dir.clone());
        put(&d, "a", 139, 50);
        put(&d, "b", 140, 70);
        let s = d.lookup("a").unwrap();
        assert!(s.local && s.itag == 139 && s.record_to.is_none() && s.expires_at == i64::MAX);
        assert!(s.url.ends_with("a.139.m4a") && s.content_length == Some(50));
        assert!(d.lookup("zz").is_none());
        let sets = vec![("liked".to_string(), vec!["a".into(), "q".into()]), ("playlist:3".to_string(), vec!["b".into()])];
        let st = DownloadStatus::new(sets, &d, true);
        assert_eq!(st.targets[0], DownloadTarget { target: "liked".into(), total: 2, done: 1 });
        assert_eq!(st.targets[1].done, 1);
        assert_eq!((st.count, st.bytes, st.active), (2, 120, true));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn adopt_copies_and_keeps_the_original() {
        let dir = temp("adopt");
        let d = Downloads::new(dir.join("dl"));
        std::fs::write(dir.join("src.m4a"), b"song").unwrap();
        d.adopt(&dir.join("src.m4a"), "a", 140).unwrap();
        assert!(d.lookup("a").is_some() && dir.join("src.m4a").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
