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
    /// The cover's most present real color: the base for backgrounds. Near-black, near-white
    /// and grey pixels don't count, because their hue is noise (a black cover with a faint JPEG
    /// tint would otherwise turn the background teal). A cover with almost no color gets grey.
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

/// Below this share of colored pixels, a cover counts as colorless.
const MIN_COLOR_SHARE: f32 = 0.05;

/// Whether a color's hue means something: not near-black, near-white or grey.
fn has_real_color(c: Rgb) -> bool {
    let max = c.r.max(c.g).max(c.b) as i32;
    let min = c.r.min(c.g).min(c.b) as i32;
    max - min >= 28 && max >= 40 && min <= 225
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
    let most_common = buckets
        .values()
        .max_by_key(|v| v.0)
        .map(|v| avg(*v))
        .unwrap_or(Rgb { r: 40, g: 40, b: 44 });
    let chromatic: Vec<&(u32, u32, u32, u32)> = buckets
        .values()
        .filter(|v| has_real_color(avg(**v)))
        .collect();
    let chromatic_share = chromatic.iter().map(|v| v.0).sum::<u32>() as f32 / total as f32;
    let dominant = if chromatic_share >= MIN_COLOR_SHARE {
        chromatic.iter().max_by_key(|v| v.0).map(|v| avg(**v)).unwrap_or(most_common)
    } else {
        // Essentially colorless: a grey at the cover's own brightness, with no hue to amplify.
        let l = ((most_common.r as u32 + most_common.g as u32 + most_common.b as u32) / 3) as u8;
        Rgb { r: l, g: l, b: l }
    };
    let vivid = buckets
        .values()
        .filter(|v| v.0 * 200 >= total) // at least 0.5% of the cover
        .filter(|v| has_real_color(avg(**v)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb as Px};

    fn encode(img: ImageBuffer<Px<u8>, Vec<u8>>) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    /// Like Eminem's Relapse: mostly black with a faint teal tint, a red subject.
    #[test]
    fn dark_cover_takes_its_subject_color_not_the_tinted_black() {
        let img = ImageBuffer::from_fn(48, 48, |x, y| {
            if (12..36).contains(&x) && (12..36).contains(&y) && (x + y) % 3 != 0 {
                Px([150, 30, 35])
            } else {
                Px([7, 12, 12])
            }
        });
        let p = palette_from_bytes(&encode(img)).unwrap();
        assert!(p.dominant.r > p.dominant.g * 2, "{:?}", p.dominant);
        assert!(p.vivid.r > p.vivid.g * 2, "{:?}", p.vivid);
    }

    #[test]
    fn colorless_cover_gets_grey() {
        let img = ImageBuffer::from_fn(48, 48, |x, _| if x < 24 { Px([10, 14, 14]) } else { Px([235, 235, 232]) });
        let p = palette_from_bytes(&encode(img)).unwrap();
        assert_eq!(p.dominant.r, p.dominant.g);
        assert_eq!(p.dominant.g, p.dominant.b);
    }
}
