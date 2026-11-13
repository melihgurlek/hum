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
    /// High, stepping down to DataSaver when playback stalls (see the engine).
    #[default]
    Auto,
    /// ~50 kbps AAC (itag 139). About 0.4 MB per minute.
    DataSaver,
    /// Kept so old settings still load. YouTube's AAC streams come in two sizes only, so this
    /// was always the same stream as High; it plays as High.
    Normal,
    /// The best stream available: ~130 kbps AAC (itag 140), about 1 MB per minute.
    High,
}

/// Bits-per-second ceiling for data saver. The library prefers AAC, whose small stream is
/// ~50 kbps and large one ~130 kbps.
const SAVER_CEILING: u32 = 80_000;

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub artist_id: Option<String>,
    /// Each artist's channel id, in `artists` order (missing for songs saved before this was
    /// kept; `artist_id` still has the first).
    #[serde(default)]
    pub artist_ids: Vec<Option<String>>,
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

/// Same as [`sized`], for a non-square image (artist banners).
fn sized_wh(url: &str, w: u32, h: u32) -> String {
    match url.rfind('=') {
        Some(i) if url[i..].starts_with("=w") => format!("{}=w{w}-h{h}-p-l90-rj", &url[..i]),
        _ => url.to_string(),
    }
}

fn best_thumb(thumbs: &[ytmusic::Thumbnail]) -> Option<&str> {
    thumbs.iter().max_by_key(|th| th.width).map(|th| th.url.as_str())
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtistLink {
    pub name: String,
    pub id: Option<String>,
}

impl From<ytmusic::ArtistRef> for ArtistLink {
    fn from(a: ytmusic::ArtistRef) -> Self {
        ArtistLink { name: a.name, id: a.id }
    }
}

/// An album or single, as a card on an artist page.
#[derive(Clone, Debug, Serialize)]
pub struct AlbumCard {
    pub id: String,
    pub title: String,
    /// "Album", "Single", "EP" or "Compilation".
    pub kind: &'static str,
    pub year: Option<i32>,
    pub artists: Vec<ArtistLink>,
    pub thumb: Option<String>,
}

fn kind_name(k: ytmusic::AlbumKind) -> &'static str {
    match k {
        ytmusic::AlbumKind::Album => "Album",
        ytmusic::AlbumKind::Single => "Single",
        ytmusic::AlbumKind::Ep => "EP",
        ytmusic::AlbumKind::Compilation => "Compilation",
    }
}

impl AlbumCard {
    fn from_ytm(a: ytmusic::Album) -> AlbumCard {
        AlbumCard {
            thumb: best_thumb(&a.thumbnails).map(|u| sized(u, 352)),
            id: a.browse_id,
            title: a.title,
            kind: kind_name(a.kind),
            year: a.year,
            artists: a.artists.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtistPage {
    pub id: String,
    pub name: String,
    pub subscribers: Option<String>,
    /// Wide header image, when the artist has one.
    pub banner: Option<String>,
    pub top: Vec<Track>,
    pub albums: Vec<AlbumCard>,
    pub singles: Vec<AlbumCard>,
}

/// YouTube Music's own top search result, when it is an artist or an album. Songs and music
/// videos are left to the songs search, which returns the audio track.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TopResult {
    Artist { id: String, name: String, detail: Option<String>, thumb: Option<String> },
    Album { id: String, title: String, detail: Option<String>, thumb: Option<String> },
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchResults {
    pub top: Option<TopResult>,
    pub songs: Vec<Track>,
}

/// A shelf of YouTube Music's home page: songs, or cards that open or play something.
#[derive(Clone, Debug, Serialize)]
pub struct HomeShelf {
    pub title: String,
    pub songs: Vec<Track>,
    pub cards: Vec<HomeCard>,
}

/// Tagged by `type`, since an album card has its own `kind` ("Single", "EP").
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HomeCard {
    Album(AlbumCard),
    Playlist { id: String, title: String, subtitle: Option<String>, thumb: Option<String> },
}

fn home_shelf(shelf: &serde_json::Value) -> Option<HomeShelf> {
    let title = ytmusic::parse::runs_text(shelf.pointer("/header/musicCarouselShelfBasicHeaderRenderer/title")?)?;
    let (mut songs, mut cards) = (Vec::new(), Vec::new());
    for item in shelf.get("contents")?.as_array()? {
        if item.get("musicResponsiveListItemRenderer").is_some() {
            songs.extend(ytmusic::parse::list_item_track(item).and_then(Track::from_ytm));
        } else if let Some(p) = ytmusic::parse::two_row_playlist(item) {
            let subtitle = item.pointer("/musicTwoRowItemRenderer/subtitle").and_then(ytmusic::parse::runs_text);
            cards.push(HomeCard::Playlist {
                thumb: best_thumb(&p.thumbnails).map(|u| sized(u, 352)),
                id: p.id,
                title: p.title,
                subtitle,
            });
        } else if let Some(a) = ytmusic::parse::two_row_album(item) {
            cards.push(HomeCard::Album(AlbumCard::from_ytm(a)));
        }
    }
    (!songs.is_empty() || !cards.is_empty()).then_some(HomeShelf { title, songs, cards })
}

#[derive(Clone, Debug, Serialize)]
pub struct AlbumPage {
    pub card: AlbumCard,
    /// Cover sized for the page header.
    pub cover: Option<String>,
    pub tracks: Vec<Track>,
}

impl Track {
    pub(crate) fn from_ytm(t: ytmusic::Track) -> Option<Track> {
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
            artist_ids: t.artists.iter().map(|a| a.id.clone()).collect(),
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
    /// Where mpv records this play for the song cache.
    #[serde(skip)]
    pub record_to: Option<PathBuf>,
    /// A copy from the song cache: no network.
    pub local: bool,
    /// How much louder than YouTube's reference level the song is, in dB (YouTube's
    /// `loudnessDb`). Unknown for a cached copy until the engine looks it up.
    pub loudness_db: Option<f32>,
    /// Volume change to play it at, in dB, decided by the engine (loudness normalization).
    #[serde(skip)]
    pub gain_db: Option<f64>,
    /// What the system shows for it (Android's notification and lock screen), set by the
    /// engine as it hands the stream to the player.
    #[serde(skip)]
    pub meta: Option<TrackMeta>,
}

/// A song's title, artists, album and cover, for the system's media controls.
#[derive(Clone, Debug, Default)]
pub struct TrackMeta {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub artwork: Option<String>,
    /// The cover's copy in the app's image cache, when there is one: it shows offline too.
    pub artwork_file: Option<String>,
}

impl From<&Track> for TrackMeta {
    fn from(t: &Track) -> Self {
        let artwork_file = [&t.cover, &t.thumb]
            .into_iter()
            .flatten()
            .find_map(|u| images::cached(u))
            .map(|p| p.to_string_lossy().into_owned());
        TrackMeta {
            title: t.title.clone(),
            artist: t.artists.join(", "),
            album: t.album.clone(),
            artwork: t.cover.clone().or_else(|| t.thumb.clone()),
            artwork_file,
        }
    }
}

/// The app's cover cache: one file per image URL, written by the app's image handler and read
/// by it and by the system's media controls (which can't reach the network offline).
pub mod images {
    use std::hash::{Hash, Hasher};
    use std::path::PathBuf;
    use std::sync::OnceLock;

    static DIR: OnceLock<PathBuf> = OnceLock::new();

    pub fn set_dir(dir: PathBuf) {
        let _ = DIR.set(dir);
    }

    /// Where `url`'s copy lives (or would), in `dir`.
    pub fn path_in(dir: &std::path::Path, url: &str) -> PathBuf {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut h);
        dir.join(format!("{:016x}", h.finish()))
    }

    /// `url`'s copy, if it has been cached.
    pub fn cached(url: &str) -> Option<PathBuf> {
        let p = path_in(DIR.get()?, url);
        p.is_file().then_some(p)
    }

    /// The big version of a cover, for Now Playing and the album page, which show it at up to
    /// ~950 device px: album art at 1024 px, and a video's 1280x720 thumbnail instead of the
    /// 400x225 one the API lists (most songs without an album are videos).
    pub fn large(url: &str) -> String {
        if let Some(rest) = url.strip_prefix("https://i.ytimg.com/vi/") {
            if let Some(id) = rest.split('/').next().filter(|id| !id.is_empty()) {
                return format!("https://i.ytimg.com/vi/{id}/hq720.jpg");
            }
        }
        super::sized(url, 1024)
    }

    /// A fifth of videos (old 4:3 uploads) have no 1280x720 thumbnail; their full 480x360 one
    /// is still bigger than what the API lists.
    fn fallback(url: &str) -> Option<String> {
        url.strip_suffix("/hq720.jpg").map(|base| format!("{base}/hqdefault.jpg"))
    }

    /// Download `url`, or its fallback when it doesn't exist.
    pub async fn download(http: &reqwest::Client, url: &str) -> Result<Vec<u8>, reqwest::Error> {
        let get = |u: String| async move {
            let r = http.get(&u).send().await?.error_for_status()?;
            Ok::<_, reqwest::Error>(r.bytes().await?.to_vec())
        };
        match get(url.to_string()).await {
            Err(e) if e.status().is_some() => match fallback(url) {
                Some(f) => get(f).await,
                None => Err(e),
            },
            r => r,
        }
    }

    /// Fetch `url` into the cache unless it's there (a downloaded song's covers, so they show
    /// offline even if the app never displayed them).
    pub async fn fetch(url: &str) {
        let Some(dir) = DIR.get() else { return };
        let p = path_in(dir, url);
        if p.is_file() || !url.starts_with("https://") {
            return;
        }
        static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
        let http = HTTP.get_or_init(reqwest::Client::new);
        if let Ok(bytes) = download(http, url).await {
            let _ = tokio::fs::create_dir_all(dir).await;
            let _ = tokio::fs::write(&p, &bytes).await;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn large_covers() {
            assert_eq!(
                large("https://i.ytimg.com/vi/CP26mUeCaBQ/hqdefault.jpg?sqp=-oaymwEW&rs=AMzJ"),
                "https://i.ytimg.com/vi/CP26mUeCaBQ/hq720.jpg"
            );
            assert_eq!(
                large("https://lh3.googleusercontent.com/abc=w544-h544-l90-rj"),
                "https://lh3.googleusercontent.com/abc=w1024-h1024-l90-rj"
            );
            assert_eq!(
                fallback("https://i.ytimg.com/vi/x/hq720.jpg").as_deref(),
                Some("https://i.ytimg.com/vi/x/hqdefault.jpg")
            );
            assert_eq!(fallback("https://lh3.googleusercontent.com/abc=w1024-h1024"), None);
        }
    }
}

pub struct Source {
    ytm: Arc<YtMusic>,
    /// The signed-in client, when radio should come from the account.
    radio_ytm: std::sync::RwLock<Option<Arc<YtMusic>>>,
}

impl Source {
    pub fn new(storage_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&storage_dir).ok();
        let ytm = YtMusic::anonymous()
            .cache_player(storage_dir.join("player.json"))
            .cache_resolutions(storage_dir.join("resolutions.json"));
        Ok(Source { ytm: Arc::new(ytm), radio_ytm: Default::default() })
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

    /// Songs, plus YouTube Music's top result when that is an artist or an album. The two
    /// requests run side by side; a failed top result just leaves it out.
    pub async fn search(&self, query: &str) -> Result<SearchResults> {
        let (songs, top) = tokio::join!(self.search_tracks(query), self.top_result(query));
        Ok(SearchResults {
            songs: songs?,
            top: top.unwrap_or_else(|e| {
                log::debug!("top result for {query:?}: {e:#}");
                None
            }),
        })
    }

    async fn top_result(&self, query: &str) -> Result<Option<TopResult>> {
        use ytmusic::Client;
        let res = self
            .ytm
            .execute("search", Client::Music, serde_json::json!({ "query": query }))
            .await?;
        let Some(card) = find_key(&res, "musicCardShelfRenderer") else { return Ok(None) };
        let title_run = card.pointer("/title/runs/0");
        let title = title_run.and_then(|r| r["text"].as_str()).unwrap_or_default().to_string();
        let browse = title_run.and_then(|r| r.pointer("/navigationEndpoint/browseEndpoint"));
        let (Some(id), Some(page)) = (
            browse.and_then(|b| b["browseId"].as_str()),
            browse.and_then(|b| {
                b.pointer("/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
                    .and_then(|p| p.as_str())
            }),
        ) else {
            return Ok(None); // a song or video: the songs list covers it
        };
        // "Artist • 479M monthly audience" → "479M monthly audience".
        let subtitle: String = card
            .pointer("/subtitle/runs")
            .and_then(|r| r.as_array())
            .map(|runs| runs.iter().filter_map(|r| r["text"].as_str()).collect())
            .unwrap_or_default();
        let detail = subtitle.split_once(" • ").map(|(_, rest)| rest.replace(" • ", " · "));
        let thumb = card
            .get("thumbnail")
            .and_then(|t| find_key(t, "thumbnails"))
            .and_then(|t| t.as_array())
            .and_then(|a| a.iter().max_by_key(|t| t["width"].as_u64().unwrap_or(0)))
            .and_then(|t| t["url"].as_str())
            .map(|u| sized(u, 352));
        let id = id.to_string();
        Ok(match page {
            "MUSIC_PAGE_TYPE_ARTIST" => Some(TopResult::Artist { id, name: title, detail, thumb }),
            "MUSIC_PAGE_TYPE_ALBUM" => Some(TopResult::Album { id, title, detail, thumb }),
            _ => None,
        })
    }

    /// Radio from this client instead of the anonymous one; `None` goes back.
    pub fn set_radio_client(&self, ytm: Option<Arc<YtMusic>>) {
        *self.radio_ytm.write().unwrap() = ytm;
    }

    pub async fn radio(&self, video_id: &str) -> Result<Vec<Track>> {
        let ytm = self.radio_ytm.read().unwrap().clone().unwrap_or_else(|| self.ytm.clone());
        let res = ytm.track_radio(video_id).await?;
        Ok(res.into_iter().filter_map(Track::from_ytm).collect())
    }

    /// YouTube Music's home page: the first page and a few more, about a second in all. The
    /// account's when `ytm` is signed in (personal), the anonymous one's otherwise (charts, new
    /// releases and moods for the region).
    pub async fn home(&self, ytm: Option<Arc<YtMusic>>) -> Result<Vec<HomeShelf>> {
        use ytmusic::Client;
        let ytm = ytm.unwrap_or_else(|| self.ytm.clone());
        let mut page = ytm
            .execute("browse", Client::Music, serde_json::json!({ "browseId": "FEmusic_home" }))
            .await?;
        let mut shelves = Vec::new();
        for _ in 0..4 {
            shelves.extend(
                ytmusic::parse::find_renderers(&page, "musicCarouselShelfRenderer").into_iter().filter_map(home_shelf),
            );
            let token = ["sectionListRenderer", "sectionListContinuation"]
                .iter()
                .find_map(|k| ytmusic::parse::find_renderer(&page, k).and_then(ytmusic::parse::shelf_continuation));
            let Some(token) = token else { break };
            match ytm.execute("browse", Client::Music, serde_json::json!({ "continuation": token })).await {
                Ok(next) => page = next,
                Err(e) => {
                    log::debug!("home: next page: {e:#}");
                    break;
                }
            }
        }
        anyhow::ensure!(!shelves.is_empty(), "YouTube Music's home page came back empty");
        // Song shelves first: one click plays. Otherwise YouTube's order.
        shelves.sort_by_key(|s| s.songs.is_empty());
        Ok(shelves)
    }

    /// A YouTube Music playlist's songs, to play it from a Home card.
    pub async fn playlist_tracks(&self, id: &str) -> Result<Vec<Track>> {
        let d = self.ytm.playlist(id).await?;
        Ok(d.tracks.into_iter().filter_map(Track::from_ytm).collect())
    }

    /// The radios of up to four songs, fetched side by side. A failed one comes back empty.
    pub async fn radios(&self, ids: &[String]) -> Vec<Vec<Track>> {
        let one = |i: usize| async move {
            let Some(id) = ids.get(i) else { return Vec::new() };
            self.radio(id).await.unwrap_or_else(|e| {
                log::debug!("radio for {id}: {e:#}");
                Vec::new()
            })
        };
        let (a, b, c, d) = tokio::join!(one(0), one(1), one(2), one(3));
        [a, b, c, d].into_iter().take(ids.len()).collect()
    }

    pub async fn artist(&self, id: &str) -> Result<ArtistPage> {
        let a = self.ytm.artist(id).await?;
        Ok(ArtistPage {
            id: a.browse_id,
            name: a.name,
            subscribers: a.subscribers,
            banner: best_thumb(&a.thumbnails).map(|u| sized_wh(u, 1440, 600)),
            top: a.top_tracks.into_iter().filter_map(Track::from_ytm).collect(),
            albums: a.albums.into_iter().map(AlbumCard::from_ytm).collect(),
            singles: a.singles.into_iter().map(AlbumCard::from_ytm).collect(),
        })
    }

    pub async fn album(&self, id: &str) -> Result<AlbumPage> {
        let d = self.ytm.album(id).await?;
        let cover = best_thumb(&d.album.thumbnails).map(|u| sized(u, 544));
        Ok(AlbumPage {
            cover,
            card: AlbumCard::from_ytm(d.album),
            tracks: d.tracks.into_iter().filter_map(Track::from_ytm).collect(),
        })
    }

    /// The artist's own Mix: their songs with similar artists between, as YouTube Music's
    /// Radio button on the artist page plays it.
    pub async fn artist_radio(&self, id: &str) -> Result<Vec<Track>> {
        use ytmusic::Client;
        let page = self
            .ytm
            .execute("browse", Client::Music, serde_json::json!({ "browseId": id }))
            .await?;
        let endpoint = find_key(&page, "startRadioButton")
            .and_then(|b| find_key(b, "watchEndpoint"))
            .ok_or_else(|| anyhow::anyhow!("this artist has no radio"))?;
        let mut body = serde_json::json!({
            "playlistId": endpoint["playlistId"],
            "enablePersistentPlaylistPanel": true,
            "isAudioOnly": true,
        });
        for k in ["videoId", "params"] {
            if let Some(v) = endpoint.get(k) {
                body[k] = v.clone();
            }
        }
        let next = self.ytm.execute("next", Client::Music, body).await?;
        let mut tracks = Vec::new();
        for panel in ytmusic::parse::find_renderers(&next, "playlistPanelRenderer") {
            for item in panel.get("contents").and_then(|c| c.as_array()).into_iter().flatten() {
                if let Some(t) = ytmusic::parse::panel_track(item).and_then(Track::from_ytm) {
                    tracks.push(t);
                }
            }
        }
        anyhow::ensure!(!tracks.is_empty(), "this artist's radio is empty");
        Ok(tracks)
    }

    /// YouTube Music's own (plain, unsynced) lyrics: the watch page's Lyrics tab.
    pub async fn yt_lyrics(&self, video_id: &str) -> Result<Option<String>> {
        use ytmusic::Client;
        let next = self
            .ytm
            .execute("next", Client::Music, serde_json::json!({ "videoId": video_id }))
            .await?;
        let Some(browse_id) = find_str(&next, "browseId", |s| s.starts_with("MPLYt")) else {
            return Ok(None);
        };
        let page = self
            .ytm
            .execute("browse", Client::Music, serde_json::json!({ "browseId": browse_id }))
            .await?;
        let text = find_key(&page, "musicDescriptionShelfRenderer")
            .and_then(|shelf| shelf.pointer("/description/runs/0/text"))
            .and_then(|t| t.as_str())
            .map(str::to_string);
        Ok(text)
    }

    /// The song's best stream, already arriving, to save to disk (offline downloads). Reads it a
    /// range at a time: the stream host serves a plain GET at a trickle.
    pub async fn open_audio(&self, video_id: &str) -> Result<(ytmusic::AudioFormat, ytmusic::AudioStream)> {
        self.ytm.open_audio(video_id).await
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
            record_to: None,
            local: false,
            loudness_db: fmt.loudness_db,
            gain_db: None,
            meta: None,
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

/// Depth-first search for the first string under `key` that satisfies `ok`.
fn find_str(v: &serde_json::Value, key: &str, ok: impl Fn(&str) -> bool + Copy) -> Option<String> {
    match v {
        serde_json::Value::Object(m) => {
            if let Some(s) = m.get(key).and_then(|s| s.as_str()).filter(|s| ok(s)) {
                return Some(s.to_string());
            }
            m.values().find_map(|c| find_str(c, key, ok))
        }
        serde_json::Value::Array(a) => a.iter().find_map(|c| find_str(c, key, ok)),
        _ => None,
    }
}

/// Depth-first search for the first object stored under `key`.
fn find_key<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    match v {
        serde_json::Value::Object(m) => m.get(key).or_else(|| m.values().find_map(|c| find_key(c, key))),
        serde_json::Value::Array(a) => a.iter().find_map(|c| find_key(c, key)),
        _ => None,
    }
}
