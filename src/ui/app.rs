use std::sync::atomic::Ordering;

use flume::{Receiver, Sender};

use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    symbols::{self, border},
    widgets::{block::Title, Block, Borders, Widget},
    Frame,
};

use crate::{audio::backend::AudioPlayer, event::events::Event, keymap};

use super::{
    components::player::PlayerWidget,
    tui::{self, TerminalEvent},
};
pub struct App {
    pub event_rx: Receiver<Event>,
    pub event_tx: Sender<Event>,
    pub player: AudioPlayer,
    pub has_focus: bool,
    pub should_quit: bool,
}

impl App {
    pub async fn new() -> color_eyre::Result<Self> {
        let (event_tx, event_rx) = flume::unbounded();
        let player = AudioPlayer::new(event_tx.clone()).await?;

        Ok(Self {
            event_rx,
            event_tx,
            player,
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
            TerminalEvent::Init => self.player.init().await?,
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
                KeyCode::Char(' ') => self.player.play_pause(),
                KeyCode::Char('p') => self.player.play_previous().await,
                KeyCode::Char('n') => self.player.play_next().await,
                KeyCode::Char('+') => self.player.volume_up(10),
                KeyCode::Char('-') => self.player.volume_down(10),
                KeyCode::Char('=') => self.player.set_volume(100),
                KeyCode::Char('H') => self.player.seek_backwards(10),
                KeyCode::Char('L') => self.player.seek_forwards(10),
                KeyCode::Char('r') => self.player.toggle_repeat_mode(),
                KeyCode::Char('s') => self.player.toggle_shuffling(),
                KeyCode::Char('m') => self.player.toggle_mute(),
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
            Event::Play(track_id) => self.player.play_track(track_id).await,
            Event::TrackEnded => self.player.on_track_end().await,
            _ => {}
        }
    }

    fn ui(&self, frame: &mut Frame) {
        if self.has_focus {
            frame.render_widget(self, frame.size());
        }
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        buf.set_style(area, Style::new().bg(Color::from_u32(0x00181818)));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(area);

        let title = Title::default()
            .alignment(Alignment::Center)
            .content("Yandex Music");

        Block::new()
            .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
            .border_set(border::Set {
                bottom_left: symbols::line::NORMAL.vertical_right,
                bottom_right: symbols::line::NORMAL.vertical_left,
                ..symbols::border::PLAIN
            })
            .title(title)
            .render(chunks[0], buf);

        let track_title: &str;
        let track_artist: Option<String>;
        if let Some(track) = self.player.track.as_ref() {
            track_title = track.title.as_deref().unwrap_or("Unknown");
            track_artist = Some(
                track
                    .artists
                    .iter()
                    .map(|a| a.name.as_deref().unwrap_or("Unknown"))
                    .collect::<Vec<&str>>()
                    .join(", "),
            );
        } else {
            track_title = "No track";
            track_artist = None;
        }

        let player_widget = PlayerWidget::new(
            &self.player.track_progress,
            track_title,
            track_artist,
            self.player.repeat_mode,
            self.player.is_shuffled,
            if self.player.is_muted {
                0
            } else {
                self.player.volume
            },
            self.player.is_playing.load(Ordering::Relaxed),
        );
        player_widget.render(chunks[1], buf);
    }
}
