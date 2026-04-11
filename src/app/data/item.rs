use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::app::theme::theme;

#[derive(Debug, Clone)]
pub struct ListItem<'a> {
    pub content: Vec<Line<'a>>,
    pub prefix_lines: Option<Vec<Line<'a>>>,
    pub height: u16,
    pub style: Style,
    pub cover_url: Option<String>,
}

impl<'a> ListItem<'a> {
    pub fn simple(text: impl Into<String>) -> Self {
        Self {
            content: vec![Line::from(text.into())],
            prefix_lines: None,
            height: 1,
            style: Style::default(),
            cover_url: None,
        }
    }

    pub fn two_line(title: impl Into<String>, subtitle: impl Into<String>) -> Self {
        let muted_style = Style::default().fg(theme().text.muted);
        Self {
            content: vec![
                Line::from(title.into()),
                Line::from(Span::styled(subtitle.into(), muted_style)),
            ],
            prefix_lines: None,
            height: 2,
            style: Style::default(),
            cover_url: None,
        }
    }

    pub fn from_lines(lines: Vec<Line<'a>>) -> Self {
        let height = lines.len() as u16;
        Self {
            content: lines,
            prefix_lines: None,
            height,
            style: Style::default(),
            cover_url: None,
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn with_cover(mut self, url: Option<String>) -> Self {
        self.cover_url = url;
        self
    }

    pub fn with_prefix(mut self, prefix_lines: Vec<Line<'a>>) -> Self {
        self.prefix_lines = Some(prefix_lines);
        self
    }

    pub fn bold(mut self) -> Self {
        self.style = self.style.add_modifier(Modifier::BOLD);
        self
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.style = self.style.fg(color);
        self
    }

    pub fn dim(mut self) -> Self {
        self.style = self.style.add_modifier(Modifier::DIM);
        self
    }
}

impl<'a> From<ListItem<'a>> for ratatui::widgets::ListItem<'a> {
    fn from(item: ListItem<'a>) -> Self {
        ratatui::widgets::ListItem::new(item.content).style(item.style)
    }
}

#[derive(Debug, Clone, Default)]
pub struct MatchHighlights {
    pub title: Vec<usize>,
    pub artist: Vec<usize>,
    pub album: Vec<usize>,
    pub search_scope: Option<SearchScope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchScope {
    Full,
    Title,
    Artist,
    Album,
}

impl SearchScope {
    pub fn next(&self) -> Self {
        match self {
            SearchScope::Full => SearchScope::Title,
            SearchScope::Title => SearchScope::Artist,
            SearchScope::Artist => SearchScope::Album,
            SearchScope::Album => SearchScope::Full,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            SearchScope::Full => SearchScope::Album,
            SearchScope::Album => SearchScope::Artist,
            SearchScope::Artist => SearchScope::Title,
            SearchScope::Title => SearchScope::Full,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SearchScope::Full => "ALL",
            SearchScope::Title => "TITLE",
            SearchScope::Artist => "ARTIST",
            SearchScope::Album => "ALBUM",
        }
    }
}

pub trait ItemRenderer<T>: Send + Sync {
    fn render(
        &self,
        item: &T,
        index: usize,
        is_selected: bool,
        is_playing: bool,
    ) -> ListItem<'static>;

    fn render_with_context(
        &self,
        item: &T,
        index: usize,
        is_selected: bool,
        is_playing: bool,
        available_width: u16,
        highlights: &MatchHighlights,
    ) -> ListItem<'static> {
        let _ = (available_width, highlights);
        self.render(item, index, is_selected, is_playing)
    }

    fn item_height(&self, item: &T) -> u16 {
        let _ = item;
        1
    }
}

pub struct FnRenderer<T, F> {
    render_fn: F,
    _marker: std::marker::PhantomData<T>,
}

impl<T, F> FnRenderer<T, F>
where
    F: Fn(&T, usize, bool, bool) -> ListItem<'static> + Send + Sync,
{
    pub fn new(render_fn: F) -> Self {
        Self {
            render_fn,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T, F> ItemRenderer<T> for FnRenderer<T, F>
where
    T: Send + Sync,
    F: Fn(&T, usize, bool, bool) -> ListItem<'static> + Send + Sync,
{
    fn render(
        &self,
        item: &T,
        index: usize,
        is_selected: bool,
        is_playing: bool,
    ) -> ListItem<'static> {
        (self.render_fn)(item, index, is_selected, is_playing)
    }
}
