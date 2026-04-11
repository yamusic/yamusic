use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use yandex_music::model::playlist::Playlist;

use super::icons::PLAYLIST_ICON;
use crate::app::data::{ItemRenderer, ListItem};
use crate::app::theme::theme;

pub struct PlaylistRenderer;

impl PlaylistRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlaylistRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemRenderer<Playlist> for PlaylistRenderer {
    fn render(
        &self,
        playlist: &Playlist,
        _index: usize,
        is_selected: bool,
        _is_playing: bool,
    ) -> ListItem<'static> {
        let colors = theme();
        let selected_style = colors.selected;
        let text_muted = colors.muted;
        let text_style = Style::default().fg(colors.text.primary);
        let accent_style = Style::default().fg(colors.accent.primary);
        let title = playlist.title.clone();
        let track_count = playlist.track_count;
        let owner = playlist
            .owner
            .name
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());

        let mut spans = Vec::new();

        spans.push(Span::styled(format!("{} ", PLAYLIST_ICON), accent_style));

        spans.push(Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        ));

        spans.push(Span::styled(
            format!(" • {} tracks • {}", track_count, owner),
            text_muted,
        ));

        let style = if is_selected {
            selected_style.add_modifier(Modifier::BOLD)
        } else {
            text_style
        };

        ListItem::from_lines(vec![Line::from(spans)]).style(style)
    }
}
