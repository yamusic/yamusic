use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::{
    event::events::Event,
    keymap,
    ui::{
        app::App,
        tui::{TerminalEvent, Tui},
    },
};

pub struct EventHandler;

impl EventHandler {
    pub async fn handle_events(app: &mut App, tui: &Tui) -> color_eyre::Result<()> {
        if let Some(evt) = tui.next().await {
            Self::handle_event(app, evt).await?;
        }

        while let Ok(evt) = app.event_rx.try_recv() {
            Self::handle_action(app, evt).await;
        }

        Ok(())
    }

    pub async fn handle_event(app: &mut App, evt: TerminalEvent) -> color_eyre::Result<()> {
        match evt {
            TerminalEvent::Init => app.audio_system.init().await?,
            TerminalEvent::Quit => app.should_quit = true,
            TerminalEvent::FocusGained => app.has_focus = true,
            TerminalEvent::FocusLost => app.has_focus = false,
            TerminalEvent::Key(key) => Self::handle_key_event(app, key).await,
            _ => {}
        }

        Ok(())
    }

    pub async fn handle_action(app: &mut App, evt: Event) {
        match evt {
            Event::Play(track_id) => {
                app.audio_system
                    .play_track_at_index(track_id as usize)
                    .await;
            }
            Event::TrackEnded => {
                app.audio_system.on_track_ended().await;
            }
            Event::TrackStarted(_track, _index) => {}
            _ => {}
        }
    }

    async fn handle_key_event(app: &mut App, evt: KeyEvent) {
        #[allow(clippy::single_match)]
        if evt.kind == KeyEventKind::Press {
            keymap! { evt,
                KeyCode::Char('c') | CONTROL => app.should_quit = true,
                KeyCode::Char('q') => app.should_quit = true,
                KeyCode::Char(' ') => app.audio_system.play_pause(),
                KeyCode::Char('p') => app.audio_system.play_previous().await,
                KeyCode::Char('n') => app.audio_system.play_next().await,
                KeyCode::Char('+') => app.audio_system.volume_up(10),
                KeyCode::Char('-') => app.audio_system.volume_down(10),
                KeyCode::Char('=') => app.audio_system.set_volume(100),
                KeyCode::Char('H') => app.audio_system.seek_backwards(10),
                KeyCode::Char('L') => app.audio_system.seek_forwards(10),
                KeyCode::Char('r') => app.audio_system.toggle_repeat_mode(),
                KeyCode::Char('s') => app.audio_system.toggle_shuffle(),
                KeyCode::Char('m') => app.audio_system.toggle_mute(),
            }
        }
    }
}
