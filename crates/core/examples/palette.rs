//! cargo run -p hum-core --example palette -- <image url>
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::args().nth(1).expect("url");
    let p = hum_core::color::palette_from_url(&reqwest::Client::new(), &url).await?;
    println!("dominant {:?}\nvivid    {:?}", p.dominant, p.vivid);
    Ok(())
}
