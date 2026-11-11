//! cargo run -p hum-core --example lyrics -- "query"
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let q = std::env::args().nth(1).unwrap_or_else(|| "duman bu akşam".into());
    let src = hum_core::Source::new(std::env::temp_dir().join("hum-example"))?;
    let t = src.search_tracks(&q).await?.into_iter().next().expect("no result");
    let http = reqwest::Client::new();
    match hum_core::lyrics::fetch(&http, &src, &t).await? {
        Some(l) => println!("{} — {}: {} synced lines, plain: {}\n  {:?}", t.title, l.source,
            l.lines.as_ref().map_or(0, |v| v.len()), l.plain.is_some(),
            l.lines.as_ref().and_then(|v| v.get(1)).map(|x| (x.at, &x.text))),
        None => println!("{}: no lyrics", t.title),
    }
    let yt = src.yt_lyrics(&t.id).await?;
    println!("  YouTube Music fallback: {:?}", yt.map(|s| s.chars().take(60).collect::<String>()));
    Ok(())
}
