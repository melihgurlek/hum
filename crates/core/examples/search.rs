//! cargo run -p hum-core --example search -- "query"
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let q = std::env::args().nth(1).unwrap_or_else(|| "falling faster".into());
    let src = hum_core::Source::new(std::env::temp_dir().join("hum-example"))?;
    for t in src.search_tracks(&q).await?.into_iter().take(3) {
        println!("{} | {:?}", t.title, t.thumb);
    }
    Ok(())
}
