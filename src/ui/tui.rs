use std::{
    ops::{Deref, DerefMut},
    thread,
    time::Duration,
};

use color_eyre::eyre::Result;

use flume::{Receiver, Sender};
use ratatui::crossterm::{
    cursor,
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste,
        EnableMouseCapture, Event as CrosstermEvent, KeyEvent, KeyEventKind,
        MouseEvent,
    },
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend as Backend, crossterm};

#[derive(Clone, Debug)]
pub enum TerminalEvent {
    Init,
    Quit,
    Error,
    Closed,
    Tick,
    FocusGained,
    FocusLost,
    Paste(String),
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
}

pub struct Tui {
    pub terminal: ratatui::Terminal<Backend<std::io::Stdout>>,
    pub event_rx: Receiver<TerminalEvent>,
    pub event_tx: Sender<TerminalEvent>,
    pub mouse: bool,
    pub paste: bool,
}

impl Tui {
    pub fn new() -> Result<Self> {
        let terminal = ratatui::Terminal::new(Backend::new(std::io::stdout()))?;
        let (event_tx, event_rx) = flume::unbounded();
        let mouse = false;
        let paste = false;
        Ok(Self {
            terminal,
            event_rx,
            event_tx,
            mouse,
            paste,
        })
    }

    pub fn mouse(mut self, mouse: bool) -> Self {
        self.mouse = mouse;
        self
    }

    pub fn paste(mut self, paste: bool) -> Self {
        self.paste = paste;
        self
    }

    pub fn start(&mut self) {
        let event_tx = self.event_tx.clone();
        thread::spawn(move || {
            event_tx.send(TerminalEvent::Init).unwrap();
            loop {
                let _ = event_tx.send(TerminalEvent::Tick);
                if !event::poll(Duration::from_millis(16)).unwrap() {
                    continue;
                }
                let crossterm_event = event::read();
                match crossterm_event {
                    Ok(evt) => match evt {
                        CrosstermEvent::Key(key) => {
                            if key.kind == KeyEventKind::Press {
                                let _ = event_tx.send(TerminalEvent::Key(key));
                            }
                        }
                        CrosstermEvent::Mouse(mouse) => {
                            let _ = event_tx.send(TerminalEvent::Mouse(mouse));
                        }
                        CrosstermEvent::Resize(x, y) => {
                            let _ = event_tx.send(TerminalEvent::Resize(x, y));
                        }
                        CrosstermEvent::FocusLost => {
                            let _ = event_tx.send(TerminalEvent::FocusLost);
                        }
                        CrosstermEvent::FocusGained => {
                            let _ = event_tx.send(TerminalEvent::FocusGained);
                        }
                        CrosstermEvent::Paste(s) => {
                            let _ = event_tx.send(TerminalEvent::Paste(s));
                        }
                    },
                    Err(_) => {
                        let _ = event_tx.send(TerminalEvent::Error);
                    }
                }
            }
        });
    }

    pub fn enter(&mut self) -> Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            std::io::stdout(),
            EnterAlternateScreen,
            cursor::Hide
        )?;
        if self.mouse {
            crossterm::execute!(std::io::stdout(), EnableMouseCapture)?;
        }
        if self.paste {
            crossterm::execute!(std::io::stdout(), EnableBracketedPaste)?;
        }
        self.start();
        Ok(())
    }

    pub fn exit(&mut self) -> Result<()> {
        if crossterm::terminal::is_raw_mode_enabled()? {
            self.flush()?;
            if self.paste {
                crossterm::execute!(std::io::stdout(), DisableBracketedPaste)?;
            }
            if self.mouse {
                crossterm::execute!(std::io::stdout(), DisableMouseCapture)?;
            }
            crossterm::execute!(
                std::io::stdout(),
                LeaveAlternateScreen,
                cursor::Show
            )?;
            crossterm::terminal::disable_raw_mode()?;
        }
        Ok(())
    }

    pub fn restore() -> Result<()> {
        crossterm::execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            cursor::Show
        )?;
        crossterm::terminal::disable_raw_mode()?;
        Ok(())
    }

    #[allow(clippy::should_implement_trait)]
    pub async fn next(&mut self) -> Option<TerminalEvent> {
        self.event_rx.recv_async().await.ok()
    }
}

impl Deref for Tui {
    type Target = ratatui::Terminal<Backend<std::io::Stdout>>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl DerefMut for Tui {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        self.exit().unwrap();
    }
}
