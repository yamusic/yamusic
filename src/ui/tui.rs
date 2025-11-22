use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};

use color_eyre::eyre::Result;

use crossterm::event::EventStream;
use flume::{Receiver, Sender};
use futures::{FutureExt, StreamExt};
use ratatui::crossterm::{
    cursor,
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event as CrosstermEvent, KeyEvent, KeyEventKind, MouseEvent,
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
        tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut tick_interval = tokio::time::interval(Duration::from_millis(33));
            loop {
                let tick_delay = tick_interval.tick();
                let crossterm_event = reader.next().fuse();

                tokio::select! {
                    _ = tick_delay => {
                        let _ = event_tx.send_async(TerminalEvent::Tick).await;
                    }
                    Some(Ok(evt)) = crossterm_event => {
                        match evt {
                            CrosstermEvent::Key(key) => {
                                if key.kind == KeyEventKind::Press {
                                    let _ = event_tx.send_async(TerminalEvent::Key(key)).await;
                                }
                            }
                            CrosstermEvent::Mouse(mouse) => {
                                let _ = event_tx.send_async(TerminalEvent::Mouse(mouse)).await;
                            }
                            CrosstermEvent::Resize(x, y) => {
                                let _ = event_tx.send_async(TerminalEvent::Resize(x, y)).await;
                            }
                            CrosstermEvent::FocusLost => {
                                let _ = event_tx.send_async(TerminalEvent::FocusLost).await;
                            }
                            CrosstermEvent::FocusGained => {
                                let _ = event_tx.send_async(TerminalEvent::FocusGained).await;
                            }
                            CrosstermEvent::Paste(s) => {
                                let _ = event_tx.send_async(TerminalEvent::Paste(s)).await;
                            }
                        }
                    }
                }
            }
        });
    }

    pub fn enter(&mut self) -> Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(std::io::stdout(), EnterAlternateScreen, cursor::Hide)?;
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
            Self::restore()?;
        }
        Ok(())
    }

    pub fn restore() -> Result<()> {
        crossterm::execute!(std::io::stdout(), LeaveAlternateScreen, cursor::Show)?;
        crossterm::terminal::disable_raw_mode()?;
        Ok(())
    }

    #[allow(clippy::should_implement_trait)]
    pub async fn next(&self) -> Option<TerminalEvent> {
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
