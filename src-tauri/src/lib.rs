//! Thin Tauri shell over hum-core: invoke commands in, `hum://update` events out.

use hum_core::color::{self, Palette};
use hum_core::{Engine, Quality, Source, Track};
mod media;

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use tauri::http::{Response, StatusCode};
use tauri::{Emitter, Manager, State};

struct App {
    engine: Engine,
    http: reqwest::Client,
}

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
async fn search(app: State<'_, App>, query: String) -> Res<Vec<Track>> {
    app.engine.source().search_tracks(&query).await.map_err(err)
}

/// A song's radio, without playing it (Home's quick picks).
#[tauri::command]
async fn radio(app: State<'_, App>, id: String) -> Res<Vec<Track>> {
    app.engine.source().radio(&id).await.map_err(err)
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

#[tauri::command]
fn toggle_pause(app: State<'_, App>) {
    app.engine.toggle_pause();
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
fn set_autoplay(app: State<'_, App>, on: bool) {
    app.engine.set_autoplay(on);
}

#[derive(serde::Serialize)]
struct QueueView {
    tracks: Vec<Track>,
    index: usize,
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
#[tauri::command]
fn ui_log(message: String) {
    log::warn!(target: "ui", "{message}");
}

/// Cover art goes through Rust: cached on disk (no data spent twice, works offline) and fetched
/// by reqwest, since WebKitGTK's own network stack failed to load googleusercontent images.
/// The page asks for `img://localhost/<percent-encoded original URL>`.
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
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    let file = dir.join(format!("{:016x}", h.finish()));
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
            let dir = app
                .path()
                .app_cache_dir()
                .map(|d| d.join("images"))
                .unwrap_or_else(|_| std::env::temp_dir().join("hum-images"));
            let path = request.uri().path().to_string();
            tauri::async_runtime::spawn(async move {
                responder.respond(serve_image(http, dir, path).await);
            });
        })
        .setup(|app| {
            let dir = app.path().app_cache_dir()?.join("ytmusic");
            let source = Source::new(dir)?;
            // Engine::start spawns tokio tasks, so it must run inside the runtime.
            let engine = tauri::async_runtime::block_on(async { Engine::start(source) })?;
            let mut updates = engine.subscribe();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match updates.recv().await {
                        Ok(u) => {
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
            #[cfg(not(windows))]
            let hwnd = None;
            media::spawn(app.handle().clone(), engine.clone(), hwnd);
            app.manage(App {
                engine,
                http: reqwest::Client::new(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
            radio,
            play,
            play_radio,
            next,
            previous,
            jump,
            toggle_pause,
            seek,
            set_volume,
            prefetch,
            set_quality,
            get_quality,
            set_autoplay,
            queue,
            cover_palette,
            ui_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running hum");
}
