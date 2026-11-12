//! Thin Tauri shell over hum-core: invoke commands in, `hum://update` events out.

use hum_core::color::{self, Palette};
use hum_core::library::{HistoryEntry, Playlist, PlaylistSummary, TrackNote};
use hum_core::account::Profile;
use hum_core::sync::Sync;
use hum_core::{Account, Engine, Library, Quality, Source, Track};
#[cfg(target_os = "android")]
mod android;
#[cfg(not(target_os = "android"))]
mod media;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::http::{Response, StatusCode};
use tauri::{Emitter, Manager, State};

struct App {
    engine: Engine,
    http: reqwest::Client,
    account: Arc<Account>,
    sync: Option<Sync>,
    account_radio: std::sync::atomic::AtomicBool,
}

impl App {
    /// Radio comes from the account when that's on and someone is signed in.
    fn apply_radio(&self) {
        let on = self.account_radio.load(std::sync::atomic::Ordering::Relaxed);
        self.engine.source().set_radio_client(self.account.client().filter(|_| on));
    }
}

struct SyncEvents(tauri::AppHandle, Engine);

impl hum_core::sync::Events for SyncEvents {
    fn changed(&self) {
        let _ = self.0.emit("hum://library", ());
        // The account may have added or dropped songs of a playlist kept for offline.
        self.1.sync_downloads();
    }
    fn failed(&self, what: &str, error: &anyhow::Error) {
        let _ = self.0.emit("hum://sync-error", format!("Failed {what}: {error:#}"));
    }
}

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
async fn search(app: State<'_, App>, query: String) -> Res<hum_core::source::SearchResults> {
    app.engine.source().search(&query).await.map_err(err)
}

/// A song's radio, without playing it (Home's quick picks).
#[tauri::command]
async fn radio(app: State<'_, App>, id: String) -> Res<Vec<Track>> {
    app.engine.source().radio(&id).await.map_err(err)
}

/// Audio downloaded this month and in all, plus whether Auto has stepped down.
#[tauri::command]
async fn data_used(app: State<'_, App>) -> Res<serde_json::Value> {
    let used = lib(&app)?.run(|l| l.data_used()).await.map_err(err)?;
    Ok(serde_json::json!({ "month": used.month, "total": used.total, "auto_low": app.engine.auto_low() }))
}

#[tauri::command]
async fn artist(app: State<'_, App>, id: String) -> Res<hum_core::source::ArtistPage> {
    app.engine.source().artist(&id).await.map_err(err)
}

#[tauri::command]
async fn album(app: State<'_, App>, id: String) -> Res<hum_core::source::AlbumPage> {
    app.engine.source().album(&id).await.map_err(err)
}

/// Play the artist's Mix. Autoplay keeps it going from the last song like any other queue.
#[tauri::command]
async fn play_artist_radio(app: State<'_, App>, id: String) -> Res<()> {
    let tracks = app.engine.source().artist_radio(&id).await.map_err(err)?;
    app.engine.play(tracks, 0).await.map_err(err)
}

#[tauri::command]
async fn play(app: State<'_, App>, tracks: Vec<Track>, index: usize) -> Res<()> {
    app.engine.play(tracks, index).await.map_err(err)
}

/// Play `track` and keep going with its radio.
#[tauri::command]
async fn play_radio(app: State<'_, App>, track: Track) -> Res<()> {
    app.engine.play(vec![track], 0).await.map_err(err)
}

/// Keep the current song playing and follow it with its radio.
#[tauri::command]
async fn radio_from_current(app: State<'_, App>) -> Res<()> {
    app.engine.radio_from_current().await;
    Ok(())
}

#[tauri::command]
async fn next(app: State<'_, App>) -> Res<()> {
    app.engine.next().await.map_err(err)
}

#[tauri::command]
async fn previous(app: State<'_, App>) -> Res<()> {
    app.engine.previous().await.map_err(err)
}

#[tauri::command]
async fn jump(app: State<'_, App>, index: usize) -> Res<()> {
    app.engine.jump(index).await.map_err(err)
}

fn lib(app: &App) -> Res<Library> {
    app.engine.library().cloned().ok_or_else(|| "library unavailable".to_string())
}

/// A library change the account should hear about, run off the async runtime.
async fn synced<T: Send + 'static>(
    app: &App,
    f: impl FnOnce(&Sync) -> anyhow::Result<T> + Send + 'static,
) -> Res<T> {
    let sync = app.sync.clone().ok_or_else(|| "library unavailable".to_string())?;
    tokio::task::spawn_blocking(move || f(&sync)).await.map_err(err)?.map_err(err)
}

#[tauri::command]
fn sync_likes(app: State<'_, App>) {
    if let Some(s) = &app.sync {
        s.pull_likes();
    }
}

#[tauri::command]
fn sync_playlist(app: State<'_, App>, id: i64) {
    if let Some(s) = &app.sync {
        s.refresh_playlist(id);
    }
}

#[tauri::command]
async fn hide_playlist(app: State<'_, App>, id: i64) -> Res<()> {
    synced(&app, move |s| s.hide_playlist(id)).await
}

#[tauri::command]
async fn account_playlists(app: State<'_, App>) -> Res<Vec<hum_core::sync::AccountPlaylist>> {
    match &app.sync {
        Some(s) => s.account_playlists().await.map_err(err),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
async fn set_playlist_shown(app: State<'_, App>, id: String, name: String, shown: bool) -> Res<()> {
    synced(&app, move |s| s.set_playlist_shown(&id, &name, shown)).await
}

#[tauri::command]
async fn set_liked(app: State<'_, App>, track: Track, liked: bool) -> Res<()> {
    synced(&app, move |s| s.set_liked(&track, liked)).await?;
    app.engine.sync_downloads();
    Ok(())
}

#[tauri::command]
async fn is_liked(app: State<'_, App>, id: String) -> Res<bool> {
    lib(&app)?.run(move |l| l.is_liked(&id)).await.map_err(err)
}

#[tauri::command]
async fn liked(app: State<'_, App>) -> Res<Vec<Track>> {
    lib(&app)?.run(|l| l.liked()).await.map_err(err)
}

#[tauri::command]
async fn history(app: State<'_, App>, limit: usize) -> Res<Vec<HistoryEntry>> {
    lib(&app)?.run(move |l| l.history(limit)).await.map_err(err)
}

#[tauri::command]
async fn recent(app: State<'_, App>, limit: usize) -> Res<Vec<Track>> {
    lib(&app)?.run(move |l| l.recent_tracks(limit)).await.map_err(err)
}

#[tauri::command]
async fn track_notes(
    app: State<'_, App>,
    ids: Vec<String>,
) -> Res<std::collections::HashMap<String, TrackNote>> {
    lib(&app)?.run(move |l| l.notes(&ids)).await.map_err(err)
}

#[derive(serde::Serialize)]
struct Forgotten {
    track: Track,
    last_played: i64,
}

#[tauri::command]
async fn forgotten(app: State<'_, App>, limit: usize) -> Res<Vec<Forgotten>> {
    let v = lib(&app)?.run(move |l| l.forgotten_favorites(30, limit)).await.map_err(err)?;
    Ok(v.into_iter().map(|(track, last_played)| Forgotten { track, last_played }).collect())
}

#[derive(serde::Serialize)]
struct QuickPicks {
    /// The songs the picks start from: your latest plays and favorites, or your likes when
    /// nothing has been played here yet.
    seeds: Vec<Track>,
    picks: Vec<Track>,
}

/// Home's quick picks: up to four seeds' radios fused (songs several seeds lead to first),
/// ranked by your taste, with a few of your own favorites mixed in so it isn't all new music.
#[tauri::command]
async fn quick_picks(app: State<'_, App>) -> Res<QuickPicks> {
    let l = lib(&app)?;
    let seeds = l
        .run(|l| {
            let mut seeds = l.recent_tracks(2)?;
            for t in l.top_tracks(4)?.into_iter().chain(l.liked()?) {
                if seeds.len() >= 4 {
                    break;
                }
                if !seeds.iter().any(|s| s.id == t.id) {
                    seeds.push(t);
                }
            }
            Ok(seeds)
        })
        .await
        .map_err(err)?;
    if seeds.is_empty() {
        return Ok(QuickPicks { seeds, picks: Vec::new() });
    }
    let ids: Vec<String> = seeds.iter().map(|t| t.id.clone()).collect();
    let radios = app.engine.source().radios(&ids).await;
    let picks = l
        .run(move |l| {
            let mut ranked = l.rank_radio(hum_core::library::fuse_radios(radios))?;
            ranked.retain(|t| !ids.contains(&t.id));
            let mut favs = l.top_tracks(12)?;
            favs.retain(|f| !ids.contains(&f.id) && !ranked.iter().any(|t| t.id == f.id));
            let mut out = Vec::new();
            let (mut r, mut f) = (ranked.into_iter(), favs.into_iter());
            // Three new for every one you know.
            while out.len() < 12 {
                let mut took = false;
                for _ in 0..3 {
                    if let Some(t) = r.next() {
                        out.push(t);
                        took = true;
                    }
                }
                if let Some(t) = f.next() {
                    out.push(t);
                    took = true;
                }
                if !took {
                    break;
                }
            }
            out.truncate(12);
            Ok(out)
        })
        .await
        .map_err(err)?;
    Ok(QuickPicks { seeds, picks })
}

/// Keep this track's artist out of radio and quick picks, and drop their upcoming songs.
#[tauri::command]
async fn block_artist(app: State<'_, App>, track: Track) -> Res<()> {
    let key = lib(&app)?.run(move |l| l.block_artist(&track)).await.map_err(err)?;
    app.engine.retain_upcoming(|t| hum_core::library::artist_key(t) != key);
    Ok(())
}

#[tauri::command]
async fn unblock_artist(app: State<'_, App>, key: String) -> Res<()> {
    lib(&app)?.run(move |l| l.unblock_artist(&key)).await.map_err(err)
}

#[derive(serde::Serialize)]
struct BlockedArtist {
    key: String,
    name: String,
}

#[tauri::command]
async fn blocked_artists(app: State<'_, App>) -> Res<Vec<BlockedArtist>> {
    let v = lib(&app)?.run(|l| l.blocked_artists()).await.map_err(err)?;
    Ok(v.into_iter().map(|(key, name)| BlockedArtist { key, name }).collect())
}

/// YouTube Music's home page, from the account when signed in.
#[tauri::command]
async fn home_feed(app: State<'_, App>) -> Res<Vec<hum_core::source::HomeShelf>> {
    app.engine.source().home(app.account.client()).await.map_err(err)
}

/// A YouTube Music playlist's songs (Home cards).
#[tauri::command]
async fn yt_playlist(app: State<'_, App>, id: String) -> Res<Vec<Track>> {
    app.engine.source().playlist_tracks(&id).await.map_err(err)
}

#[tauri::command]
async fn lyrics(app: State<'_, App>, track: Track) -> Res<Option<hum_core::lyrics::Lyrics>> {
    let l = lib(&app)?;
    let id = track.id.clone();
    if let Some(hit) = l.run(move |l| l.cached_lyrics(&id)).await.map_err(err)? {
        return Ok(hit);
    }
    let found = hum_core::lyrics::fetch(&app.http, app.engine.source(), &track)
        .await
        .map_err(err)?;
    let store = found.clone();
    let _ = l.run(move |l| l.store_lyrics(&track.id, store.as_ref())).await;
    Ok(found)
}

#[tauri::command]
async fn playlists_with(app: State<'_, App>, track_id: String) -> Res<Vec<(i64, usize)>> {
    lib(&app)?.run(move |l| l.playlists_with(&track_id)).await.map_err(err)
}

#[tauri::command]
async fn playlists(app: State<'_, App>) -> Res<Vec<PlaylistSummary>> {
    lib(&app)?.run(|l| l.playlists()).await.map_err(err)
}

#[tauri::command]
async fn playlist(app: State<'_, App>, id: i64) -> Res<Option<Playlist>> {
    lib(&app)?.run(move |l| l.playlist(id)).await.map_err(err)
}

/// Create a playlist, optionally with songs already in it. Returns its id.
#[tauri::command]
async fn create_playlist(app: State<'_, App>, name: String, tracks: Vec<Track>) -> Res<i64> {
    synced(&app, move |s| s.create_playlist(&name, &tracks)).await
}

#[tauri::command]
async fn rename_playlist(app: State<'_, App>, id: i64, name: String) -> Res<()> {
    synced(&app, move |s| s.rename_playlist(id, &name)).await
}

#[tauri::command]
async fn delete_playlist(app: State<'_, App>, id: i64) -> Res<()> {
    synced(&app, move |s| s.delete_playlist(id)).await?;
    app.engine.sync_downloads();
    Ok(())
}

/// Returns how many songs were new to the playlist.
#[tauri::command]
async fn add_to_playlist(app: State<'_, App>, id: i64, tracks: Vec<Track>) -> Res<usize> {
    let added = synced(&app, move |s| s.add_to_playlist(id, &tracks)).await?;
    app.engine.sync_downloads();
    Ok(added)
}

#[tauri::command]
async fn remove_from_playlist(app: State<'_, App>, id: i64, index: usize) -> Res<()> {
    synced(&app, move |s| s.remove_from_playlist(id, index)).await?;
    app.engine.sync_downloads();
    Ok(())
}

#[tauri::command]
async fn move_in_playlist(app: State<'_, App>, id: i64, from: usize, to: usize) -> Res<()> {
    synced(&app, move |s| s.move_in_playlist(id, from, to)).await
}

#[tauri::command]
fn toggle_pause(app: State<'_, App>) {
    app.engine.toggle_pause();
}

#[tauri::command]
fn set_paused(app: State<'_, App>, paused: bool) {
    app.engine.set_paused(paused);
}

#[tauri::command]
fn seek(app: State<'_, App>, secs: f64) {
    app.engine.seek(secs);
}

#[tauri::command]
fn set_volume(app: State<'_, App>, volume: f64) {
    app.engine.set_volume(volume);
}

#[tauri::command]
fn prefetch(app: State<'_, App>, id: String) {
    app.engine.prefetch(id);
}

#[tauri::command]
fn set_quality(app: State<'_, App>, quality: Quality) {
    app.engine.set_quality(quality);
}

#[tauri::command]
fn get_quality(app: State<'_, App>) -> Quality {
    app.engine.quality()
}

#[tauri::command]
fn set_account_radio(app: State<'_, App>, on: bool) {
    app.account_radio.store(on, std::sync::atomic::Ordering::Relaxed);
    app.apply_radio();
}

#[derive(serde::Serialize)]
struct SavedSongs {
    count: usize,
    bytes: u64,
}

#[tauri::command]
async fn saved_songs(app: State<'_, App>) -> Res<SavedSongs> {
    let cache = app.engine.song_cache();
    let (count, bytes) = tokio::task::spawn_blocking(move || cache.map(|c| c.usage()).unwrap_or((0, 0)))
        .await
        .map_err(err)?;
    Ok(SavedSongs { count, bytes })
}

#[tauri::command]
async fn clear_saved_songs(app: State<'_, App>) -> Res<()> {
    let cache = app.engine.song_cache();
    tokio::task::spawn_blocking(move || {
        if let Some(c) = cache {
            c.clear();
        }
    })
    .await
    .map_err(err)
}

/// How many bytes of played songs to keep; 0 keeps none (kept ones still play).
#[tauri::command]
fn set_song_cache_limit(app: State<'_, App>, bytes: u64) {
    app.engine.set_song_cache_limit(bytes);
}

/// Whether downloads may use mobile data (Android); otherwise they wait for Wi-Fi.
#[tauri::command]
fn set_metered_downloads(app: State<'_, App>, on: bool) {
    app.engine.set_metered_downloads(on);
}

/// Keep a playlist (`playlist:<id>`) or `liked` for offline, or stop.
#[tauri::command]
async fn set_download(app: State<'_, App>, target: String, on: bool) -> Res<()> {
    app.engine.set_download(target, on).await.map_err(err)
}

#[tauri::command]
async fn downloads(app: State<'_, App>) -> Res<hum_core::downloads::DownloadStatus> {
    app.engine.download_status().await.map_err(err)
}

#[tauri::command]
async fn remove_all_downloads(app: State<'_, App>) -> Res<()> {
    app.engine.remove_all_downloads().await.map_err(err)
}

#[tauri::command]
fn set_autoplay(app: State<'_, App>, on: bool) {
    app.engine.set_autoplay(on);
}

#[tauri::command]
fn set_shuffle(app: State<'_, App>, on: bool) {
    app.engine.set_shuffle(on);
}

#[tauri::command]
fn set_repeat(app: State<'_, App>, repeat: hum_core::Repeat) {
    app.engine.set_repeat(repeat);
}

/// Put back the last session's queue, paused (the UI saves it).
#[tauri::command]
fn restore_queue(app: State<'_, App>, tracks: Vec<Track>, index: usize, position: f64) {
    app.engine.restore(tracks, index, position);
}

#[tauri::command]
fn set_normalize(app: State<'_, App>, on: bool) {
    app.engine.set_normalize(on);
}

#[derive(serde::Serialize)]
struct QueueView {
    tracks: Vec<Track>,
    index: usize,
}

/// What the player should show on launch: the live queue, or the one saved last time.
#[tauri::command]
fn session(app: State<'_, App>) -> hum_core::engine::Session {
    app.engine.session()
}

#[tauri::command]
fn queue(app: State<'_, App>) -> QueueView {
    let (tracks, index) = app.engine.queue();
    QueueView { tracks, index }
}

#[tauri::command]
async fn cover_palette(app: State<'_, App>, url: String) -> Res<Palette> {
    color::palette_from_url(&app.http, &url).await.map_err(err)
}

/// Webview errors, forwarded into the app log so they show up next to the engine's.
const SIGN_IN_URL: &str = "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F";

#[tauri::command]
async fn account(app: State<'_, App>) -> Res<Option<Profile>> {
    app.account.profile().await.map_err(err)
}

/// Google's sign-in in an incognito window (no IPC: capabilities cover `main` only). When it
/// lands on YouTube Music signed in, its cookies become the account.
#[tauri::command]
async fn sign_in(handle: tauri::AppHandle) -> Res<()> {
    if let Some(w) = handle.get_webview_window("signin") {
        let _ = w.set_focus();
        return Ok(());
    }
    let url: tauri::Url = SIGN_IN_URL.parse().map_err(err)?;
    let busy = Arc::new(std::sync::atomic::AtomicBool::new(false));
    tauri::WebviewWindowBuilder::new(&handle, "signin", tauri::WebviewUrl::External(url))
        .title("Sign in to YouTube Music")
        .inner_size(480.0, 680.0)
        .incognito(true)
        .on_page_load(move |window, payload| {
            if payload.event() != tauri::webview::PageLoadEvent::Finished
                || payload.url().host_str() != Some("music.youtube.com")
                || busy.swap(true, std::sync::atomic::Ordering::SeqCst)
            {
                return;
            }
            let busy = busy.clone();
            // Reading cookies inside an event handler deadlocks on Windows.
            std::thread::spawn(move || {
                let site: tauri::Url = "https://music.youtube.com/".parse().unwrap();
                let header = match window.cookies_for_url(site) {
                    Ok(cookies) => cookies
                        .iter()
                        .map(|c| format!("{}={}", c.name(), c.value()))
                        .collect::<Vec<_>>()
                        .join("; "),
                    Err(e) => {
                        log::warn!("sign-in: cannot read cookies: {e}");
                        busy.store(false, std::sync::atomic::Ordering::SeqCst);
                        return;
                    }
                };
                if !hum_core::account::has_session(&header) {
                    busy.store(false, std::sync::atomic::Ordering::SeqCst);
                    return;
                }
                let handle = window.app_handle().clone();
                let account = handle.state::<App>().account.clone();
                tauri::async_runtime::block_on(async move {
                    match account.sign_in(&header).await {
                        Ok(profile) => {
                            let app = handle.state::<App>();
                            app.apply_radio();
                            if let Some(s) = &app.sync {
                                s.pull();
                            }
                            let _ = handle.emit("hum://account", Some(profile));
                            let _ = window.close();
                        }
                        Err(e) => {
                            log::warn!("sign-in: {e:#}");
                            let _ = handle.emit("hum://account-error", format!("{e:#}"));
                            busy.store(false, std::sync::atomic::Ordering::SeqCst);
                        }
                    }
                });
            });
        })
        .build()
        .map_err(err)?;
    Ok(())
}

#[tauri::command]
fn sign_out(app: State<'_, App>, handle: tauri::AppHandle) {
    app.account.sign_out();
    app.apply_radio();
    let _ = handle.emit("hum://account", None::<Profile>);
}

#[tauri::command]
fn ui_log(message: String) {
    log::warn!(target: "ui", "{message}");
}

/// Cover art goes through Rust: cached on disk (no data spent twice, works offline) and fetched
/// by reqwest, since WebKitGTK's own network stack failed to load googleusercontent images.
/// The page asks for `img://localhost/<percent-encoded original URL>`.
/// Where cover art is cached, shared with the system media controls (`images::cached`).
fn image_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_cache_dir()
        .map(|d| d.join("images"))
        .unwrap_or_else(|_| std::env::temp_dir().join("hum-images"))
}

async fn serve_image(http: reqwest::Client, dir: PathBuf, path: String) -> Response<Vec<u8>> {
    let not_found = || {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new())
            .unwrap()
    };
    let encoded = path.trim_start_matches('/');
    let Ok(url) = percent_decode(encoded) else { return not_found() };
    if !url.starts_with("https://") {
        return not_found();
    }
    let file = hum_core::source::images::path_in(&dir, &url);
    let ok = |bytes: Vec<u8>| {
        Response::builder()
            .header("Content-Type", sniff(&bytes))
            .header("Cache-Control", "max-age=31536000, immutable")
            .body(bytes)
            .unwrap()
    };
    if let Ok(bytes) = tokio::fs::read(&file).await {
        return ok(bytes);
    }
    match http.get(&url).send().await.and_then(|r| r.error_for_status()) {
        Ok(r) => match r.bytes().await {
            Ok(b) => {
                let bytes = b.to_vec();
                let _ = tokio::fs::create_dir_all(&dir).await;
                let _ = tokio::fs::write(&file, &bytes).await;
                ok(bytes)
            }
            Err(_) => not_found(),
        },
        Err(e) => {
            log::warn!("image {url}: {e}");
            not_found()
        }
    }
}

fn sniff(b: &[u8]) -> &'static str {
    match b {
        [0xFF, 0xD8, ..] => "image/jpeg",
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        _ => "application/octet-stream",
    }
}

fn percent_decode(s: &str) -> Result<String, ()> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).map_err(|_| ())?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| ())?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| ())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .register_asynchronous_uri_scheme_protocol("img", |ctx, request, responder| {
            let app = ctx.app_handle();
            let http = app.state::<App>().http.clone();
            let dir = image_dir(app);
            let path = request.uri().path().to_string();
            tauri::async_runtime::spawn(async move {
                responder.respond(serve_image(http, dir, path).await);
            });
        })
        .setup(|app| {
            let dir = app.path().app_cache_dir()?.join("ytmusic");
            let source = Source::new(dir)?;
            let db = app.path().app_data_dir()?.join("hum.db");
            let library = match Library::open(&db) {
                Ok(l) => Some(l),
                Err(e) => {
                    log::error!("library at {}: {e:#}", db.display());
                    None
                }
            };
            // Engine::start spawns tokio tasks, so it must run inside the runtime.
            let engine = tauri::async_runtime::block_on(async { Engine::start(source, library) })?;
            hum_core::source::images::set_dir(image_dir(app.handle()));
            // About 250 songs at High.
            engine.set_song_cache(hum_core::cache::SongCache::new(app.path().app_cache_dir()?.join("songs"), 1 << 30));
            // In the data dir: the system may wipe the cache dir.
            engine.set_downloads(hum_core::downloads::Downloads::new(app.path().app_data_dir()?.join("downloads")));
            engine.set_session_file(app.path().app_data_dir()?.join("session.json"));
            let mut updates = engine.subscribe();
            let handle = app.handle().clone();
            #[cfg(target_os = "android")]
            {
                let (handle, engine) = (handle.clone(), engine.clone());
                android::on_shown(move || {
                    let u = hum_core::engine::Update::Position { secs: engine.position() };
                    let _ = handle.emit("hum://update", u);
                });
            }
            tauri::async_runtime::spawn(async move {
                loop {
                    match updates.recv().await {
                        Ok(u) => {
                            #[cfg(target_os = "android")]
                            if matches!(u, hum_core::engine::Update::Position { .. }) && !android::ui_visible() {
                                continue;
                            }
                            let _ = handle.emit("hum://update", u);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
            #[cfg(windows)]
            let hwnd = app
                .get_webview_window("main")
                .and_then(|w| w.hwnd().ok())
                .map(|h| h.0 as usize);
            #[cfg(not(any(windows, target_os = "android")))]
            let hwnd = None;
            #[cfg(not(target_os = "android"))]
            media::spawn(app.handle().clone(), engine.clone(), hwnd);
            let account = Arc::new(Account::open(app.path().app_data_dir()?.join("account")));
            let rotating = account.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    rotating.rotate().await;
                    tokio::time::sleep(std::time::Duration::from_secs(10 * 60)).await;
                }
            });
            let events = Arc::new(SyncEvents(app.handle().clone(), engine.clone()));
            let sync = engine.library().cloned().map(|lib| {
                tauri::async_runtime::block_on(async { Sync::start(lib, account.clone(), events) })
            });
            if let Some(s) = &sync {
                s.pull();
            }
            app.manage(App {
                engine,
                http: reqwest::Client::new(),
                account,
                sync,
                account_radio: Default::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
            radio,
            artist,
            album,
            data_used,
            play_artist_radio,
            play,
            play_radio,
            radio_from_current,
            next,
            previous,
            jump,
            toggle_pause,
            set_paused,
            seek,
            set_volume,
            prefetch,
            set_quality,
            get_quality,
            set_autoplay,
            set_repeat,
            set_shuffle,
            playlists_with,
            set_normalize,
            restore_queue,
            session,
            saved_songs,
            clear_saved_songs,
            set_song_cache_limit,
            set_download,
            set_metered_downloads,
            downloads,
            remove_all_downloads,
            set_account_radio,
            queue,
            cover_palette,
            set_liked,
            is_liked,
            liked,
            history,
            recent,
            track_notes,
            forgotten,
            quick_picks,
            home_feed,
            block_artist,
            unblock_artist,
            blocked_artists,
            yt_playlist,
            lyrics,
            playlists,
            playlist,
            create_playlist,
            rename_playlist,
            delete_playlist,
            add_to_playlist,
            remove_from_playlist,
            move_in_playlist,
            account,
            sync_likes,
            hide_playlist,
            account_playlists,
            set_playlist_shown,
            sync_playlist,
            sign_in,
            sign_out,
            ui_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running hum");
}
