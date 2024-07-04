use flume::{Receiver, Sender};

use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{block::Title, Block, Widget},
    Frame,
};

use crate::{audio::backend::AudioPlayer, event::events::GlobalEvent};

use super::{
    components::progress::ProgressWidget,
    tui::{self, Event},
};
pub struct App {
    pub event_rx: Receiver<GlobalEvent>,
    pub event_tx: Sender<GlobalEvent>,
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

        loop {
            tui.draw(|f| {
                self.ui(f);
            })?;

            if let Some(evt) = tui.next().await {
                self.handle_event(evt).await?;
            };

            if self.should_quit {
                break;
            }
        }

        tui.exit()?;

        Ok(())
    }

    async fn handle_event(&mut self, evt: Event) -> color_eyre::Result<()> {
        match evt {
            Event::Init => self.player.init().await?,
            Event::Quit => self.should_quit = true,
            Event::Tick => self.handle_actions().await,
            Event::FocusGained => self.has_focus = true,
            Event::FocusLost => self.has_focus = false,
            Event::Key(key) => self.handle_key_event(key).await,
            _ => {}
        }

        Ok(())
    }

    async fn handle_key_event(&mut self, evt: KeyEvent) {
        #[allow(clippy::single_match)]
        match evt.kind {
            KeyEventKind::Press => match evt.code {
                KeyCode::Char('c') => {
                    if evt.modifiers == event::KeyModifiers::CONTROL {
                        self.should_quit = true;
                    }
                }
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Char('p') => self.player.play_previous().await,
                KeyCode::Char('n') => self.player.play_next().await,
                KeyCode::Char('+') => self.player.volume_up(10),
                KeyCode::Char('-') => self.player.volume_down(10),
                KeyCode::Char('=') => self.player.set_volume(100),
                KeyCode::Left => self.player.seek_backwards(10),
                KeyCode::Right => self.player.seek_forwards(10),
                _ => {}
            },
            _ => {}
        }
    }

    async fn handle_actions(&mut self) {
        while let Ok(evt) = self.event_rx.try_recv() {
            self.handle_action(evt).await;
        }
    }

    async fn handle_action(&mut self, evt: GlobalEvent) {
        match evt {
            GlobalEvent::Play(track_id) => {
                self.player.play_track(track_id).await
            }
            GlobalEvent::TrackEnded => self.player.play_next().await,
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
            .constraints([Constraint::Min(1), Constraint::Length(8)])
            .split(area);

        let mut title =
            Title::default().alignment(ratatui::layout::Alignment::Center);

        if let Some(track) = self.player.track.as_ref() {
            title = title.content(format!(
                "{} @ {}%",
                track.title.as_deref().unwrap_or("Unknown"),
                self.player.volume
            ));
        } else {
            title = title.content("No track");
        }
        Block::bordered().title(title).render(chunks[0], buf);

        let progress_widget = ProgressWidget::new(&self.player.track_progress);
        progress_widget.render(chunks[1], buf);
    }
}
