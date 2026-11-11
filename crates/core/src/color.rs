//! A cover's colors, for tinting Now Playing and the accent. Computed from a small thumbnail.

use std::collections::HashMap;

use anyhow::Result;
use image::GenericImageView;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Palette {
    /// The cover's most present color: the base for backgrounds.
    pub dominant: Rgb,
    /// The most vivid well-represented color: the base for accents.
    pub vivid: Rgb,
}

fn hsv(c: Rgb) -> (f32, f32, f32) {
    let (r, g, b) = (c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let s = if max == 0.0 { 0.0 } else { (max - min) / max };
    (0.0, s, max)
}

/// Bucket pixels into a 4-bit-per-channel histogram and pick the most common bucket (dominant)
/// and the bucket with the best population × saturation × brightness score (vivid).
pub fn palette_from_bytes(bytes: &[u8]) -> Result<Palette> {
    let img = image::load_from_memory(bytes)?.thumbnail(48, 48);
    let mut buckets: HashMap<(u8, u8, u8), (u32, u32, u32, u32)> = HashMap::new();
    for (_, _, px) in img.pixels() {
        let [r, g, b, a] = px.0;
        if a < 128 {
            continue;
        }
        let e = buckets.entry((r >> 4, g >> 4, b >> 4)).or_default();
        e.0 += 1;
        e.1 += r as u32;
        e.2 += g as u32;
        e.3 += b as u32;
    }
    let avg = |(n, r, g, b): (u32, u32, u32, u32)| Rgb {
        r: (r / n) as u8,
        g: (g / n) as u8,
        b: (b / n) as u8,
    };
    let total: u32 = buckets.values().map(|v| v.0).sum::<u32>().max(1);
    let dominant = buckets
        .values()
        .max_by_key(|v| v.0)
        .map(|v| avg(*v))
        .unwrap_or(Rgb { r: 40, g: 40, b: 44 });
    let vivid = buckets
        .values()
        .filter(|v| v.0 * 200 >= total) // at least 0.5% of the cover
        .map(|v| {
            let c = avg(*v);
            let (_, s, val) = hsv(c);
            let score = (v.0 as f32).sqrt() * s * s * (0.35 + val);
            (score, c)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c)
        .unwrap_or(dominant);
    Ok(Palette { dominant, vivid })
}

pub async fn palette_from_url(http: &reqwest::Client, url: &str) -> Result<Palette> {
    let bytes = http.get(url).send().await?.error_for_status()?.bytes().await?;
    tokio::task::spawn_blocking(move || palette_from_bytes(&bytes)).await?
}
