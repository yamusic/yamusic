use crate::{
    app::{actions::Route, components::Lyrics},
    framework::{signals::Signal, theme::ThemeStyles},
};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct OverlayRenderer;

impl OverlayRenderer {
    pub fn render(
        frame: &mut Frame,
        content_area: Rect,
        route: &Route,
        theme: &Signal<ThemeStyles>,
        lyrics: &mut Lyrics,
    ) {
        match route {
            Route::Lyrics => {
                let styles = theme.get();
                frame.render_widget(Clear, content_area);
                frame.buffer_mut().set_style(content_area, styles.text);
                lyrics.view(frame, content_area);
            }
            _ => {
                let styles = theme.get();
                frame.render_widget(Clear, content_area);
                frame.buffer_mut().set_style(content_area, styles.text);

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(styles.block_focused)
                    .style(styles.text)
                    .title(format!(" {} ", route.title()));
                let inner = block.inner(content_area);
                frame.render_widget(block, content_area);

                let paragraph = Paragraph::new(format!("Overlay view: {}", route.title()));
                frame.render_widget(paragraph, inner);
            }
        }
    }
}
