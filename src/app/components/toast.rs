use std::time::Instant;

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::framework::{signals::Signal, theme::ThemeStyles};

const TOAST_DURATION: f32 = 2.0;
const SLIDE_IN_DURATION: f32 = 0.4;
const SLIDE_OUT_DURATION: f32 = 0.3;
const REPLACE_OUT_DURATION: f32 = 0.2;

#[derive(Debug, Clone, PartialEq)]
enum ToastPhase {
    SlideIn { started: Instant },
    Visible { dismiss_at: Instant },
    SlideOutRight { started: Instant },
    FadeOutDown { started: Instant },
}

#[derive(Debug, Clone)]
struct ToastEntry {
    message: Vec<Line<'static>>,
    icon: Option<String>,
    phase: ToastPhase,
}

pub struct ToastManager {
    current: Option<ToastEntry>,
    outgoing: Option<ToastEntry>,
    theme: Signal<ThemeStyles>,
    is_animating: Signal<bool>,
}

impl ToastManager {
    pub fn new(theme: Signal<ThemeStyles>) -> Self {
        Self {
            current: None,
            outgoing: None,
            theme,
            is_animating: Signal::new(false),
        }
    }

    pub fn is_animating(&self) -> Signal<bool> {
        self.is_animating.clone()
    }

    pub fn push(&mut self, message: String) {
        self.push_with_icon(message, None);
    }

    pub fn push_with_icon(&mut self, message: String, icon: Option<String>) {
        self.push_lines(
            vec![Line::from(vec![Span::styled(
                message,
                Style::default().add_modifier(Modifier::BOLD),
            )])],
            icon,
        );
    }

    pub fn push_line(&mut self, message: Line<'static>, icon: Option<String>) {
        self.push_lines(vec![message], icon);
    }

    pub fn push_lines(&mut self, message: Vec<Line<'static>>, icon: Option<String>) {
        let now = Instant::now();
        self.is_animating.set(true);

        if let Some(mut old) = self.current.take() {
            old.phase = ToastPhase::FadeOutDown { started: now };
            self.outgoing = Some(old);
        }

        self.current = Some(ToastEntry {
            message,
            icon,
            phase: ToastPhase::SlideIn { started: now },
        });
    }

    pub fn is_active(&self) -> bool {
        self.current.is_some() || self.outgoing.is_some()
    }

    pub fn view(&mut self, frame: &mut Frame, area: Rect) {
        let now = Instant::now();
        let styles = self.theme.get();

        self.tick_phases(now);

        if let Some(entry) = &self.outgoing
            && let ToastPhase::FadeOutDown { started } = entry.phase
        {
            let elapsed = now.duration_since(started).as_secs_f32();
            let t = (elapsed / REPLACE_OUT_DURATION).min(1.0);
            let opacity = 1.0 - ease_in_quad(t);
            let y_offset = (ease_in_quad(t) * 2.0) as i16;
            self.render_toast(frame, area, entry, opacity, 0, y_offset, &styles);
        }

        if let Some(entry) = &self.current {
            let (opacity, x_offset, y_offset) = match entry.phase {
                ToastPhase::SlideIn { started } => {
                    let elapsed = now.duration_since(started).as_secs_f32();
                    let t = (elapsed / SLIDE_IN_DURATION).min(1.0);
                    let eased = ease_out_cubic(t);
                    let x_off = ((1.0 - eased) * 30.0) as i16;
                    (eased, x_off, 0)
                }
                ToastPhase::Visible { .. } => (1.0, 0, 0),
                ToastPhase::SlideOutRight { started } => {
                    let elapsed = now.duration_since(started).as_secs_f32();
                    let t = (elapsed / SLIDE_OUT_DURATION).min(1.0);
                    let eased = ease_in_quad(t);
                    let x_off = (eased * 30.0) as i16;
                    (1.0 - eased, x_off, 0)
                }
                ToastPhase::FadeOutDown { .. } => (0.0, 0, 0),
            };
            if opacity > 0.01 {
                self.render_toast(frame, area, entry, opacity, x_offset, y_offset, &styles);
            }
        }
    }

    fn tick_phases(&mut self, now: Instant) {
        if let Some(entry) = &mut self.current {
            match entry.phase {
                ToastPhase::SlideIn { started } => {
                    let elapsed = now.duration_since(started).as_secs_f32();
                    if elapsed >= SLIDE_IN_DURATION {
                        entry.phase = ToastPhase::Visible {
                            dismiss_at: now + std::time::Duration::from_secs_f32(TOAST_DURATION),
                        };
                    }
                }
                ToastPhase::Visible { dismiss_at } => {
                    if now >= dismiss_at {
                        entry.phase = ToastPhase::SlideOutRight { started: now };
                    }
                }
                ToastPhase::SlideOutRight { started } => {
                    let elapsed = now.duration_since(started).as_secs_f32();
                    if elapsed >= SLIDE_OUT_DURATION {
                        self.current = None;
                    }
                }
                _ => {}
            }
        }

        if let Some(entry) = &self.outgoing
            && let ToastPhase::FadeOutDown { started } = entry.phase
        {
            let elapsed = now.duration_since(started).as_secs_f32();
            if elapsed >= REPLACE_OUT_DURATION {
                self.outgoing = None;
            }
        }

        if self.current.is_none() && self.outgoing.is_none() {
            self.is_animating.set(false);
        }
    }

    fn render_toast(
        &self,
        frame: &mut Frame,
        area: Rect,
        entry: &ToastEntry,
        opacity: f32,
        x_offset: i16,
        y_offset: i16,
        styles: &ThemeStyles,
    ) {
        let mut msg_lines = entry.message.clone();

        if let Some(icon) = &entry.icon
            && let Some(first_line) = msg_lines.first_mut()
        {
            first_line.spans.insert(
                0,
                Span::styled(
                    format!("{} ", icon),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            );
        }

        let content_len = msg_lines.iter().map(|l| l.width()).max().unwrap_or(0) as u16;
        let toast_width = (content_len + 8).max(28).min(area.width.saturating_sub(4));
        let toast_height = (msg_lines.len() as u16 + 2).min(area.height.saturating_sub(2));

        let right_margin = 2;
        let base_x = area.right().saturating_sub(toast_width + right_margin);
        let x = (base_x as i16 + x_offset).max(0) as u16;

        let y = (area.top() as i16 + 1 + y_offset).max(0) as u16;

        let toast_area = Rect::new(x, y, toast_width, toast_height).intersection(area);

        if toast_area.width == 0 || toast_area.height == 0 {
            return;
        }

        let accent_fg = styles.accent.fg.unwrap_or(Color::Yellow);
        let bg = styles.text.bg.unwrap_or(Color::Black);
        let border_color = blend_color(accent_fg, bg, opacity);
        let _text_color = blend_color(styles.text.fg.unwrap_or(Color::White), bg, opacity);

        frame.render_widget(Clear, toast_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color).bg(bg))
            .style(Style::default().bg(bg));

        for line in &mut msg_lines {
            for span in &mut line.spans {
                let base_fg = span
                    .style
                    .fg
                    .unwrap_or(styles.text.fg.unwrap_or(Color::White));
                span.style = span.style.fg(blend_color(base_fg, bg, opacity));
            }
        }

        let paragraph = Paragraph::new(msg_lines)
            .block(block)
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, toast_area);
    }
}

fn ease_in_quad(t: f32) -> f32 {
    t * t
}

fn ease_out_cubic(t: f32) -> f32 {
    let t = t - 1.0;
    t * t * t + 1.0
}

fn blend_color(fg: Color, bg: Color, opacity: f32) -> Color {
    let (fr, fg_g, fb) = color_to_rgb(fg);
    let (br, bg_g, bb) = color_to_rgb(bg);

    let r = (fr as f32 * opacity + br as f32 * (1.0 - opacity)) as u8;
    let g = (fg_g as f32 * opacity + bg_g as f32 * (1.0 - opacity)) as u8;
    let b = (fb as f32 * opacity + bb as f32 * (1.0 - opacity)) as u8;

    Color::Rgb(r, g, b)
}

fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Yellow => (247, 212, 75),
        Color::White => (255, 255, 255),
        Color::Black => (0, 0, 0),
        Color::Red => (255, 0, 0),
        Color::Green => (0, 255, 0),
        Color::Blue => (0, 0, 255),
        _ => (200, 200, 200),
    }
}
