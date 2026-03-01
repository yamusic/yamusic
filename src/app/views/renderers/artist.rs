use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use yandex_music::model::artist::Artist;

use super::icons::ARTIST_ICON;
use crate::app::data::{ItemRenderer, ListItem};

pub struct ArtistRenderer;

impl ArtistRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ArtistRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemRenderer<Artist> for ArtistRenderer {
    fn render(
        &self,
        artist: &Artist,
        _index: usize,
        is_selected: bool,
        _is_playing: bool,
    ) -> ListItem<'static> {
        let styles = crate::framework::theme::global_theme().styles().get();
        let name = artist.name.clone().unwrap_or_else(|| "Unknown".to_string());
        let genres: String = artist
            .genres
            .as_ref()
            .map(|g| g.join(", "))
            .unwrap_or_default();

        let mut spans = Vec::new();

        spans.push(Span::styled(format!("{} ", ARTIST_ICON), styles.accent));

        spans.push(Span::styled(
            name,
            Style::default().add_modifier(Modifier::BOLD),
        ));

        if !genres.is_empty() {
            spans.push(Span::styled(format!(" • {}", genres), styles.text_muted));
        }

        let style = if is_selected {
            styles.selected.add_modifier(Modifier::BOLD)
        } else {
            styles.text
        };

        ListItem::from_lines(vec![Line::from(spans)]).style(style)
    }
}
