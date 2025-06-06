use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Style, Stylize},
    symbols::{self, border},
    text::{Line, ToSpan},
    widgets::{Block, Borders, Gauge, Paragraph, Widget},
};

use crate::{audio::enums::RepeatMode, util::colors};

pub struct PlayerControlsWidget {
    repeat_mode: RepeatMode,
    shuffle_mode: bool,
    volume: u8,
}

impl PlayerControlsWidget {
    pub fn new(
        repeat_mode: RepeatMode,
        shuffle_mode: bool,
        volume: u8,
    ) -> Self {
        Self {
            repeat_mode,
            shuffle_mode,
            volume,
        }
    }
}

impl Widget for PlayerControlsWidget {
    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) where
        Self: Sized,
    {
        let repeat_icon = match self.repeat_mode {
            RepeatMode::None => "󰑗".fg(colors::NEUTRAL),
            RepeatMode::Single => "󰑘".fg(colors::PRIMARY),
            RepeatMode::All => "󰑖".fg(colors::PRIMARY),
        };
        let shuffle_icon = if self.shuffle_mode {
            "󰒟".fg(colors::PRIMARY)
        } else {
            "󰒞".fg(colors::NEUTRAL)
        };

        let mut controls_text = Line::default();
        controls_text.push_span(repeat_icon);
        controls_text.push_span("  ");
        controls_text.push_span(shuffle_icon);

        let volume_text = format!("{}%", self.volume);
        let mut volume_text = volume_text.to_span();

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(7), Constraint::Length(12)])
            .split(area);

        let controls_block = Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
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
                colors::PRIMARY,
                colors::NEUTRAL,
            )
        } else {
            (
                (self.volume - 100) as f64 / 100.0,
                Some(colors::NEUTRAL),
                colors::SECONDARY,
                colors::PRIMARY,
            )
        };

        if let Some(fg) = volume_fg {
            volume_text = volume_text.fg(fg);
        }

        let volume_block =
            Block::default()
                .borders(Borders::ALL)
                .border_set(border::Set {
                    top_right: symbols::line::ROUNDED.vertical_left,
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
