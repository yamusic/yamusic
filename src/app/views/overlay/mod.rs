pub mod fx;
pub mod theme_picker;

use crate::{
    app::theme::theme,
    app::{actions::Route, components::Lyrics},
};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph},
};
pub use theme_picker::ThemePicker;

pub use fx::EffectsOverlay;

pub struct OverlayRenderer;

impl OverlayRenderer {
    pub fn render(
        frame: &mut Frame,
        content_area: Rect,
        route: &Route,
        lyrics: &mut Lyrics,
        effects: &mut EffectsOverlay,
        theme_picker: &mut ThemePicker,
    ) {
        let colors = theme();
        let text_style = ratatui::style::Style::default()
            .fg(colors.text.primary)
            .bg(colors.bg.base);
        let border_focused = colors.focused_border;

        frame.render_widget(Clear, content_area);
        frame.buffer_mut().set_style(content_area, text_style);

        match route {
            Route::Lyrics => {
                lyrics.view(frame, content_area);
            }
            Route::Effects => {
                effects.view(frame, content_area);
            }
            Route::ThemePicker => {
                theme_picker.view(frame, content_area);
            }
            _ => {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_focused)
                    .style(text_style)
                    .title(format!(" {} ", route.title()));
                let inner = block.inner(content_area);
                frame.render_widget(block, content_area);

                let paragraph = Paragraph::new(format!("Overlay view: {}", route.title()));
                frame.render_widget(paragraph, inner);
            }
        }
    }
}
