//! Keeps likes and playlists in step with the signed-in YouTube Music account.
//!
//! Local changes apply at once; what the account needs to hear goes through one queue, in order.
//! A pull makes hum match the account: the account wins for what it has, hum's unsent likes and
//! playlists go up, and what was deleted on another device goes here too.

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::json;
use tokio::sync::mpsc;
use ytmusic::YtMusic;

use crate::account::Account;
use crate::library::Library;
use crate::source::Track;

enum Op {
    Pull,
    PullLikes,
    Refresh(i64),
    Like(String, bool),
    Upload(i64),
    Rename(String, String),
    Delete(String),
    Add(i64, String, Vec<String>),
    Remove(String, String, String),
    Move(String, String, Option<String>),
}

#[derive(Clone)]
pub struct Sync {
    lib: Library,
    account: Arc<Account>,
    tx: mpsc::UnboundedSender<Op>,
}

/// What the app hears from the worker.
pub trait Events: Send + std::marker::Sync + 'static {
    /// Likes or playlists changed here because of the account.
    fn changed(&self);
    fn failed(&self, what: &str, error: &anyhow::Error);
}

impl Sync {
    pub fn start(lib: Library, account: Arc<Account>, events: Arc<dyn Events>) -> Sync {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sync = Sync { lib, account, tx };
        let worker = sync.clone();
        tokio::spawn(async move {
            while let Some(op) = rx.recv().await {
                let Some(ytm) = worker.account.client() else { continue };
                let (what, changes) = op.describe();
                match worker.apply(&ytm, op).await {
                    Ok(()) if changes => events.changed(),
                    Ok(()) => {}
                    Err(e) => {
                        log::warn!("sync: {what}: {e:#}");
                        events.failed(what, &e);
                    }
                }
            }
        });
        sync
    }

    fn send(&self, op: Op) {
        if self.account.is_signed_in() {
            let _ = self.tx.send(op);
        }
    }

    pub fn pull(&self) {
        self.send(Op::Pull);
    }

    pub fn pull_likes(&self) {
        self.send(Op::PullLikes);
    }

    pub fn refresh_playlist(&self, id: i64) {
        if let Ok(Some(_)) = self.lib.playlist_remote(id) {
            self.send(Op::Refresh(id));
        }
    }

    pub fn set_liked(&self, track: &Track, liked: bool) -> Result<()> {
        self.lib.set_liked(track, liked)?;
        self.send(Op::Like(track.id.clone(), liked));
        Ok(())
    }

    pub fn create_playlist(&self, name: &str, tracks: &[Track]) -> Result<i64> {
        let id = self.lib.create_playlist(name)?;
        self.lib.add_to_playlist(id, tracks)?;
        self.send(Op::Upload(id));
        Ok(id)
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<()> {
        self.lib.rename_playlist(id, name)?;
        if let Some(remote) = self.lib.playlist_remote(id)? {
            self.send(Op::Rename(remote, name.to_string()));
        }
        Ok(())
    }

    pub fn delete_playlist(&self, id: i64) -> Result<()> {
        let remote = self.lib.playlist_remote(id)?;
        self.lib.delete_playlist(id)?;
        if let Some(remote) = remote {
            self.send(Op::Delete(remote));
        }
        Ok(())
    }

    pub fn add_to_playlist(&self, id: i64, tracks: &[Track]) -> Result<usize> {
        let before: HashSet<String> = self.lib.set_video_ids(id)?.into_iter().map(|(t, _)| t).collect();
        let added = self.lib.add_to_playlist(id, tracks)?;
        if let Some(remote) = self.lib.playlist_remote(id)? {
            let ids: Vec<String> =
                tracks.iter().filter(|t| !before.contains(&t.id)).map(|t| t.id.clone()).collect();
            if !ids.is_empty() {
                self.send(Op::Add(id, remote, ids));
            }
        }
        Ok(added)
    }

    pub fn remove_from_playlist(&self, id: i64, index: usize) -> Result<()> {
        let entries = self.lib.set_video_ids(id)?;
        self.lib.remove_from_playlist(id, index)?;
        if let (Some(remote), Some((track, set_id))) = (self.lib.playlist_remote(id)?, entries.get(index)) {
            match set_id {
                Some(set_id) => self.send(Op::Remove(remote, track.clone(), set_id.clone())),
                None => self.send(Op::Refresh(id)),
            }
        }
        Ok(())
    }

    pub fn move_in_playlist(&self, id: i64, from: usize, to: usize) -> Result<()> {
        let mut entries = self.lib.set_video_ids(id)?;
        self.lib.move_in_playlist(id, from, to)?;
        let Some(remote) = self.lib.playlist_remote(id)? else { return Ok(()) };
        if from >= entries.len() || to >= entries.len() || from == to {
            return Ok(());
        }
        let moved = entries.remove(from);
        entries.insert(to, moved);
        // YouTube moves an entry to just before another one, or to the end.
        let successor = entries.get(to + 1).map(|(_, s)| s.clone());
        match (&entries[to].1, successor) {
            (Some(set_id), None) => self.send(Op::Move(remote, set_id.clone(), None)),
            (Some(set_id), Some(Some(next))) => self.send(Op::Move(remote, set_id.clone(), Some(next))),
            _ => self.send(Op::Refresh(id)),
        }
        Ok(())
    }

    async fn apply(&self, ytm: &YtMusic, op: Op) -> Result<()> {
        match op {
            Op::Pull => {
                self.apply_likes(ytm).await?;
                self.apply_playlists(ytm).await
            }
            Op::PullLikes => self.apply_likes(ytm).await,
            Op::Refresh(id) => {
                if let Some(remote) = self.lib.playlist_remote(id)? {
                    self.fetch(ytm, Some(id), &remote, None).await?;
                }
                Ok(())
            }
            Op::Like(id, liked) => {
                ytm.rate_track(&id, liked).await?;
                match liked {
                    true => self.lib.mark_liked_remote(&id),
                    false => self.lib.clear_unlike(&id),
                }
            }
            Op::Upload(id) => self.upload(ytm, id).await,
            Op::Rename(remote, name) => ytm.rename_playlist(&remote, &name).await,
            Op::Delete(remote) => {
                ytm.delete_playlist(&remote).await?;
                self.lib.clear_playlist_delete(&remote)
            }
            Op::Add(id, remote, ids) => {
                for v in &ids {
                    ytm.add_playlist_track(&remote, v).await?;
                }
                // Learn the new entries' ids, needed to remove or move them later.
                self.fetch(ytm, Some(id), &remote, None).await
            }
            Op::Remove(remote, track, set_id) => ytm.remove_playlist_track(&remote, &track, &set_id).await,
            Op::Move(remote, set_id, successor) => {
                let mut action = json!({ "action": "ACTION_MOVE_VIDEO_BEFORE", "setVideoId": set_id });
                if let Some(next) = successor {
                    action["movedSetVideoIdSuccessor"] = json!(next);
                }
                let reply = ytm
                    .execute(
                        "browse/edit_playlist",
                        ytmusic::Client::Music,
                        json!({ "playlistId": remote.trim_start_matches("VL"), "actions": [action] }),
                    )
                    .await?;
                match reply["status"].as_str() {
                    Some("STATUS_SUCCEEDED") => Ok(()),
                    s => anyhow::bail!("YouTube refused the move ({s:?})"),
                }
            }
        }
    }

    async fn apply_likes(&self, ytm: &YtMusic) -> Result<()> {
        let remote: Vec<Track> =
            ytm.liked_songs().await.context("reading liked songs")?.into_iter().filter_map(Track::from_ytm).collect();
        let mut on_account: HashSet<String> = remote.iter().map(|t| t.id.clone()).collect();
        for id in self.lib.pending_unlikes()? {
            if on_account.remove(&id) {
                ytm.rate_track(&id, false).await?;
            }
            self.lib.clear_unlike(&id)?;
        }
        let mut here = HashSet::new();
        for (id, sent) in self.lib.like_states()? {
            if on_account.contains(&id) {
                if !sent {
                    self.lib.mark_liked_remote(&id)?;
                }
            } else if sent {
                self.lib.drop_like(&id)?;
            } else {
                ytm.rate_track(&id, true).await?;
                self.lib.mark_liked_remote(&id)?;
            }
            here.insert(id);
        }
        let new: Vec<Track> = remote.into_iter().filter(|t| on_account.contains(&t.id) && !here.contains(&t.id)).collect();
        self.lib.add_remote_likes(&new)
    }

    async fn apply_playlists(&self, ytm: &YtMusic) -> Result<()> {
        let mut lists: Vec<ytmusic::Playlist> = ytm
            .library_playlists()
            .await
            .context("reading playlists")?
            .into_iter()
            .filter(|p| p.owned && !matches!(bare(&p.id), "LM" | "SE"))
            .collect();
        for remote in self.lib.pending_playlist_deletes()? {
            if let Some(i) = lists.iter().position(|p| bare(&p.id) == bare(&remote)) {
                ytm.delete_playlist(&remote).await?;
                lists.remove(i);
            }
            self.lib.clear_playlist_delete(&remote)?;
        }
        let links = self.lib.playlist_links()?;
        for (id, remote) in &links {
            match remote {
                Some(r) if !lists.iter().any(|p| bare(&p.id) == bare(r)) => self.lib.drop_playlist(*id)?,
                Some(_) => {}
                None => self.upload(ytm, *id).await?,
            }
        }
        for p in &lists {
            let local = links.iter().find(|(_, r)| r.as_deref().map(bare) == Some(bare(&p.id))).map(|(id, _)| *id);
            self.fetch(ytm, local, &p.id, Some(&p.title)).await?;
        }
        Ok(())
    }

    /// Replace the local copy of account playlist `remote` with the account's.
    async fn fetch(&self, ytm: &YtMusic, local: Option<i64>, remote: &str, title: Option<&str>) -> Result<()> {
        let detail = ytm.playlist(remote).await.with_context(|| format!("reading playlist {remote}"))?;
        let title = title.map(str::to_string).unwrap_or(detail.playlist.title);
        let entries: Vec<(Track, Option<String>)> = detail
            .tracks
            .into_iter()
            .filter_map(|t| {
                let set_id = t.set_video_id.clone();
                Track::from_ytm(t).map(|t| (t, set_id))
            })
            .collect();
        self.lib.replace_playlist(local, bare(remote), &title, &entries)?;
        Ok(())
    }

    async fn upload(&self, ytm: &YtMusic, id: i64) -> Result<()> {
        let Some(list) = self.lib.playlist(id)? else { return Ok(()) };
        if self.lib.playlist_remote(id)?.is_some() {
            return Ok(());
        }
        let remote = ytm.create_playlist(&list.name).await?;
        self.lib.link_playlist(id, &remote)?;
        for t in &list.tracks {
            ytm.add_playlist_track(&remote, &t.id).await?;
        }
        self.fetch(ytm, Some(id), &remote, Some(&list.name)).await
    }
}

impl Op {
    /// A name for errors, and whether a success changes what hum shows.
    fn describe(&self) -> (&'static str, bool) {
        match self {
            Op::Pull => ("syncing with YouTube Music", true),
            Op::PullLikes => ("syncing liked songs", true),
            Op::Refresh(_) => ("syncing a playlist", true),
            Op::Like(_, true) => ("liking a song on YouTube Music", false),
            Op::Like(_, false) => ("unliking a song on YouTube Music", false),
            Op::Upload(_) => ("uploading a playlist", true),
            Op::Rename(..) => ("renaming a playlist on YouTube Music", false),
            Op::Delete(_) => ("deleting a playlist on YouTube Music", false),
            Op::Add(..) => ("adding to a playlist on YouTube Music", true),
            Op::Remove(..) => ("removing from a playlist on YouTube Music", false),
            Op::Move(..) => ("moving a song on YouTube Music", false),
        }
    }
}

fn bare(id: &str) -> &str {
    id.strip_prefix("VL").unwrap_or(id)
}
