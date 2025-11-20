use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List, ListItem, Widget},
};

use crate::util::colors;

pub struct Sidebar<'a> {
    items: Vec<&'a str>,
    selected_index: usize,
}

impl<'a> Sidebar<'a> {
    pub fn new(items: Vec<&'a str>, selected_index: usize) -> Self {
        Self {
            items,
            selected_index,
        }
    }
}

impl<'a> Widget for Sidebar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if i == self.selected_index {
                    Style::default()
                        .fg(colors::PRIMARY)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(colors::NEUTRAL)
                };
                ListItem::new(format!("  {}", item)).style(style)
            })
            .collect();

        let list = List::new(items);

        list.render(area, buf);
    }
}
