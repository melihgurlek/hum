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
        Account { dir, signed_in: RwLock::new(signed_in) }
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

    /// Google stops accepting a session nobody rotates, as an open tab would.
    pub async fn rotate(&self) {
        let Some(ytm) = self.client() else { return };
        match ytm.rotate_cookies().await {
            Ok(()) => log::debug!("account: cookies rotated"),
            Err(e) => log::warn!("account: cookie rotation: {e:#}"),
        }
    }
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
