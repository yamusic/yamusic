use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use std::sync::Arc;
use yandex_music::{DEFAULT_CLIENT_ID, YandexMusicClient};

const KEYRING_SERVICE: &str = "yamusic";
const KEYRING_USER: &str = "default";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    Environment,
    Keyring,
    UserInput,
}

pub struct TokenProvider;

impl TokenProvider {
    pub fn resolve() -> Option<(String, TokenSource)> {
        dotenv::dotenv().ok();

        if let Ok(token) = std::env::var("YANDEX_MUSIC_TOKEN") {
            if !token.is_empty() {
                return Some((token, TokenSource::Environment));
            }
        }

        match Self::load_from_keyring() {
            Ok(token) if !token.is_empty() => {
                return Some((token, TokenSource::Keyring));
            }
            Ok(_) => tracing::debug!("Keyring entry exists but token is empty"),
            Err(e) => tracing::debug!("Keyring lookup failed: {e}"),
        }

        None
    }

    pub fn store(token: &str) -> color_eyre::Result<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)?;
        entry.set_password(token).map_err(|e| {
            tracing::error!("Failed to store token in keyring: {e}");
            e
        })?;
        Ok(())
    }

    fn load_from_keyring() -> color_eyre::Result<String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)?;
        let password = entry.get_password()?;
        Ok(password)
    }

    pub fn delete() -> color_eyre::Result<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)?;
        entry.delete_credential()?;
        Ok(())
    }

    pub async fn validate(token: String) -> color_eyre::Result<(Arc<YandexMusicClient>, u64)> {
        let mut headers = HeaderMap::new();

        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("OAuth {}", token))?,
        );
        headers.insert(
            "X-Yandex-Music-Client",
            HeaderValue::from_str(DEFAULT_CLIENT_ID)?,
        );
        headers.insert("Accept-Language", HeaderValue::from_str("en")?);
        headers.insert("Accept", HeaderValue::from_str("*/*")?);
        headers.insert(
            "Origin",
            HeaderValue::from_str("music-application://desktop")?,
        );

        let http_client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) YandexMusic/5.82.0 Chrome/140.0.7339.133 Electron/38.2.2 Safari/537.36")
            .default_headers(headers)
            .pool_max_idle_per_host(0)
            .build()?;

        let client = Arc::new(YandexMusicClient::from_client(http_client));

        let user_id = client
            .get_account_status()
            .await?
            .account
            .uid
            .ok_or(color_eyre::eyre::eyre!("No user id found"))?;

        Ok((client, user_id))
    }
}
