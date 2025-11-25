use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    symbols::{self, border},
    widgets::{Block, Borders},
};

use crate::{
    ui::{
        app::App,
        components::{player::PlayerWidget, sidebar::Sidebar},
    },
    util::colors,
};

pub struct AppLayout<'a> {
    pub app: &'a mut App,
}

impl<'a> AppLayout<'a> {
    pub fn new(app: &'a mut App) -> Self {
        Self { app }
    }

    pub fn render(self, f: &mut Frame, area: Rect) {
        let buf = f.buffer_mut();
        buf.set_style(area, Style::new().bg(colors::BACKGROUND));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(area);

        let main_area = chunks[0];
        let player_area = chunks[1];

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(25), Constraint::Min(1)])
            .split(main_area);

        let sidebar_area = main_chunks[0];
        let content_area = main_chunks[1];
        let sidebar_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .title("yamusic")
            .title_alignment(Alignment::Center);

        let content_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::Set {
                ..symbols::border::ROUNDED
            });

        let sidebar_inner = sidebar_block.inner(sidebar_area);
        let content_inner = content_block.inner(content_area);

        f.render_widget(sidebar_block, sidebar_area);
        f.render_widget(content_block, content_area);
        let sidebar_items = vec!["  Search", "󰐻  My Wave", "  My Favorites", "  Playlists"];
        f.render_widget(
            Sidebar::new(sidebar_items, self.app.state.ui.sidebar_index),
            sidebar_inner,
        );

        self.app
            .router
            .render(f, content_inner, &self.app.state, &self.app.ctx);

        let track_title: String;
        let track_artist: Option<String>;

        let current_track = self.app.ctx.audio_system.current_track();
        if let Some(track) = &current_track {
            track_title = track.title.clone().unwrap_or("Unknown".to_string());
            track_artist = Some(
                track
                    .artists
                    .iter()
                    .map(|a| a.name.as_deref().unwrap_or("Unknown"))
                    .collect::<Vec<&str>>()
                    .join(", "),
            );
        } else {
            track_title = "No track".to_string();
            track_artist = None;
        }

        let player_widget = PlayerWidget::new(
            self.app.ctx.audio_system.track_progress(),
            &track_title,
            track_artist,
            self.app.ctx.audio_system.repeat_mode(),
            self.app.ctx.audio_system.is_shuffled(),
            if self.app.ctx.audio_system.is_muted() {
                0
            } else {
                self.app.ctx.audio_system.volume()
            },
            self.app.ctx.audio_system.is_playing(),
        );
        f.render_widget(player_widget, player_area);
    }
}
