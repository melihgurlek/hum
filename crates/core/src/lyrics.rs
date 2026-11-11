//! Lyrics: time-synced from LRCLIB when it has them, otherwise plain text from LRCLIB or
//! YouTube Music.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::source::{Source, Track};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Line {
    /// Seconds from the start of the song.
    pub at: f64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Lyrics {
    /// Present when the lyrics are time-synced.
    pub lines: Option<Vec<Line>>,
    pub plain: Option<String>,
    /// Where they came from, for credit: "LRCLIB" or "YouTube Music".
    pub source: String,
}

/// Identifies hum to LRCLIB, as its API asks, without naming the user.
const USER_AGENT: &str = "hum/0.1 (open-source music player)";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibRecord {
    duration: Option<f64>,
    instrumental: Option<bool>,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

/// Parse LRC text: `[mm:ss.xx] words`, possibly several timestamps on one line.
pub fn parse_lrc(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            let tag = &stripped[..end];
            rest = stripped[end + 1..].trim_start();
            if let Some(t) = parse_stamp(tag) {
                stamps.push(t);
            }
        }
        for at in stamps {
            out.push(Line {
                at,
                text: rest.to_string(),
            });
        }
    }
    out.sort_by(|a, b| a.at.total_cmp(&b.at));
    out
}

fn parse_stamp(tag: &str) -> Option<f64> {
    let (m, s) = tag.split_once(':')?;
    let m: f64 = m.trim().parse().ok()?;
    let s: f64 = s.trim().parse().ok()?;
    Some(m * 60.0 + s)
}

fn from_record(r: LrclibRecord) -> Option<Lyrics> {
    if r.instrumental == Some(true) {
        return Some(Lyrics {
            lines: None,
            plain: Some("♪ Instrumental ♪".into()),
            source: "LRCLIB".into(),
        });
    }
    let lines = r
        .synced_lyrics
        .as_deref()
        .map(parse_lrc)
        .filter(|l| !l.is_empty());
    let plain = r.plain_lyrics.filter(|p| !p.trim().is_empty());
    (lines.is_some() || plain.is_some()).then(|| Lyrics {
        lines,
        plain,
        source: "LRCLIB".into(),
    })
}

async fn lrclib(http: &reqwest::Client, t: &Track) -> Result<Option<Lyrics>> {
    let artist = t.artists.first().cloned().unwrap_or_default();
    let duration = t.duration.unwrap_or(0) as f64;

    // Exact match first: title + artist + duration (±2 s on their side).
    let mut q: Vec<(&str, String)> = vec![("track_name", t.title.clone()), ("artist_name", artist.clone())];
    if let Some(a) = &t.album {
        q.push(("album_name", a.clone()));
    }
    if duration > 0.0 {
        q.push(("duration", format!("{}", duration as u64)));
    }
    let res = http
        .get("https://lrclib.net/api/get")
        .header("User-Agent", USER_AGENT)
        .query(&q)
        .send()
        .await?;
    if res.status().is_success() {
        if let Some(l) = from_record(res.json().await?) {
            return Ok(Some(l));
        }
    }

    // Then a search, taking the closest duration and preferring synced lyrics.
    let res = http
        .get("https://lrclib.net/api/search")
        .header("User-Agent", USER_AGENT)
        .query(&[("track_name", t.title.as_str()), ("artist_name", artist.as_str())])
        .send()
        .await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let mut found: Vec<LrclibRecord> = res.json().await?;
    found.retain(|r| duration == 0.0 || r.duration.is_none_or(|d| (d - duration).abs() <= 5.0));
    found.sort_by_key(|r| {
        (
            r.synced_lyrics.is_none(),
            r.duration.map(|d| ((d - duration).abs() * 10.0) as i64).unwrap_or(999),
        )
    });
    Ok(found.into_iter().find_map(from_record))
}

/// Find lyrics for `track`: LRCLIB (synced if possible), then YouTube Music's plain lyrics.
pub async fn fetch(http: &reqwest::Client, source: &Source, track: &Track) -> Result<Option<Lyrics>> {
    match lrclib(http, track).await {
        Ok(Some(l)) => return Ok(Some(l)),
        Ok(None) => {}
        Err(e) => log::warn!("lrclib {}: {e:#}", track.id),
    }
    Ok(source.yt_lyrics(&track.id).await?.map(|plain| Lyrics {
        lines: None,
        plain: Some(plain),
        source: "YouTube Music".into(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_with_multiple_stamps() {
        let l = parse_lrc("[00:25.81] Kendime geldim\n[01:02.00][02:10.50]Nakarat\n[ar:Duman]\nno stamp");
        assert_eq!(l.len(), 3);
        assert_eq!(l[0], Line { at: 25.81, text: "Kendime geldim".into() });
        assert_eq!(l[1].at, 62.0);
        assert_eq!(l[2].text, "Nakarat");
        assert!((l[2].at - 130.5).abs() < 1e-9);
    }
}
