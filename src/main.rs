use yatui::{
    ui::app::App,
    util::{hook::set_panic_hook, log::initialize_logging},
};

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> color_eyre::Result<()> {
    setup()?;

    let mut app = App::new().await?;
    app.run().await
}

fn setup() -> color_eyre::Result<()> {
    color_eyre::install()?;
    dotenv::dotenv().ok();
    set_panic_hook();
    initialize_logging()
}
