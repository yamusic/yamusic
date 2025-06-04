use std::sync::Arc;

use flume::{Receiver, Sender};

use ratatui::Frame;

use crate::{
    audio::system::AudioSystem, event::events::Event, http::ApiService,
};

use super::{
    tui::{self, TerminalEvent},
    util::handler::EventHandler,
};

pub struct App {
    pub event_rx: Receiver<Event>,
    pub event_tx: Sender<Event>,
    pub api: Arc<ApiService>,
    pub audio_system: AudioSystem,
    pub has_focus: bool,
    pub should_quit: bool,
}

impl App {
    pub async fn new() -> color_eyre::Result<Self> {
        let (event_tx, event_rx) = flume::unbounded();
        let api = Arc::new(ApiService::new().await?);
        let audio_system =
            AudioSystem::new(event_tx.clone(), api.clone()).await?;

        Ok(Self {
            event_rx,
            event_tx,
            audio_system,
            api,
            has_focus: true,
            should_quit: false,
        })
    }

    pub async fn run(&mut self) -> color_eyre::Result<()> {
        let mut tui = tui::Tui::new()?;
        tui.enter()?;

        EventHandler::handle_event(self, TerminalEvent::Init).await?;
        while !self.should_quit {
            tui.draw(|f| {
                self.ui(f);
            })?;

            EventHandler::handle_events(self, &tui).await?;
        }

        tui.exit()?;
        Ok(())
    }

    fn ui(&self, frame: &mut Frame) {
        if self.has_focus {
            frame.render_widget(self, frame.area());
        }
    }
}
