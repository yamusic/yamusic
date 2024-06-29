use yatui::{
    audio::backend::AudioPlayer,
    event::events::GlobalEvent,
    ui::{app::App, tui},
};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    let mut terminal = tui::init()?;

    let (tx, rx) = crossbeam_channel::unbounded::<GlobalEvent>();
    let player = AudioPlayer::new(tx.clone(), rx.clone()).await.unwrap();

    // player.play_track(123987398).await;
    let mut app = App {
        player,
        tx: tx.clone(),
        rx,
        exit: false,
        tracks: Vec::new(),
        current_track: None,
        current_track_index: 0,
    };

    let app_result = app.run(&mut terminal).await;

    tui::restore()?;

    app_result
}
