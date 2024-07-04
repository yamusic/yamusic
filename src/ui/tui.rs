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
pub enum Event {
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
    pub terminal: ratatui::Terminal<Backend<std::io::Stderr>>,
    pub event_rx: Receiver<Event>,
    pub event_tx: Sender<Event>,
    pub mouse: bool,
    pub paste: bool,
}

impl Tui {
    pub fn new() -> Result<Self> {
        let terminal = ratatui::Terminal::new(Backend::new(std::io::stderr()))?;
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
            event_tx.send(Event::Init).unwrap();
            loop {
                let _ = event_tx.send(Event::Tick);
                if !event::poll(Duration::from_millis(8)).unwrap() {
                    continue;
                }
                let crossterm_event = event::read();
                match crossterm_event {
                    Ok(evt) => match evt {
                        CrosstermEvent::Key(key) => {
                            if key.kind == KeyEventKind::Press {
                                event_tx.send(Event::Key(key)).unwrap();
                            }
                        }
                        CrosstermEvent::Mouse(mouse) => {
                            event_tx.send(Event::Mouse(mouse)).unwrap();
                        }
                        CrosstermEvent::Resize(x, y) => {
                            event_tx.send(Event::Resize(x, y)).unwrap();
                        }
                        CrosstermEvent::FocusLost => {
                            event_tx.send(Event::FocusLost).unwrap();
                        }
                        CrosstermEvent::FocusGained => {
                            event_tx.send(Event::FocusGained).unwrap();
                        }
                        CrosstermEvent::Paste(s) => {
                            event_tx.send(Event::Paste(s)).unwrap();
                        }
                    },
                    Err(_) => {
                        event_tx.send(Event::Error).unwrap();
                    }
                }
            }
        });
    }

    pub fn enter(&mut self) -> Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            std::io::stderr(),
            EnterAlternateScreen,
            cursor::Hide
        )?;
        if self.mouse {
            crossterm::execute!(std::io::stderr(), EnableMouseCapture)?;
        }
        if self.paste {
            crossterm::execute!(std::io::stderr(), EnableBracketedPaste)?;
        }
        self.start();
        Ok(())
    }

    pub fn exit(&mut self) -> Result<()> {
        if crossterm::terminal::is_raw_mode_enabled()? {
            self.flush()?;
            if self.paste {
                crossterm::execute!(std::io::stderr(), DisableBracketedPaste)?;
            }
            if self.mouse {
                crossterm::execute!(std::io::stderr(), DisableMouseCapture)?;
            }
            crossterm::execute!(
                std::io::stderr(),
                LeaveAlternateScreen,
                cursor::Show
            )?;
            crossterm::terminal::disable_raw_mode()?;
        }
        Ok(())
    }

    pub fn restore() -> Result<()> {
        crossterm::execute!(
            std::io::stderr(),
            LeaveAlternateScreen,
            cursor::Show
        )?;
        crossterm::terminal::disable_raw_mode()?;
        Ok(())
    }

    #[allow(clippy::should_implement_trait)]
    pub async fn next(&mut self) -> Option<Event> {
        self.event_rx.recv_async().await.ok()
    }
}

impl Deref for Tui {
    type Target = ratatui::Terminal<Backend<std::io::Stderr>>;

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
