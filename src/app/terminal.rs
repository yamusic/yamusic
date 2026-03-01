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
        DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture, Event as CrosstermEvent, KeyEvent, KeyEventKind,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickRate {
    High,
    Animation,
    Normal,
    Idle,
}

impl TickRate {
    pub fn as_duration(self) -> Duration {
        match self {
            TickRate::High => Duration::from_millis(16),
            TickRate::Animation => Duration::from_millis(16),
            TickRate::Normal => Duration::from_millis(125),
            TickRate::Idle => Duration::from_millis(1000),
        }
    }
}

pub struct Terminal {
    pub terminal: ratatui::Terminal<Backend<std::io::Stdout>>,
    pub event_rx: Receiver<TerminalEvent>,
    pub event_tx: Sender<TerminalEvent>,
    pub tick_tx: Sender<TickRate>,
    pub tick_rx: Receiver<TickRate>,
    pub mouse: bool,
    pub paste: bool,
}

impl Terminal {
    pub fn new() -> Result<Self> {
        let terminal = ratatui::Terminal::new(Backend::new(std::io::stdout()))?;
        let (event_tx, event_rx) = flume::unbounded();
        let (tick_tx, tick_rx) = flume::unbounded();
        let mouse = false;
        let paste = false;

        Ok(Self {
            terminal,
            event_rx,
            event_tx,
            tick_tx,
            tick_rx,
            mouse,
            paste,
        })
    }

    pub fn mouse(mut self, mouse: bool) -> Self {
        self.mouse = mouse;
        self
    }

    #[allow(dead_code)]
    pub fn paste(mut self, paste: bool) -> Self {
        self.paste = paste;
        self
    }

    pub fn start(&mut self) {
        let event_tx = self.event_tx.clone();
        let tick_rx = self.tick_rx.clone();

        tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut current_rate = TickRate::Normal;
            let mut tick_interval = tokio::time::interval(current_rate.as_duration());
            tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                let tick_delay = tick_interval.tick();
                let crossterm_event = reader.next().fuse();
                let tick_rate_update = tick_rx.recv_async();

                tokio::select! {
                    _ = tick_delay => {
                        let _ = event_tx.send(TerminalEvent::Tick);
                    }
                    Ok(new_rate) = tick_rate_update => {
                        if current_rate != new_rate {
                            tracing::debug!("Tick rate changed: {:?} -> {:?}", current_rate, new_rate);
                            current_rate = new_rate;
                            tick_interval = tokio::time::interval(current_rate.as_duration());
                            tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                        }
                    }
                    Some(Ok(evt)) = crossterm_event => {
                        let event = match evt {
                            CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press =>
                                Some(TerminalEvent::Key(key)),
                            CrosstermEvent::Mouse(mouse) => Some(TerminalEvent::Mouse(mouse)),
                            CrosstermEvent::Resize(x, y) => Some(TerminalEvent::Resize(x, y)),
                            CrosstermEvent::FocusLost => Some(TerminalEvent::FocusLost),
                            CrosstermEvent::FocusGained => Some(TerminalEvent::FocusGained),
                            CrosstermEvent::Paste(s) => Some(TerminalEvent::Paste(s)),
                            _ => None,
                        };
                        if let Some(e) = event {
                            let _ = event_tx.send(e);
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
        crossterm::execute!(std::io::stdout(), EnableFocusChange)?;
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
            crossterm::execute!(std::io::stdout(), DisableFocusChange)?;
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

    pub fn set_tick_rate(&self, rate: TickRate) {
        let _ = self.tick_tx.send(rate);
    }
}

impl Deref for Terminal {
    type Target = ratatui::Terminal<Backend<std::io::Stdout>>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl DerefMut for Terminal {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.exit();
    }
}
