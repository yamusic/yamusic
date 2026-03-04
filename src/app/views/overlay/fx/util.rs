use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub const ARC_ANGLE_START: f64 = std::f64::consts::PI * 0.87;
pub const ARC_ANGLE_END: f64 = std::f64::consts::PI * 0.13;
pub const ASPECT_RATIO: f64 = 2.1;

pub fn map_to_width(value: f32, min: f32, max: f32, width: usize) -> usize {
    if width == 0 || max <= min {
        return 0;
    }
    let t = ((value - min) / (max - min)).clamp(0.0, 1.0);
    (t * (width.saturating_sub(1)) as f32).round() as usize
}

pub fn render_cell_line(frame: &mut Frame, area: Rect, row: usize, cells: &[(char, Color)]) {
    let spans: Vec<Span> = cells
        .iter()
        .map(|&(ch, color)| Span::styled(String::from(ch), Style::default().fg(color)))
        .collect();
    frame.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect {
            x: area.x,
            y: area.y + row as u16,
            width: area.width,
            height: 1,
        },
    );
}

pub fn edge_fade_curve(progress: f32, fade_amount: f32) -> f32 {
    if progress < fade_amount {
        (progress / fade_amount).powf(1.5)
    } else if progress > 1.0 - fade_amount {
        ((1.0 - progress) / fade_amount).powf(1.5)
    } else {
        1.0
    }
}

pub fn normalize_q(q: f32, min_q: f32, max_q: f32) -> f32 {
    if max_q <= min_q {
        return 0.5;
    }
    let log_q = q.ln();
    let log_min = min_q.ln();
    let log_max = max_q.ln();
    ((log_q - log_min) / (log_max - log_min)).clamp(0.0, 1.0)
}
