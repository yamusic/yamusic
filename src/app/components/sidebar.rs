use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    symbols::border,
    widgets::{Block, Borders, List, ListItem},
};

use crate::{
    app::actions::Route,
    framework::{signals::Signal, theme::ThemeStyles},
};

pub struct Sidebar {
    theme: Signal<ThemeStyles>,
}

impl Sidebar {
    pub fn new(theme: Signal<ThemeStyles>) -> Self {
        Self { theme }
    }

    pub fn view(
        &self,
        frame: &mut Frame,
        area: Rect,
        current_route: &Route,
        border_style: ratatui::style::Style,
    ) {
        let styles = self.theme.get();
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
                    styles.selected.add_modifier(Modifier::BOLD)
                } else {
                    styles.text_muted
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
