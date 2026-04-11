use ratatui::{
    layout::Rect,
    style::Modifier,
    symbols::border,
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::{app::actions::Route, app::theme::theme};

pub struct Sidebar;

impl Sidebar {
    pub fn new() -> Self {
        Self
    }

    pub fn view(
        &self,
        frame: &mut Frame,
        area: Rect,
        current_route: &Route,
        border_style: ratatui::style::Style,
    ) {
        let selected = theme().selected;
        let text_muted = theme().muted;
        let items = [
            ("  Search", Route::Search),
            ("󰐻  My Wave", Route::Home),
            ("  My Favorites", Route::Liked),
            ("  Playlists", Route::Playlists),
        ];

        let list_items: Vec<ListItem> = items
            .iter()
            .map(|(label, route)| {
                let style = if current_route == route {
                    selected.add_modifier(Modifier::BOLD)
                } else {
                    text_muted
                };
                ListItem::new(format!("  {}", label)).style(style)
            })
            .collect();

        let list = List::new(list_items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(border::ROUNDED)
                .border_style(border_style)
                .title("yamusic")
                .title_alignment(ratatui::layout::Alignment::Center),
        );

        frame.render_widget(list, area);
    }
}
