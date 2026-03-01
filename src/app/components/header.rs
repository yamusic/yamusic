use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::framework::{signals::Signal, theme::ThemeStyles};
use im::Vector;

#[derive(Debug, Clone, PartialEq)]
pub enum HeaderLine {
    Text(String),
    Title(String),
    Subtitle(String),
    Spans(Vec<Span<'static>>),
}

impl HeaderLine {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn title(text: impl Into<String>) -> Self {
        Self::Title(text.into())
    }

    pub fn subtitle(text: impl Into<String>) -> Self {
        Self::Subtitle(text.into())
    }

    pub fn from_spans(spans: Vec<Span<'static>>) -> Self {
        Self::Spans(spans)
    }
}

pub struct Header {
    lines: Signal<Vector<HeaderLine>>,
    height: u16,
    show_border: bool,
    theme: Signal<ThemeStyles>,
}

impl Header {
    pub fn new(lines: Vec<HeaderLine>, theme: Signal<ThemeStyles>) -> Self {
        let height = lines.len() as u16 + 2;
        Self {
            lines: Signal::new(Vector::from(lines)),
            height,
            show_border: true,
            theme,
        }
    }

    pub fn from_signal(lines: Signal<Vector<HeaderLine>>, theme: Signal<ThemeStyles>) -> Self {
        Self {
            lines,
            height: 6,
            show_border: true,
            theme,
        }
    }

    pub fn with_height(mut self, height: u16) -> Self {
        self.height = height;
        self
    }

    pub fn with_border(mut self, show: bool) -> Self {
        self.show_border = show;
        self
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn set_lines(&self, lines: Vec<HeaderLine>) {
        self.lines.set(Vector::from(lines));
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        let lines = self.lines.with(|l| l.clone());
        let styles = self.theme.get();
        let content: Vec<Line<'static>> = lines
            .into_iter()
            .map(|line| match line {
                HeaderLine::Text(text) => Line::from(vec![Span::styled(text, styles.text)]),
                HeaderLine::Title(text) => Line::from(vec![Span::styled(
                    text,
                    styles.accent.add_modifier(Modifier::BOLD),
                )]),
                HeaderLine::Subtitle(text) => {
                    Line::from(vec![Span::styled(text, styles.text_muted)])
                }
                HeaderLine::Spans(spans) => Line::from(spans),
            })
            .collect();

        let mut block = Block::default();
        if self.show_border {
            block = block.borders(Borders::BOTTOM);
        }
        block = block.padding(ratatui::widgets::Padding::new(1, 1, 0, 1));

        let paragraph = Paragraph::new(content).block(block);
        frame.render_widget(paragraph, area);
    }
}

pub struct HeaderBuilder;

impl HeaderBuilder {
    pub fn playlist(
        title: &str,
        owner: &str,
        track_count: usize,
        duration: Option<String>,
        theme: Signal<ThemeStyles>,
    ) -> Header {
        let mut lines = vec![
            HeaderLine::title(title),
            HeaderLine::subtitle(format!("by {}", owner)),
            HeaderLine::text(format!("{} tracks", track_count)),
        ];

        if let Some(dur) = duration {
            lines.push(HeaderLine::text(dur));
        }

        Header::new(lines, theme)
    }

    pub fn album(
        title: &str,
        artists: &str,
        year: Option<i32>,
        track_count: usize,
        theme: Signal<ThemeStyles>,
    ) -> Header {
        let mut lines = vec![HeaderLine::title(title), HeaderLine::subtitle(artists)];

        if let Some(y) = year {
            lines.push(HeaderLine::text(format!("{} • {} tracks", y, track_count)));
        } else {
            lines.push(HeaderLine::text(format!("{} tracks", track_count)));
        }

        Header::new(lines, theme)
    }

    pub fn artist(
        name: &str,
        genres: &str,
        likes: u64,
        track_count: usize,
        theme: Signal<ThemeStyles>,
    ) -> Header {
        Header::new(
            vec![
                HeaderLine::title(name),
                HeaderLine::subtitle(genres),
                HeaderLine::text(format!("{} tracks • {} likes", track_count, likes)),
            ],
            theme,
        )
    }

    pub fn search(query: &str, result_count: usize, theme: Signal<ThemeStyles>) -> Header {
        Header::new(
            vec![
                HeaderLine::title(format!("Search: {}", query)),
                HeaderLine::text(format!("{} results", result_count)),
            ],
            theme,
        )
    }
}
