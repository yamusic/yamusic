use crossbeam_channel::{Receiver, Sender};
use std::io;

use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{block::Title, Block, BorderType, Borders, Widget},
    Frame,
};
use yandex_music::model::track_model::track::Track;

use crate::{audio::backend::AudioPlayer, event::events::GlobalEvent};

use super::tui;

pub struct App {
    pub player: AudioPlayer,
    pub tx: Sender<GlobalEvent>,
    pub rx: Receiver<GlobalEvent>,
    pub exit: bool,
    pub tracks: Vec<Track>,
    pub current_track: Option<Track>,
    pub current_track_index: usize,
}

impl App {
    pub async fn run(&mut self, terminal: &mut tui::Tui) -> io::Result<()> {
        self.tx.send(GlobalEvent::Initialize).unwrap();
        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_tui_events()?;
            self.handle_audio_events().await;
        }

        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.size());
    }

    #[allow(clippy::single_match)]
    fn handle_tui_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(event) => {
                if event.kind == KeyEventKind::Press {
                    self.handle_key_event(event)
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('c') | KeyCode::Char('с') => {
                if key_event.modifiers.contains(event::KeyModifiers::CONTROL) {
                    self.exit();
                }
            }
            KeyCode::Char('q') | KeyCode::Char('й') => self.exit(),
            KeyCode::Char('p') | KeyCode::Char('з') => {
                if self.current_track_index >= self.tracks.len() {
                    self.current_track_index = 0;
                }

                let track = &self.tracks[self.current_track_index];
                // [random(0, (self.tracks.len() as i32) - 1) as usize];
                self.current_track_index += 1;
                let track_id = track.id;
                self.current_track = Some(track.clone());

                self.tx.send(GlobalEvent::Play(track_id)).unwrap();
            }
            _ => {}
        }
    }

    async fn handle_audio_events(&mut self) {
        let event = match self.player.rx.try_recv() {
            Ok(event) => event,
            _ => return,
        };

        match event {
            GlobalEvent::TracksFetched(tracks) => self.tracks = tracks,
            GlobalEvent::Initialize => self.player.fetch_tracks().await,
            GlobalEvent::Play(track_id) => {
                self.player.play_track(track_id).await
            }
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, Style::new().bg(Color::from_u32(0x00181818)));

        let block = match self.current_track.as_ref() {
            Some(track) => Block::bordered()
                .border_type(BorderType::Plain)
                .borders(Borders::ALL)
                .title(
                    Title::from(format!(
                        "\u{f28b}  Now playing {} by {} [{}/{}]",
                        track
                            .title
                            .clone()
                            .unwrap_or_else(|| "Unknown".to_string()),
                        track
                            .artists
                            .iter()
                            .map(|a| {
                                a.name.clone().unwrap_or_else(|| {
                                    "Unknown Artist".to_string()
                                })
                            })
                            .collect::<Vec<String>>()
                            .join(", "),
                        self.current_track_index,
                        self.tracks.len()
                    ))
                    .alignment(Alignment::Center),
                ),
            None => Block::bordered()
                .border_type(BorderType::Plain)
                .borders(Borders::ALL)
                .title(
                    Title::from(format!(
                        "\u{f144}  No track is playing out of {} provided",
                        self.tracks.len()
                    ))
                    .alignment(Alignment::Center),
                ),
        };

        block.render(area, buf);
    }
}
