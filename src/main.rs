use std::sync::Arc;
use yamusic::{
    app::App,
    audio::system::AudioSystem,
    auth::{LoginScreen, TokenProvider},
    http::ApiService,
    util::{hook::set_panic_hook, log::initialize_logging},
};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> color_eyre::Result<()> {
    setup()?;

    let (client, user_id) = resolve_and_verify_token().await?;

    let (event_tx, event_rx) = flume::unbounded();
    let api = Arc::new(ApiService::new("".to_string(), Some(client), Some(user_id)).await?);

    let audio = AudioSystem::new(event_tx.clone(), api.clone()).await?;
    let mut app = App::new(audio, api, event_tx, event_rx).await?;
    app.run().await
}

async fn resolve_and_verify_token()
-> color_eyre::Result<(std::sync::Arc<yandex_music::YandexMusicClient>, u64)> {
    if let Some((token, _)) = TokenProvider::resolve() {
        match TokenProvider::validate(token).await {
            Ok((client, user_id)) => return Ok((client, user_id)),
            Err(_) => {
                let _ = TokenProvider::delete();
            }
        }
    }

    let mut login = LoginScreen::new();
    match login.run().await? {
        Some((client, user_id)) => Ok((client, user_id)),
        None => {
            std::process::exit(0);
        }
    }
}

fn setup() -> color_eyre::Result<()> {
    color_eyre::install()?;
    dotenv::dotenv().ok();
    set_panic_hook();
    initialize_logging()
}
