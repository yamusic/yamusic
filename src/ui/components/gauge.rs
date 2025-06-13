use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    symbols,
    text::Span,
    widgets::{Block, Widget},
};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CustomGauge<'a> {
    block: Option<Block<'a>>,
    played_ratio: f64,
    buffered_ratio: f64,
    label: Option<Span<'a>>,
    use_unicode: bool,
    style: Style,
    played_style: Style,
    buffered_style: Style,
    remaining_style: Style,
}

impl<'a> CustomGauge<'a> {
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn ratios(mut self, played: f64, buffered: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&played),
            "Played ratio must be between 0 and 1"
        );
        assert!(
            (0.0..=1.0).contains(&buffered),
            "Buffered ratio must be between 0 and 1"
        );

        self.played_ratio = played;
        self.buffered_ratio = buffered;
        self
    }

    pub fn label<T>(mut self, label: T) -> Self
    where
        T: Into<Span<'a>>,
    {
        self.label = Some(label.into());
        self
    }

    pub const fn use_unicode(mut self, use_unicode: bool) -> Self {
        self.use_unicode = use_unicode;
        self
    }

    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    pub fn played_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.played_style = style.into();
        self
    }

    pub fn buffered_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.buffered_style = style.into();
        self
    }

    pub fn remaining_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.remaining_style = style.into();
        self
    }
}

fn get_unicode_block(frac: f64) -> &'static str {
    match (frac * 8.0).round() as u16 {
        0 => " ",
        1 => symbols::block::ONE_EIGHTH,
        2 => symbols::block::ONE_QUARTER,
        3 => symbols::block::THREE_EIGHTHS,
        4 => symbols::block::HALF,
        5 => symbols::block::FIVE_EIGHTHS,
        6 => symbols::block::THREE_QUARTERS,
        7 => symbols::block::SEVEN_EIGHTHS,
        _ => symbols::block::FULL,
    }
}

impl Widget for CustomGauge<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        if let Some(ref block) = self.block {
            block.render(area, buf);
        }

        let inner = self.block.as_ref().map_or(area, |b| b.inner(area));
        if inner.is_empty() {
            return;
        }

        self.render_gauge(inner, buf);
    }
}

impl CustomGauge<'_> {
    fn render_gauge(&self, gauge_area: Rect, buf: &mut Buffer) {
        if gauge_area.is_empty() {
            return;
        }

        let width = gauge_area.width as f64;
        let played_pos = width * self.played_ratio;
        let buffered_pos = width * self.buffered_ratio;

        let label = if let Some(label) = self.label.as_ref() {
            label
        } else {
            &Span::raw(format!(
                "{}% / {}%",
                (self.played_ratio * 100.0).round() as u16,
                (self.buffered_ratio * 100.0).round() as u16
            ))
        };

        let label_col = gauge_area.left() + (gauge_area.width - label.width() as u16) / 2;
        let label_row = gauge_area.top() + gauge_area.height / 2;

        for y in gauge_area.top()..gauge_area.bottom() {
            for x in gauge_area.left()..gauge_area.right() {
                let pos = x - gauge_area.left();
                let pos_f64 = pos as f64;

                let mut symbol = symbols::block::FULL;
                let mut style = self.remaining_style;

                if pos_f64 < played_pos {
                    style = self.played_style;
                    if self.use_unicode && pos_f64 + 1.0 > played_pos {
                        let frac = played_pos - pos_f64;
                        symbol = get_unicode_block(frac);
                    }
                } else if pos_f64 < buffered_pos {
                    style = self.buffered_style;
                    if self.use_unicode && pos_f64 + 1.0 > buffered_pos {
                        let frac = buffered_pos - pos_f64;
                        symbol = get_unicode_block(frac);
                    }
                } else {
                    symbol = if self.use_unicode {
                        " "
                    } else {
                        symbols::block::FULL
                    };
                }

                if x >= label_col && x < label_col + label.width() as u16 && y == label_row {
                    symbol = " ";
                    style = style.bg(style.fg.unwrap_or_default());
                }

                buf[(x, y)]
                    .set_symbol(symbol)
                    .set_fg(style.fg.unwrap_or_default())
                    .set_bg(style.bg.unwrap_or_default());
            }
        }

        buf.set_span(label_col, label_row, label, label.width() as u16);
    }
}
