use std::sync::Arc;

use flume::Receiver;

use ratatui::Frame;

use crate::{
    audio::system::AudioSystem,
    event::events::Event,
    http::ApiService,
    ui::{
        context::{AppContext, GlobalUiState},
        layout::AppLayout,
        traits::Component,
        tui::{self, TerminalEvent},
        util::handler::EventHandler,
    },
};

pub struct App {
    pub ctx: AppContext,
    pub state: GlobalUiState,
    pub view_stack: Vec<Box<dyn Component>>,
    pub event_rx: Receiver<Event>,
    pub has_focus: bool,
    pub should_quit: bool,
}

impl App {
    pub async fn new() -> color_eyre::Result<Self> {
        let (event_tx, event_rx) = flume::unbounded();
        let api = Arc::new(ApiService::new().await?);
        let audio_system = AudioSystem::new(event_tx.clone(), api.clone()).await?;

        let ctx = AppContext {
            api,
            audio_system,
            event_tx,
        };

        let state = GlobalUiState::default();

        Ok(Self {
            ctx,
            state,
            view_stack: vec![Box::new(crate::ui::views::MyVibe::default())],
            event_rx,
            has_focus: true,
            should_quit: false,
        })
    }

    pub async fn run(&mut self) -> color_eyre::Result<()> {
        let mut tui = tui::Tui::new()?.mouse(true);
        tui.enter()?;

        EventHandler::handle_event(self, TerminalEvent::Init).await?;

        tui.draw(|f| {
            self.ui(f);
        })?;

        while !self.should_quit {
            if EventHandler::handle_events(self, &tui).await? {
                tui.draw(|f| {
                    self.ui(f);
                })?;
            }
        }

        Ok(())
    }

    fn ui(&mut self, frame: &mut Frame) {
        if self.has_focus {
            AppLayout::new(self).render(frame, frame.area());
        }
    }
}
