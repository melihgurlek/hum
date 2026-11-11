//! Phase 1 spike: can we get a playable YouTube Music audio stream fast, without a login?
//!
//! Two backends, built separately because both bundle QuickJS and cannot link together:
//!   cargo run --release                          # ytmusic-rs (Sonora's InnerTube client)
//!   cargo run --release --no-default-features --features rp   # rustypipe
//!
//! Per track it measures search latency, time to a resolved stream URL, time to the first
//! audio bytes (the network part of click-to-sound), the data-saver itag, and radio size.
//! The first track is the cold one (player JS / solver not cached yet).

use std::time::{Duration, Instant};

use anyhow::Result;

const QUERIES: &[&str] = &[
    "duman bu akşam",
    "tame impala the less i know the better",
    "massive attack teardrop",
    "mor ve ötesi bir derdim var",
    "arctic monkeys do i wanna know",
    "sezen aksu gülümse",
    "daft punk get lucky",
    "radiohead weird fishes",
];

/// Data-saver ceiling in bits per second (opus itag 249/250 sit around 50-70 kbps).
#[allow(dead_code)]
const SAVER_CEILING: u32 = 80_000;

fn ms(d: Duration) -> String {
    format!("{:>5} ms", d.as_millis())
}

struct Row {
    title: String,
    resolved: Option<Duration>,
    first: Option<Duration>,
    best: String,
    saver: String,
}

fn report(search: &[Duration], rows: &[Row], radio: &[(Duration, String, usize)]) {
    println!("\n{:<28} {:>9} {:>9} {:>18} {:>12}", "track", "resolve", "1st-bytes", "best", "saver");
    for r in rows {
        let t: String = r.title.chars().take(26).collect();
        let f = |d: Option<Duration>| d.map(ms).unwrap_or_else(|| "FAIL".into());
        println!("{:<28} {:>9} {:>9} {:>18} {:>12}", t, f(r.resolved), f(r.first), r.best, r.saver);
    }
    for (d, t, n) in radio {
        println!("radio {}  {:<30} {} tracks", ms(*d), t, n);
    }
    let stats = |name: &str, mut v: Vec<Duration>| {
        if v.is_empty() {
            return println!("{name}: no successes");
        }
        v.sort();
        println!("{name}: n={} min {} median {} max {}", v.len(), ms(v[0]), ms(v[v.len() / 2]), ms(v[v.len() - 1]));
    };
    println!();
    stats("search", search.to_vec());
    stats("resolve", rows.iter().filter_map(|r| r.resolved).collect());
    stats("click→first bytes", rows.iter().filter_map(|r| r.first).collect());
    stats("click→first bytes (warm, excl. 1st)", rows.iter().skip(1).filter_map(|r| r.first).collect());
    let ok = rows.iter().filter(|r| r.first.is_some()).count();
    println!("playable: {ok}/{}", rows.len());
}

fn queries() -> Vec<String> {
    let mut q: Vec<String> = QUERIES.iter().map(|s| s.to_string()).collect();
    q.extend(std::env::args().skip(1));
    q
}

#[cfg(feature = "ytm")]
async fn run() -> Result<()> {
    use std::sync::Arc;
    use ytmusic::YtMusic;
    println!("backend: ytmusic-rs (anonymous)");
    let ytm = Arc::new(YtMusic::anonymous());
    let (mut search, mut ids) = (vec![], vec![]);
    for q in queries() {
        let t = Instant::now();
        let tracks = ytm.search_songs(&q).await?;
        search.push(t.elapsed());
        if let Some(tr) = tracks.iter().find(|t| t.video_id.is_some()) {
            ids.push((tr.video_id.clone().unwrap(), tr.title.clone()));
        } else {
            println!("no result: {q}");
        }
    }
    let mut rows = vec![];
    for (id, title) in &ids {
        let t = Instant::now();
        let mut row = Row { title: title.clone(), resolved: None, first: None, best: "-".into(), saver: "-".into() };
        match ytm.open_audio(id).await {
            Ok((fmt, mut stream)) => {
                row.resolved = Some(t.elapsed());
                row.best = format!("{} {}k {}", fmt.itag, fmt.bitrate / 1000, fmt.codec);
                match stream.chunk().await {
                    Ok(Some(_)) => row.first = Some(t.elapsed()),
                    other => eprintln!("  {id}: first chunk {:?}", other.err()),
                }
            }
            Err(e) => eprintln!("  {id}: {e:#}"),
        }
        row.saver = match ytm.open_audio_within(id, Some(SAVER_CEILING)).await {
            Ok((fmt, _)) => format!("{} {}k", fmt.itag, fmt.bitrate / 1000),
            Err(_) => "FAIL".into(),
        };
        rows.push(row);
    }
    let mut radio = vec![];
    for (id, title) in ids.iter().take(3) {
        let t = Instant::now();
        let n = ytm.track_radio(id).await.map(|v| v.len()).unwrap_or(0);
        radio.push((t.elapsed(), title.clone(), n));
    }
    report(&search, &rows, &radio);
    Ok(())
}

#[cfg(feature = "rp")]
async fn run() -> Result<()> {
    use rustypipe::client::RustyPipe;
    println!("backend: rustypipe (anonymous)");
    let rp = RustyPipe::new();
    let http = reqwest::Client::new();
    let (mut search, mut ids) = (vec![], vec![]);
    for q in queries() {
        let t = Instant::now();
        let res = rp.query().music_search_tracks(&q).await?;
        search.push(t.elapsed());
        match res.items.items.first() {
            Some(tr) => ids.push((tr.id.clone(), tr.name.clone())),
            None => println!("no result: {q}"),
        }
    }
    let mut rows = vec![];
    for (id, title) in &ids {
        let t = Instant::now();
        let mut row = Row { title: title.clone(), resolved: None, first: None, best: "-".into(), saver: "-".into() };
        match rp.query().player(id.as_str()).await {
            Ok(player) => {
                row.resolved = Some(t.elapsed());
                let mut auds = player.audio_streams.clone();
                auds.sort_by_key(|s| s.bitrate);
                row.saver = auds.first().map(|s| format!("{} {}k", s.itag, s.bitrate / 1000)).unwrap_or("-".into());
                if let Some(best) = auds.last() {
                    row.best = format!("{} {}k {:?}", best.itag, best.bitrate / 1000, best.codec);
                    // first 64 KiB, as a player would fetch
                    match http.get(&best.url).header("Range", "bytes=0-65535").send().await {
                        Ok(r) if r.status().is_success() => {
                            if r.bytes().await.is_ok() {
                                row.first = Some(t.elapsed());
                            }
                        }
                        Ok(r) => eprintln!("  {id}: host said {}", r.status()),
                        Err(e) => eprintln!("  {id}: {e}"),
                    }
                }
            }
            Err(e) => eprintln!("  {id}: {e:#}"),
        }
        rows.push(row);
    }
    let mut radio = vec![];
    for (id, title) in ids.iter().take(3) {
        let t = Instant::now();
        let n = rp.query().music_radio_track(id.as_str()).await.map(|p| p.items.len()).unwrap_or(0);
        radio.push((t.elapsed(), title.clone(), n));
    }
    report(&search, &rows, &radio);
    Ok(())
}

/// Full-download check: does the whole file come through, not just the first chunk?
#[cfg(feature = "ytm")]
async fn full() -> Result<()> {
    use std::sync::Arc;
    let ytm = Arc::new(ytmusic::YtMusic::anonymous());
    for id in ["bOGTg4msji0", "12oIkuOrkMo", "u9Dg-g7t2l4"] {
        let t = Instant::now();
        match ytm.open_audio(id).await {
            Ok((fmt, mut stream)) => {
                let mut got = 0usize;
                let mut err = None;
                loop {
                    match stream.chunk().await {
                        Ok(Some(b)) => got += b.len(),
                        Ok(None) => break,
                        Err(e) => { err = Some(format!("{e:#}")); break; }
                    }
                }
                println!("ytm {id} itag {} {}k: {got}/{:?} bytes in {} {}", fmt.itag, fmt.bitrate/1000,
                    fmt.content_length, ms(t.elapsed()), err.unwrap_or_default());
            }
            Err(e) => println!("ytm {id}: open failed {e:#}"),
        }
    }
    Ok(())
}

#[cfg(feature = "rp")]
async fn full() -> Result<()> {
    use rustypipe::client::{ClientType, RustyPipe};
    let rp = RustyPipe::new();
    let http = reqwest::Client::new();
    for ct in [ClientType::Ios, ClientType::Tv, ClientType::Android, ClientType::Desktop, ClientType::DesktopMusic, ClientType::Mobile] {
        let id = "bOGTg4msji0";
        let t = Instant::now();
        let p = match rp.query().player_from_client(id, ct).await {
            Ok(p) => p,
            Err(e) => { println!("rp {ct:?}: player failed: {e}"); continue; }
        };
        let Some(s) = p.audio_streams.iter().max_by_key(|s| s.bitrate) else { println!("rp {ct:?}: no audio"); continue; };
        let (mut start, mut ok, mut bad) = (0u64, 0, 0);
        while start < s.size {
            let end = (start + 512 * 1024 - 1).min(s.size - 1);
            let st = http.get(&s.url).header("Range", format!("bytes={start}-{end}")).send().await?.status();
            if st.is_success() { ok += 1 } else { bad += 1; if bad > 2 { break; } }
            start = end + 1;
        }
        println!("rp {ct:?} itag {}: chunks ok {ok} failed {bad} in {}", s.itag, ms(t.elapsed()));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();
    if std::env::var("FULL").is_ok() { return full().await; }
    #[cfg(feature = "ytm")]
    if std::env::var("URL").is_ok() {
        let ytm = ytmusic::YtMusic::anonymous();
        let f = ytm.best_audio("bOGTg4msji0").await?;
        println!("{}\n{}\n{:?}", f.url, f.user_agent, f.content_length);
        return Ok(());
    }
    run().await
}
