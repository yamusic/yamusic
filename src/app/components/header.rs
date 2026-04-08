use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};
use std::sync::Arc;

use crate::{
    cache::image::ImageCache,
    framework::{signals::Signal, theme::ThemeStyles},
};
use im::Vector;
use image::DynamicImage;

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
    cover_url: Option<String>,
    cover_protocol: Option<StatefulProtocol>,
    last_cover: Option<Arc<DynamicImage>>,
}

impl Header {
    pub fn new(lines: Vec<HeaderLine>, theme: Signal<ThemeStyles>) -> Self {
        let height = lines.len() as u16 + 1;
        Self {
            lines: Signal::new(Vector::from(lines)),
            height,
            show_border: true,
            theme,
            cover_url: None,
            cover_protocol: None,
            last_cover: None,
        }
    }

    pub fn from_signal(lines: Signal<Vector<HeaderLine>>, theme: Signal<ThemeStyles>) -> Self {
        Self {
            lines,
            height: 5,
            show_border: true,
            theme,
            cover_url: None,
            cover_protocol: None,
            last_cover: None,
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

    pub fn with_cover_url(mut self, url: Option<String>) -> Self {
        self.cover_url = url;
        self
    }

    pub fn set_cover_url(&mut self, url: Option<String>) {
        if self.cover_url != url {
            self.cover_url = url;
            self.cover_protocol = None;
            self.last_cover = None;
        }
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn set_lines(&self, lines: Vec<HeaderLine>) {
        self.lines.set(Vector::from(lines));
    }

    fn resolve_cover(&mut self, picker: &mut Picker, img_height: u16) {
        let Some(url) = &self.cover_url else {
            self.cover_protocol = None;
            self.last_cover = None;
            return;
        };

        let cache = ImageCache::global();
        let current = cache.get_or_fetch(url);

        let changed = match (&self.last_cover, &current) {
            (Some(old), Some(new)) => !Arc::ptr_eq(old, new),
            (None, None) => false,
            _ => true,
        };

        if changed {
            self.cover_protocol = current
                .as_ref()
                .map(|img| picker.new_resize_protocol((**img).clone()));
            self.last_cover = current;
        }

        if self.cover_protocol.is_none() {
            if let Some(img) = &self.last_cover {
                if img_height > 0 {
                    self.cover_protocol = Some(picker.new_resize_protocol((**img).clone()));
                }
            }
        }
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        self.view_inner(frame, area, None);
    }

    pub fn view_with_picker(&mut self, frame: &mut Frame, area: Rect, picker: &mut Picker) {
        self.resolve_cover(picker, area.height.saturating_sub(2));
        self.view_inner_mut(frame, area);
    }

    fn view_inner(&self, frame: &mut Frame, area: Rect, _picker: Option<()>) {
        let lines = self.lines.with(|l| l.clone());
        let styles = self.theme.get();

        let content: Vec<Line<'static>> = Self::build_content(lines, &styles);

        let mut block = Block::default();
        if self.show_border {
            block = block.borders(Borders::BOTTOM).border_style(styles.block);
        }
        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let text_area = ratatui::layout::Rect {
            x: inner_area.x.saturating_add(1),
            y: inner_area.y,
            width: inner_area.width.saturating_sub(2),
            height: inner_area.height,
        };

        let paragraph = Paragraph::new(content);
        frame.render_widget(paragraph, text_area);
    }

    fn view_inner_mut(&mut self, frame: &mut Frame, area: Rect) {
        let lines = self.lines.with(|l| l.clone());
        let styles = self.theme.get();

        let content: Vec<Line<'static>> = Self::build_content(lines, &styles);

        let mut block = Block::default();
        if self.show_border {
            block = block.borders(Borders::BOTTOM).border_style(styles.block);
        }
        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let has_cover = self.cover_protocol.is_some();
        let inner_h = inner_area.height;
        let img_w = if has_cover {
            inner_h.saturating_mul(2).min(area.width / 4).max(4)
        } else {
            0
        };

        if img_w > 0 {
            let img_area = Rect {
                x: inner_area.x + 1,
                y: inner_area.y,
                width: img_w,
                height: inner_h,
            };

            if let Some(proto) = &mut self.cover_protocol {
                frame.render_stateful_widget(StatefulImage::new(), img_area, proto);
            }

            let text_x = inner_area.x + img_w + 2;
            let text_w = inner_area.width.saturating_sub(img_w + 3);
            let text_area = Rect {
                x: text_x,
                y: inner_area.y,
                width: text_w,
                height: inner_area.height,
            };

            let padded_text_area = ratatui::layout::Rect {
                x: text_area.x,
                y: text_area.y,
                width: text_area.width.saturating_sub(1),
                height: text_area.height,
            };
            frame.render_widget(Paragraph::new(content), padded_text_area);
        } else {
            let padded_text_area = ratatui::layout::Rect {
                x: inner_area.x.saturating_add(1),
                y: inner_area.y,
                width: inner_area.width.saturating_sub(2),
                height: inner_area.height,
            };
            let paragraph = Paragraph::new(content);
            frame.render_widget(paragraph, padded_text_area);
        }
    }

    fn build_content(
        lines: im::Vector<HeaderLine>,
        styles: &crate::framework::theme::ThemeStyles,
    ) -> Vec<Line<'static>> {
        lines
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
            .collect()
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
