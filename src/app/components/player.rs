use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    symbols::{self, border},
    text::{Line, Span, ToSpan},
    widgets::{Block, Borders, Gauge, Paragraph, Widget},
};

use crate::{
    audio::enums::RepeatMode,
    framework::{signals::Signal, theme::ThemeStyles},
};

pub struct PlayerSignals {
    pub track_title: Signal<Option<String>>,
    pub track_artists: Signal<Option<String>>,
    pub is_playing: Signal<bool>,
    pub is_liked: Signal<bool>,
    pub is_disliked: Signal<bool>,
    pub position_ms: Signal<u64>,
    pub duration_ms: Signal<u64>,
    pub buffered_ratio: Signal<f32>,
    pub volume: Signal<u8>,
    pub is_muted: Signal<bool>,
    pub is_shuffled: Signal<bool>,
    pub repeat_mode: Signal<RepeatMode>,
}

impl PlayerSignals {
    pub fn new() -> Self {
        Self {
            track_title: Signal::new(None),
            track_artists: Signal::new(None),
            is_playing: Signal::new(false),
            is_liked: Signal::new(false),
            is_disliked: Signal::new(false),
            position_ms: Signal::new(0),
            duration_ms: Signal::new(0),
            buffered_ratio: Signal::new(0.0),
            volume: Signal::new(50),
            is_muted: Signal::new(false),
            is_shuffled: Signal::new(false),
            repeat_mode: Signal::new(RepeatMode::None),
        }
    }
}

impl Default for PlayerSignals {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PlayerBar {
    signals: PlayerSignals,
    theme: Signal<ThemeStyles>,
}

impl PlayerBar {
    pub fn new(signals: PlayerSignals, theme: Signal<ThemeStyles>) -> Self {
        Self { signals, theme }
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        let styles = self.theme.get();

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(10), Constraint::Length(19)])
            .split(area);

        let progress_widget = ProgressWidget {
            position_ms: self.signals.position_ms.get(),
            duration_ms: self.signals.duration_ms.get(),
            buffered_ratio: self.signals.buffered_ratio.get(),
            track_title: self.signals.track_title.get(),
            track_artist: self.signals.track_artists.get(),
            is_playing: self.signals.is_playing.get(),
            styles: styles.clone(),
        };
        frame.render_widget(progress_widget, layout[0]);

        let controls_widget = ControlsWidget {
            repeat_mode: self.signals.repeat_mode.get(),
            shuffle_mode: self.signals.is_shuffled.get(),
            is_liked: self.signals.is_liked.get(),
            is_disliked: self.signals.is_disliked.get(),
            volume: self.signals.volume.get(),
            styles,
        };
        frame.render_widget(controls_widget, layout[1]);
    }
}

struct ProgressWidget {
    position_ms: u64,
    duration_ms: u64,
    buffered_ratio: f32,
    track_title: Option<String>,
    track_artist: Option<String>,
    is_playing: bool,
    styles: ThemeStyles,
}

impl Widget for ProgressWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (current, total) = (self.position_ms, self.duration_ms);
        let percent = if total > 0 {
            current as f64 / total as f64
        } else {
            0.0
        };

        let play_icon = if self.is_playing { "" } else { "" };
        let mut track_info = format!(
            "{}  {}",
            play_icon,
            self.track_title.as_deref().unwrap_or("No track"),
        );
        if let Some(artist) = self.track_artist {
            track_info = format!("{track_info} by {artist}");
        }

        let duration_info = format!("{} / {}", format_duration(current), format_duration(total));

        let buffered_ratio = self.buffered_ratio as f64;

        let gauge = CustomGauge::default()
            .block(
                Block::default()
                    .title_top(
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            track_info,
                            self.styles.text,
                        ))
                        .alignment(Alignment::Center),
                    )
                    .borders(Borders::ALL)
                    .border_style(self.styles.block_focused)
                    .border_set(border::Set {
                        top_right: symbols::line::ROUNDED.horizontal_down,
                        bottom_right: symbols::line::ROUNDED.horizontal_up,
                        ..symbols::border::ROUNDED
                    }),
            )
            .ratios(percent.min(1.0), buffered_ratio.min(1.0))
            .label(
                duration_info
                    .to_span()
                    .fg(self.styles.text.fg.unwrap_or_default()),
            )
            .played_style(self.styles.progress_fg)
            .buffered_style(self.styles.progress_bg)
            .remaining_style(
                Style::default()
                    .fg(self.styles.text.bg.unwrap_or_default())
                    .bg(self.styles.text.bg.unwrap_or_default()),
            )
            .use_unicode(true);

        gauge.render(area, buf);
    }
}

struct ControlsWidget {
    repeat_mode: RepeatMode,
    shuffle_mode: bool,
    is_liked: bool,
    is_disliked: bool,
    volume: u8,
    styles: ThemeStyles,
}

impl Widget for ControlsWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let repeat_icon = match self.repeat_mode {
            RepeatMode::None => "󰑗".fg(self.styles.text_muted.fg.unwrap_or_default()),
            RepeatMode::Single => "󰑘".fg(self.styles.accent.fg.unwrap_or_default()),
            RepeatMode::All => "󰑖".fg(self.styles.accent.fg.unwrap_or_default()),
        };
        let shuffle_icon = if self.shuffle_mode {
            "󰒟".fg(self.styles.accent.fg.unwrap_or_default())
        } else {
            "󰒞".fg(self.styles.text_muted.fg.unwrap_or_default())
        };

        let heart_icon = if self.is_liked {
            "󰋑".fg(self.styles.accent.fg.unwrap_or_default())
        } else if self.is_disliked {
            "󰋖".fg(self.styles.text_muted.fg.unwrap_or_default())
        } else {
            "󰋕".fg(self.styles.text_muted.fg.unwrap_or_default())
        };

        let mut controls_text = Line::default();
        controls_text.push_span(heart_icon);
        controls_text.push_span("  ");
        controls_text.push_span(repeat_icon);
        controls_text.push_span("  ");
        controls_text.push_span(shuffle_icon);

        let volume_text = format!("{}%", self.volume);
        let mut volume_text = volume_text.to_span();

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(9), Constraint::Length(12)])
            .split(area);

        let controls_block = Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .border_style(self.styles.block_focused)
            .border_set(border::Set {
                top_left: symbols::line::ROUNDED.horizontal_down,
                top_right: symbols::line::ROUNDED.horizontal_down,
                bottom_left: symbols::line::ROUNDED.horizontal_up,
                bottom_right: symbols::line::ROUNDED.horizontal_up,
                ..symbols::border::ROUNDED
            });
        let controls = Paragraph::new(controls_text)
            .block(controls_block)
            .centered();
        controls.render(layout[0], buf);

        let (volume, volume_fg, fg, bg) = if self.volume <= 100 {
            (
                self.volume as f64 / 100.0,
                None,
                self.styles.accent.fg.unwrap_or_default(),
                self.styles.text.bg.unwrap_or_default(),
            )
        } else {
            (
                (self.volume - 100) as f64 / 100.0,
                Some(self.styles.text_muted.fg.unwrap_or_default()),
                self.styles.progress_fg.fg.unwrap_or_default(),
                self.styles.progress_fg.fg.unwrap_or_default(),
            )
        };

        if let Some(fg) = volume_fg {
            volume_text = volume_text.fg(fg);
        }

        let volume_block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.styles.block_focused)
            .border_set(border::Set {
                top_left: symbols::line::ROUNDED.horizontal_down,
                bottom_left: symbols::line::ROUNDED.horizontal_up,
                ..symbols::border::ROUNDED
            });

        let volume_gauge = Gauge::default()
            .block(volume_block)
            .gauge_style(Style::new().fg(fg).bg(bg))
            .ratio(volume)
            .label(volume_text);

        volume_gauge.render(layout[1], buf);
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
struct CustomGauge<'a> {
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
    fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    fn ratios(mut self, played: f64, buffered: f64) -> Self {
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

    fn label<T>(mut self, label: T) -> Self
    where
        T: Into<Span<'a>>,
    {
        self.label = Some(label.into());
        self
    }

    const fn use_unicode(mut self, use_unicode: bool) -> Self {
        self.use_unicode = use_unicode;
        self
    }

    fn played_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.played_style = style.into();
        self
    }

    fn buffered_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.buffered_style = style.into();
        self
    }

    fn remaining_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.remaining_style = style.into();
        self
    }

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
                        symbol = unicode_block(frac);
                    }
                } else if pos_f64 < buffered_pos {
                    style = self.buffered_style;
                    if self.use_unicode && pos_f64 + 1.0 > buffered_pos {
                        let frac = buffered_pos - pos_f64;
                        symbol = unicode_block(frac);
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

fn unicode_block(frac: f64) -> &'static str {
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

fn format_duration(duration: u64) -> String {
    let total_seconds = duration / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}
