//! Headless check: search, play the first hit for 12 s, skip to the radio's next track, quit.
//! cargo run -p hum-core --example play -- "duman bu akşam"

use std::time::Duration;

use hum_core::{Engine, Source, Update};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let q = std::env::args().nth(1).unwrap_or_else(|| "duman bu akşam".into());
    let dir = std::env::temp_dir().join("hum-example");
    let engine = Engine::start(Source::new(dir)?)?;
    let mut rx = engine.subscribe();
    tokio::spawn(async move {
        while let Ok(u) = rx.recv().await {
            match u {
                Update::Position { .. } => {}
                Update::Queue { tracks, index } => println!("queue: {} tracks, at {index}", tracks.len()),
                other => println!("{other:?}"),
            }
        }
    });
    tokio::time::sleep(Duration::from_secs(3)).await; // let warm-up finish
    let tracks = engine.source().search_tracks(&q).await?;
    println!("search: {} results", tracks.len());
    engine.play(tracks, 0).await?;
    tokio::time::sleep(Duration::from_secs(12)).await;
    println!("-> next");
    engine.next().await?;
    tokio::time::sleep(Duration::from_secs(8)).await;
    Ok(())
}
