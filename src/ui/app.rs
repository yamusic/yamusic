use std::sync::Arc;

use flume::{Receiver, Sender};

use ratatui::{
    crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind},
    Frame,
};

use crate::{
    audio::system::AudioSystem, event::events::Event, http::ApiService, keymap,
};

use super::tui::{self, TerminalEvent};

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

        self.handle_event(TerminalEvent::Init).await?;
        loop {
            tui.draw(|f| {
                self.ui(f);
            })?;

            if let Some(evt) = tui.next().await {
                self.handle_event(evt).await?;
            }

            self.handle_actions().await;

            if self.should_quit {
                break;
            }
        }

        tui.exit()?;

        Ok(())
    }

    async fn handle_event(
        &mut self,
        evt: TerminalEvent,
    ) -> color_eyre::Result<()> {
        match evt {
            TerminalEvent::Init => self.audio_system.init().await?,
            TerminalEvent::Quit => self.should_quit = true,
            TerminalEvent::FocusGained => self.has_focus = true,
            TerminalEvent::FocusLost => self.has_focus = false,
            TerminalEvent::Key(key) => self.handle_key_event(key).await,
            _ => {}
        }

        Ok(())
    }

    async fn handle_key_event(&mut self, evt: KeyEvent) {
        #[allow(clippy::single_match)]
        if evt.kind == KeyEventKind::Press {
            keymap! { evt,
                KeyCode::Char('c') | CONTROL => self.should_quit = true,
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Char(' ') => self.audio_system.play_pause(),
                KeyCode::Char('p') => self.audio_system.play_previous().await,
                KeyCode::Char('n') => self.audio_system.play_next().await,
                KeyCode::Char('+') => self.audio_system.volume_up(10),
                KeyCode::Char('-') => self.audio_system.volume_down(10),
                KeyCode::Char('=') => self.audio_system.set_volume(100),
                KeyCode::Char('H') => self.audio_system.seek_backwards(10),
                KeyCode::Char('L') => self.audio_system.seek_forwards(10),
                KeyCode::Char('r') => self.audio_system.toggle_repeat_mode(),
                KeyCode::Char('s') => self.audio_system.toggle_shuffle(),
                KeyCode::Char('m') => self.audio_system.toggle_mute(),
            }
        }
    }

    async fn handle_actions(&mut self) {
        while let Ok(evt) = self.event_rx.try_recv() {
            self.handle_action(evt).await;
        }
    }

    async fn handle_action(&mut self, evt: Event) {
        match evt {
            Event::Play(track_id) => {
                self.audio_system
                    .play_track_at_index(track_id as usize)
                    .await;
            }
            Event::TrackEnded => {
                self.audio_system.on_track_ended().await;
            }
            Event::TrackChanged(_track, _index) => {}
            _ => {}
        }
    }

    fn ui(&self, frame: &mut Frame) {
        if self.has_focus {
            frame.render_widget(self, frame.size());
        }
    }
}
