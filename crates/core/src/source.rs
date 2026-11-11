//! YouTube Music metadata and stream resolution, through Sonora's `ytmusic-rs`.
//!
//! Why not rustypipe: on 2026-11-11 rustypipe 0.11.4 failed to decipher every client but iOS,
//! and iOS URLs are refused after the first megabyte without a PO token. ytmusic-rs asks the
//! VisionOS guest client first, whose URLs need no deciphering and no token, and they accept
//! open-ended ranges, so mpv can play them directly. See spike/RESULTS.md.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use serde::Serialize;
use ytmusic::YtMusic;

/// Sound quality, as the user picks it in Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    /// The best stream the connection handles. Currently the same as High; stepping down when
    /// mpv starts stalling for cache is the next step.
    #[default]
    Auto,
    /// ~50 kbps. About 0.4 MB per minute.
    DataSaver,
    /// ~130 kbps.
    Normal,
    /// The best stream available.
    High,
}

/// Bits-per-second ceiling for data saver.
const SAVER_CEILING: u32 = 80_000;

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub artist_id: Option<String>,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub duration: Option<u32>,
    /// Cover URL sized for a list row.
    pub thumb: Option<String>,
    /// Cover URL sized for Now Playing.
    pub cover: Option<String>,
}

/// googleusercontent thumbnails take their size from a `=w..-h..` suffix, so any size can be
/// asked for regardless of which sizes the API listed.
fn sized(url: &str, px: u32) -> String {
    match url.rfind('=') {
        Some(i) if url[i..].starts_with("=w") => format!("{}=w{px}-h{px}-l90-rj", &url[..i]),
        _ => url.to_string(),
    }
}

impl Track {
    fn from_ytm(t: ytmusic::Track) -> Option<Track> {
        let id = t.video_id?;
        let best = t
            .thumbnails
            .iter()
            .max_by_key(|th| th.width)
            .map(|th| th.url.clone());
        Some(Track {
            id,
            title: t.title,
            artist_id: t.artists.first().and_then(|a| a.id.clone()),
            artists: t.artists.into_iter().map(|a| a.name).collect(),
            album: t.album.as_ref().map(|a| a.name.clone()),
            album_id: t.album.and_then(|a| a.id),
            duration: t.duration.map(|d| d.as_secs() as u32),
            thumb: best.as_deref().map(|u| sized(u, 120)),
            cover: best.as_deref().map(|u| sized(u, 544)),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Stream {
    pub video_id: String,
    pub url: String,
    pub itag: u32,
    pub bitrate: u32,
    pub codec: String,
    pub user_agent: String,
    pub content_length: Option<u64>,
    /// Unix seconds after which the URL stops working.
    pub expires_at: i64,
}

pub struct Source {
    ytm: Arc<YtMusic>,
}

impl Source {
    pub fn new(storage_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&storage_dir).ok();
        let ytm = YtMusic::anonymous()
            .cache_player(storage_dir.join("player.json"))
            .cache_resolutions(storage_dir.join("resolutions.json"));
        Ok(Source { ytm: Arc::new(ytm) })
    }

    /// Open connections and fetch the visitor session, so the first real click doesn't pay for
    /// them.
    pub async fn warm(&self) {
        let t = Instant::now();
        match self.ytm.best_audio("dQw4w9WgXcQ").await {
            Ok(_) => log::info!("source warm in {:?}", t.elapsed()),
            Err(e) => log::warn!("source warm-up failed: {e:#}"),
        }
    }

    pub async fn search_tracks(&self, query: &str) -> Result<Vec<Track>> {
        let res = self.ytm.search_songs(query).await?;
        Ok(res.into_iter().filter_map(Track::from_ytm).collect())
    }

    pub async fn radio(&self, video_id: &str) -> Result<Vec<Track>> {
        let res = self.ytm.track_radio(video_id).await?;
        Ok(res.into_iter().filter_map(Track::from_ytm).collect())
    }

    pub async fn resolve(&self, video_id: &str, quality: Quality) -> Result<Stream> {
        let t = Instant::now();
        let fmt = match quality {
            Quality::DataSaver => {
                // Opening the body is the only public way to pick by bitrate; dropping the
                // stream right away aborts it after the first range.
                let (fmt, stream) = self.ytm.open_audio_within(video_id, Some(SAVER_CEILING)).await?;
                drop(stream);
                fmt
            }
            Quality::Auto | Quality::Normal | Quality::High => self.ytm.best_audio(video_id).await?,
        };
        let stream = Stream {
            video_id: video_id.to_string(),
            expires_at: expiry(&fmt.url),
            url: fmt.url,
            itag: fmt.itag,
            bitrate: fmt.bitrate,
            codec: fmt.codec,
            user_agent: fmt.user_agent.to_string(),
            content_length: fmt.content_length,
        };
        log::debug!(
            "resolved {video_id} itag {} {}k in {:?}",
            stream.itag,
            stream.bitrate / 1000,
            t.elapsed()
        );
        Ok(stream)
    }
}

/// The `expire=` unix timestamp in a googlevideo URL, or five hours from now.
fn expiry(url: &str) -> i64 {
    url.split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("expire="))
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64 + 5 * 3600)
                .unwrap_or(0)
        })
}
