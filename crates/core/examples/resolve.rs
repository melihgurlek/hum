//! Print a freshly resolved stream: cargo run -p hum-core --example resolve -- <video_id>
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let id = std::env::args().nth(1).unwrap_or_else(|| "bOGTg4msji0".into());
    let src = hum_core::Source::new(std::env::temp_dir().join("hum-example"))?;
    for q in [hum_core::Quality::DataSaver, hum_core::Quality::Normal] {
        let s = src.resolve(&id, q).await?;
        println!("{q:?}: itag {} {}k {} expires {}", s.itag, s.bitrate / 1000, s.codec, s.expires_at);
    }
    Ok(())
}
