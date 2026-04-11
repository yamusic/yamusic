use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use yandex_music::model::album::Album;

use super::icons::ALBUM_ICON;
use crate::app::data::{ItemRenderer, ListItem};
use crate::app::theme::theme;

pub struct AlbumRenderer;

impl AlbumRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AlbumRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemRenderer<Album> for AlbumRenderer {
    fn render(
        &self,
        album: &Album,
        _index: usize,
        is_selected: bool,
        _is_playing: bool,
    ) -> ListItem<'static> {
        let colors = theme();
        let selected_style = colors.selected;
        let text_muted = colors.muted;
        let text_style = Style::default().fg(colors.text.primary);
        let accent_style = Style::default().fg(colors.accent.primary);
        let title = album.title.clone().unwrap_or_else(|| "Unknown".to_string());
        let artists: String = album
            .artists
            .iter()
            .filter_map(|a| a.name.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let year = album.year.map(|y| format!(" ({})", y)).unwrap_or_default();
        let track_count = album.track_count.unwrap_or(0);

        let mut spans = Vec::new();

        spans.push(Span::styled(format!("{} ", ALBUM_ICON), accent_style));

        spans.push(Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        ));

        if !artists.is_empty() {
            spans.push(Span::styled(format!(" - {}", artists), text_muted));
        }

        spans.push(Span::styled(
            format!("{} • {} tracks", year, track_count),
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
