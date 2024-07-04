use yatui::ui::{app::App, hook::install_hooks, log::initialize_logging};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    setup()?;

    let mut app = App::new().await?;
    app.run().await
}

fn setup() -> color_eyre::Result<()> {
    dotenv::dotenv().ok();
    install_hooks()?;
    initialize_logging()
}
