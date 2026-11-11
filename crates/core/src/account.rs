//! The signed-in YouTube Music account, used only for likes and playlists. Search and playback
//! stay on the anonymous client, so a lapsed session never stops music.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use ytmusic::YtMusic;

#[derive(Clone, Debug, Serialize)]
pub struct Profile {
    pub name: String,
    pub email: Option<String>,
    pub photo: Option<String>,
}

pub struct Account {
    dir: PathBuf,
    signed_in: RwLock<Option<Signed>>,
    sts: std::sync::Mutex<Option<u32>>,
}

#[derive(Clone)]
struct Signed {
    ytm: Arc<YtMusic>,
    profile: Option<Profile>,
}

/// Signed-in requests are authorized from SAPISID.
pub fn has_session(cookie_header: &str) -> bool {
    cookie_header
        .split(';')
        .filter_map(|p| p.trim().split_once('='))
        .any(|(name, _)| name == "SAPISID" || name == "__Secure-3PAPISID")
}

impl Account {
    pub fn open(dir: PathBuf) -> Self {
        let signed_in = std::fs::read_to_string(dir.join("cookies.txt"))
            .ok()
            .filter(|seed| has_session(seed))
            .map(|seed| Signed { ytm: Arc::new(client(&dir, &seed)), profile: None });
        Account { dir, signed_in: RwLock::new(signed_in), sts: std::sync::Mutex::new(None) }
    }

    pub fn is_signed_in(&self) -> bool {
        self.signed_in.read().unwrap().is_some()
    }

    pub fn client(&self) -> Option<Arc<YtMusic>> {
        self.signed_in.read().unwrap().as_ref().map(|s| s.ytm.clone())
    }

    /// `cookie_header` is from music.youtube.com; saved only if the account answers.
    pub async fn sign_in(&self, cookie_header: &str) -> Result<Profile> {
        if !has_session(cookie_header) {
            bail!("no Google session in the cookies");
        }
        // A store left from an earlier account would replace the new cookies.
        let _ = std::fs::remove_file(self.dir.join("cookies.json"));
        let ytm = Arc::new(client(&self.dir, cookie_header));
        let profile = to_profile(ytm.profile().await.context("YouTube Music didn't accept the sign-in")?);
        std::fs::create_dir_all(&self.dir).ok();
        write_private(&self.dir.join("cookies.txt"), cookie_header.as_bytes())?;
        // Google answers a rotation with fresh cookies, which writes cookies.json.
        if let Err(e) = ytm.rotate_cookies().await {
            log::debug!("account: rotation right after sign-in: {e:#}");
        }
        *self.signed_in.write().unwrap() = Some(Signed { ytm, profile: Some(profile.clone()) });
        log::info!("account: signed in");
        Ok(profile)
    }

    pub fn sign_out(&self) {
        *self.signed_in.write().unwrap() = None;
        for f in ["cookies.txt", "cookies.json"] {
            let _ = std::fs::remove_file(self.dir.join(f));
        }
        log::info!("account: signed out");
    }

    pub async fn profile(&self) -> Result<Option<Profile>> {
        let Some(signed) = self.signed_in.read().unwrap().clone() else { return Ok(None) };
        if let Some(p) = signed.profile {
            return Ok(Some(p));
        }
        let p = to_profile(signed.ytm.profile().await?);
        if let Some(s) = self.signed_in.write().unwrap().as_mut()
            && Arc::ptr_eq(&s.ytm, &signed.ytm)
        {
            s.profile = Some(p.clone());
        }
        Ok(Some(p))
    }

    /// Add a play to the account's history, as YouTube Music's player does: a signed-in player
    /// response names stats URLs, and fetching them with the account's cookies counts the play.
    pub async fn report_play(&self, video_id: &str, secs: f64) -> Result<()> {
        let Some(ytm) = self.client() else { return Ok(()) };
        // Without the player script's timestamp the signed-in player often answers UNPLAYABLE.
        let payload = serde_json::json!({
            "videoId": video_id,
            "racyCheckOk": true,
            "contentCheckOk": true,
            "playbackContext": { "contentPlaybackContext": { "signatureTimestamp": self.sts().await? } },
        });
        let player = ytm.execute("player", ytmusic::Client::Music, payload).await?;
        let tracking = |key: &str| {
            player.pointer(&format!("/playbackTracking/{key}/baseUrl")).and_then(|v| v.as_str()).map(str::to_string)
        };
        let playback = tracking("videostatsPlaybackUrl").context("the player response has no playback tracking URL")?;
        let cpn = cpn();
        let http = reqwest::Client::new();
        for (url, watched) in [(Some(playback), false), (tracking("videostatsWatchtimeUrl"), true)] {
            let Some(url) = url else { continue };
            let mut url = reqwest::Url::parse(&url)?;
            url.query_pairs_mut().append_pair("ver", "2").append_pair("cpn", &cpn);
            if watched {
                let secs = format!("{:.0}", secs.max(1.0));
                url.query_pairs_mut().append_pair("st", "0").append_pair("et", &secs);
            }
            let cookies = self.cookie_header(&url).context("no saved cookies")?;
            let status = http
                .get(url)
                .header("Cookie", cookies)
                .header("User-Agent", ytmusic::Client::Music.user_agent())
                .header("Origin", "https://music.youtube.com")
                .header("Referer", "https://music.youtube.com/")
                .send()
                .await?
                .status();
            if !status.is_success() {
                bail!("YouTube answered {status}");
            }
        }
        Ok(())
    }

    /// YouTube Music's player script timestamp, read once per run.
    async fn sts(&self) -> Result<u32> {
        if let Some(sts) = *self.sts.lock().unwrap() {
            return Ok(sts);
        }
        let http = reqwest::Client::new();
        let ua = ytmusic::Client::Music.user_agent();
        let page = http.get("https://music.youtube.com/").header("User-Agent", ua).send().await?.text().await?;
        let js = page
            .split("\"jsUrl\":\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .context("no player script on the YouTube Music page")?;
        let code = http.get(format!("https://music.youtube.com{js}")).send().await?.text().await?;
        let sts: u32 = code
            .split("signatureTimestamp:")
            .nth(1)
            .map(|s| s.chars().take_while(char::is_ascii_digit).collect::<String>())
            .context("no signature timestamp in the player script")?
            .parse()?;
        *self.sts.lock().unwrap() = Some(sts);
        Ok(sts)
    }

    /// The session's cookies now, rotated ones included.
    fn cookie_header(&self, url: &reqwest::Url) -> Option<String> {
        let file = std::fs::File::open(self.dir.join("cookies.json")).ok()?;
        let store = cookie_store::serde::json::load(std::io::BufReader::new(file)).ok()?;
        let pairs: Vec<String> = store.get_request_values(url).map(|(n, v)| format!("{n}={v}")).collect();
        (!pairs.is_empty()).then(|| pairs.join("; "))
    }

    /// Google stops accepting a session nobody rotates, as an open tab would.
    pub async fn rotate(&self) {
        let Some(ytm) = self.client() else { return };
        match ytm.rotate_cookies().await {
            Ok(()) => log::debug!("account: cookies rotated"),
            Err(e) => log::warn!("account: cookie rotation: {e:#}"),
        }
    }
}

/// A client playback nonce: 16 characters from YouTube's URL-safe alphabet.
fn cpn() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    (0..16).map(|_| ALPHABET[rand::random_range(0..64)] as char).collect()
}

fn client(dir: &Path, seed: &str) -> YtMusic {
    YtMusic::with_cookies(seed).persist_cookies(dir.join("cookies.json"))
}

fn to_profile(p: ytmusic::Profile) -> Profile {
    let photo = p.thumbnails.iter().max_by_key(|t| t.width).map(|t| t.url.clone());
    Profile { name: p.name, email: p.email, photo }
}

fn write_private(path: &Path, body: &[u8]) -> Result<()> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut f = options.open(path).with_context(|| format!("cannot write {}", path.display()))?;
    f.write_all(body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_needs_sapisid() {
        assert!(has_session("SID=a; SAPISID=b/c; HSID=d"));
        assert!(has_session("__Secure-3PAPISID=x"));
        assert!(!has_session("VISITOR_INFO1_LIVE=abc; YSC=def"));
        assert!(!has_session(""));
    }

    #[test]
    fn signed_out_without_a_seed() {
        let dir = std::env::temp_dir().join(format!("hum-account-test-{}", std::process::id()));
        let a = Account::open(dir.clone());
        assert!(!a.is_signed_in());
        let _ = std::fs::remove_dir_all(dir);
    }
}
