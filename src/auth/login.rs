use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::framework::theme::{ThemeColor, global_theme};

use super::token::TokenProvider;

const LOGO: &str = r#"
                              _       
                             (_)      
 _   _  ____ ____  _   _  ___ _  ____ 
| | | |/ _  |    \| | | |/___) |/ ___)
| |_| ( ( | | | | | |_| |___ | ( (___ 
 \__  |\_||_|_|_|_|\____(___/|_|\____)
(____/                                "#;

struct LoginColors {
    accent: Color,
    accent_rgb: (u8, u8, u8),
    accent_dim: Color,
    bg: Color,
    fg: Color,
    fg_muted: Color,
    error: Color,
    success: Color,
}

impl LoginColors {
    fn from_theme() -> Self {
        let config = global_theme().get_config();
        let accent_tc = config.accent;
        let accent = accent_tc.to_ratatui();
        let accent_rgb = match accent_tc {
            ThemeColor::Rgb(r, g, b) => (r, g, b),
            _ => (247, 212, 75),
        };
        Self {
            accent,
            accent_rgb,
            accent_dim: accent_tc.darken(0.3).to_ratatui(),
            bg: config.background.to_ratatui(),
            fg: config.foreground.to_ratatui(),
            fg_muted: config.muted.to_ratatui(),
            error: config.error.to_ratatui(),
            success: config.success.to_ratatui(),
        }
    }
}

#[derive(PartialEq, Eq)]
enum Focus {
    TokenInput,
    ShowButton,
    SubmitButton,
}

pub struct LoginScreen {
    token_input: String,
    cursor_pos: usize,
    show_token: bool,
    error_message: Option<String>,
    status_message: Option<String>,
    focus: Focus,
    frame_count: usize,
}

impl Default for LoginScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginScreen {
    pub fn new() -> Self {
        Self {
            token_input: String::new(),
            cursor_pos: 0,
            show_token: false,
            error_message: None,
            status_message: None,
            focus: Focus::TokenInput,
            frame_count: 0,
        }
    }

    pub async fn run(
        &mut self,
    ) -> color_eyre::Result<Option<(std::sync::Arc<yandex_music::YandexMusicClient>, u64)>> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::cursor::Hide
        )?;

        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = ratatui::Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal).await;

        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        )?;

        result
    }

    async fn event_loop(
        &mut self,
        terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>,
    ) -> color_eyre::Result<Option<(std::sync::Arc<yandex_music::YandexMusicClient>, u64)>> {
        loop {
            self.frame_count = self.frame_count.wrapping_add(1);
            terminal.draw(|f| self.view(f))?;

            if event::poll(std::time::Duration::from_millis(80))?
                && let Event::Key(key) = event::read()?
            {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return Ok(None);
                }

                match self.handle_key(key) {
                    KeyAction::None => {}
                    KeyAction::Submit => {
                        let token = self.token_input.trim().to_string();
                        if token.is_empty() {
                            self.error_message = Some("Token cannot be empty".into());
                            continue;
                        }
                        self.error_message = None;
                        self.status_message = Some("Validating token…".into());
                        terminal.draw(|f| self.view(f))?;

                        match TokenProvider::validate(token.clone()).await {
                            Ok((client, user_id)) => {
                                self.status_message = Some("Saving token…".into());
                                terminal.draw(|f| self.view(f))?;

                                match TokenProvider::store(&token) {
                                    Ok(()) => {
                                        self.status_message = Some("Token saved ✓".into());
                                        terminal.draw(|f| self.view(f))?;
                                        std::thread::sleep(std::time::Duration::from_millis(400));
                                        return Ok(Some((client, user_id)));
                                    }
                                    Err(e) => {
                                        self.status_message = None;
                                        self.error_message = Some(format!("Failed to save: {e}"));
                                    }
                                }
                            }
                            Err(_) => {
                                self.status_message = None;
                                self.error_message = Some("Invalid token".to_string());
                            }
                        }
                    }
                    KeyAction::Quit => return Ok(None),
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        match self.focus {
            Focus::TokenInput => match key.code {
                KeyCode::Char(c) => {
                    self.token_input.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;
                    self.error_message = None;
                    KeyAction::None
                }
                KeyCode::Backspace => {
                    if self.cursor_pos > 0 {
                        self.cursor_pos -= 1;
                        self.token_input.remove(self.cursor_pos);
                    }
                    KeyAction::None
                }
                KeyCode::Delete => {
                    if self.cursor_pos < self.token_input.len() {
                        self.token_input.remove(self.cursor_pos);
                    }
                    KeyAction::None
                }
                KeyCode::Left => {
                    self.cursor_pos = self.cursor_pos.saturating_sub(1);
                    KeyAction::None
                }
                KeyCode::Right => {
                    if self.cursor_pos < self.token_input.len() {
                        self.cursor_pos += 1;
                    }
                    KeyAction::None
                }
                KeyCode::Home => {
                    self.cursor_pos = 0;
                    KeyAction::None
                }
                KeyCode::End => {
                    self.cursor_pos = self.token_input.len();
                    KeyAction::None
                }
                KeyCode::Tab | KeyCode::Down => {
                    self.focus = Focus::ShowButton;
                    KeyAction::None
                }
                KeyCode::BackTab | KeyCode::Up => {
                    self.focus = Focus::SubmitButton;
                    KeyAction::None
                }
                KeyCode::Enter => KeyAction::Submit,
                KeyCode::Esc => KeyAction::Quit,
                _ => KeyAction::None,
            },
            Focus::ShowButton => match key.code {
                KeyCode::Tab | KeyCode::Down | KeyCode::Right => {
                    self.focus = Focus::SubmitButton;
                    KeyAction::None
                }
                KeyCode::Up | KeyCode::BackTab | KeyCode::Left => {
                    self.focus = Focus::TokenInput;
                    KeyAction::None
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.show_token = !self.show_token;
                    self.error_message = None;
                    KeyAction::None
                }
                KeyCode::Esc => KeyAction::Quit,
                _ => KeyAction::None,
            },
            Focus::SubmitButton => match key.code {
                KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                    self.focus = Focus::TokenInput;
                    KeyAction::None
                }
                KeyCode::Up | KeyCode::Left | KeyCode::BackTab => {
                    self.focus = Focus::ShowButton;
                    KeyAction::None
                }
                KeyCode::Enter | KeyCode::Char(' ') => KeyAction::Submit,
                KeyCode::Esc => KeyAction::Quit,
                KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.focus = Focus::TokenInput;
                    KeyAction::None
                }
                _ => KeyAction::None,
            },
        }
    }

    fn view(&self, frame: &mut Frame) {
        let area = frame.area();
        let c = LoginColors::from_theme();

        frame.render_widget(Clear, area);
        frame.render_widget(Block::default().style(Style::default().bg(c.bg)), area);

        let card_height = 20u16;
        let card_width = 68u16.min(area.width.saturating_sub(4));

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(card_height),
                Constraint::Min(0),
            ])
            .split(area);

        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(card_width),
                Constraint::Min(0),
            ])
            .split(vertical[1]);

        let card_area = horizontal[1];

        let inner = card_area.inner(Margin::new(2, 1));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner);

        self.render_logo(frame, chunks[0], &c);

        let label = Paragraph::new(Line::from(vec![Span::styled(
            "  OAuth Token",
            Style::default().fg(c.fg).add_modifier(Modifier::BOLD),
        )]));
        frame.render_widget(label, chunks[3]);

        self.render_input(frame, chunks[4], &c);

        self.render_status(frame, chunks[5], &c);

        self.render_button(frame, chunks[6], &c);

        if chunks.len() > 8 {
            self.render_hints(frame, chunks[8], &c);
        }
    }

    fn render_logo(&self, frame: &mut Frame, area: Rect, c: &LoginColors) {
        let time = self.frame_count as f32 * 0.5;
        let (ar, ag, ab) = c.accent_rgb;
        let logo_lines: Vec<Line> = LOGO
            .lines()
            .enumerate()
            .map(|(i, line)| {
                let chars: Vec<Span> = line
                    .chars()
                    .enumerate()
                    .map(|(j, ch)| {
                        let wave_pos = (j as f32 * 0.3) + (i as f32 * 0.5) - time;
                        let wave = (wave_pos * 0.5).sin();

                        let wave2 = ((wave_pos * 0.8) + std::f32::consts::PI).cos();
                        let combined = (wave + wave2 * 0.3) * 0.5;

                        let brightness = 0.5 + 0.5 * combined;
                        let brightness = brightness.clamp(0.3, 1.0);

                        let r = (ar as f32 * brightness) as u8;
                        let g = (ag as f32 * brightness) as u8;
                        let b = (ab as f32 * brightness) as u8;
                        Span::styled(
                            ch.to_string(),
                            Style::default()
                                .fg(Color::Rgb(r, g, b))
                                .add_modifier(Modifier::BOLD),
                        )
                    })
                    .collect();
                Line::from(chars)
            })
            .collect();

        let logo = Paragraph::new(logo_lines).alignment(Alignment::Center);
        frame.render_widget(logo, area);
    }

    fn render_input(&self, frame: &mut Frame, area: Rect, c: &LoginColors) {
        let is_focused = self.focus == Focus::TokenInput;
        let border_color = if is_focused { c.accent } else { c.fg_muted };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(if is_focused {
                BorderType::Thick
            } else {
                BorderType::Rounded
            })
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(c.bg));

        let display_text = if self.token_input.is_empty() {
            if is_focused {
                String::new()
            } else {
                "Paste your OAuth token here…".into()
            }
        } else if self.show_token {
            self.token_input.clone()
        } else {
            "•".repeat(self.token_input.chars().count())
        };

        let style = if self.token_input.is_empty() && !is_focused {
            Style::default()
                .fg(c.fg_muted)
                .add_modifier(Modifier::ITALIC)
        } else {
            Style::default().fg(c.fg)
        };

        let input = Paragraph::new(Span::styled(display_text, style)).block(block);

        frame.render_widget(input, area);

        if is_focused {
            let cursor_x =
                area.x + 1 + self.cursor_pos.min(area.width.saturating_sub(3) as usize) as u16;
            let cursor_y = area.y + 1;
            if cursor_x < area.x + area.width - 1 {
                let blink = (self.frame_count / 5).is_multiple_of(2);
                if blink {
                    let cursor_block =
                        Paragraph::new(Span::styled("▎", Style::default().fg(c.accent)));
                    frame.render_widget(cursor_block, Rect::new(cursor_x, cursor_y, 1, 1));
                }
            }
        }
    }

    fn render_status(&self, frame: &mut Frame, area: Rect, c: &LoginColors) {
        if let Some(err) = &self.error_message {
            let msg = Paragraph::new(Line::from(vec![
                Span::styled("  ✘ ", Style::default().fg(c.error)),
                Span::styled(err.clone(), Style::default().fg(c.error)),
            ]));
            frame.render_widget(msg, area);
        } else if let Some(status) = &self.status_message {
            let msg = Paragraph::new(Line::from(vec![
                Span::styled("  ● ", Style::default().fg(c.success)),
                Span::styled(status.clone(), Style::default().fg(c.success)),
            ]));
            frame.render_widget(msg, area);
        }
    }

    fn render_button(&self, frame: &mut Frame, area: Rect, c: &LoginColors) {
        let spacing = 2u16;
        let max_btn_width = 20u16;

        let btn_width = ((area.width.saturating_sub(spacing)) / 2).min(max_btn_width);

        let total_width = btn_width.saturating_mul(2).saturating_add(spacing);

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(total_width),
                Constraint::Min(0),
            ])
            .split(area);

        let middle = cols[1];

        let parts = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(btn_width),
                Constraint::Length(spacing),
                Constraint::Length(btn_width),
            ])
            .split(middle);

        let show_area = parts[0];
        let sign_area = parts[2];
        let is_show_focused = self.focus == Focus::ShowButton;
        let (sfg, sbg, smod) = if is_show_focused {
            (c.bg, c.accent, Modifier::BOLD)
        } else {
            (c.fg, c.bg, Modifier::empty())
        };

        let show_label = if self.show_token {
            "Hide token"
        } else {
            "Show token"
        };
        let show_btn = Paragraph::new(Line::from(vec![Span::styled(
            format!(" {} ", show_label),
            Style::default().fg(sfg).bg(sbg).add_modifier(smod),
        )]))
        .alignment(Alignment::Center);
        frame.render_widget(show_btn, show_area);

        let is_submit_focused = self.focus == Focus::SubmitButton;
        let (fg, bg, modifier) = if is_submit_focused {
            (c.bg, c.accent, Modifier::BOLD)
        } else {
            (c.fg, c.bg, Modifier::empty())
        };

        let btn = Paragraph::new(Line::from(vec![Span::styled(
            " Sign In → ",
            Style::default().fg(fg).bg(bg).add_modifier(modifier),
        )]))
        .alignment(Alignment::Center);

        frame.render_widget(btn, sign_area);
    }

    fn render_hints(&self, frame: &mut Frame, area: Rect, c: &LoginColors) {
        let parts = vec![
            (
                "  Tab",
                Some(
                    Style::default()
                        .fg(c.accent_dim)
                        .add_modifier(Modifier::BOLD),
                ),
            ),
            (" navigate  ", Some(Style::default().fg(c.fg_muted))),
            (
                "Enter",
                Some(
                    Style::default()
                        .fg(c.accent_dim)
                        .add_modifier(Modifier::BOLD),
                ),
            ),
            (" submit  ", Some(Style::default().fg(c.fg_muted))),
            (
                "Esc",
                Some(
                    Style::default()
                        .fg(c.accent_dim)
                        .add_modifier(Modifier::BOLD),
                ),
            ),
            (" quit", Some(Style::default().fg(c.fg_muted))),
        ];

        let hint_text: String = parts
            .iter()
            .map(|(s, _)| *s)
            .collect::<Vec<&str>>()
            .join("");
        let hint_width = hint_text.chars().count().min(area.width as usize) as u16;

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(hint_width),
                Constraint::Min(0),
            ])
            .split(area);

        let middle = cols[1];

        let spans: Vec<Span> = parts
            .into_iter()
            .map(|(s, style)| match style {
                Some(st) => Span::styled(s, st),
                None => Span::raw(s),
            })
            .collect();

        let p = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
        frame.render_widget(p, middle);
    }
}

enum KeyAction {
    None,
    Submit,
    Quit,
}
