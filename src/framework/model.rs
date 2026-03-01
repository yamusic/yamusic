use crate::app::terminal::TickRate;

pub trait Model: Send + Sync {
    type Message: Clone + Send + 'static;

    fn update(&mut self, msg: Self::Message) -> Option<Self::Message>;

    fn tick_rate(&self) -> TickRate {
        TickRate::Normal
    }

    fn should_quit(&self) -> bool {
        false
    }

    fn before_render(&mut self) {}

    fn after_render(&mut self) {}
}

#[derive(Debug, Clone)]
pub enum Envelope<M> {
    Message(M),
    Quit,
    Redraw,
    Nop,
}

impl<M> Envelope<M> {
    pub fn is_quit(&self) -> bool {
        matches!(self, Envelope::Quit)
    }

    pub fn is_redraw(&self) -> bool {
        matches!(self, Envelope::Redraw)
    }

    pub fn is_nop(&self) -> bool {
        matches!(self, Envelope::Nop)
    }

    pub fn message(&self) -> Option<&M> {
        match self {
            Envelope::Message(m) => Some(m),
            _ => None,
        }
    }

    pub fn into_message(self) -> Option<M> {
        match self {
            Envelope::Message(m) => Some(m),
            _ => None,
        }
    }
}

impl<M> From<M> for Envelope<M> {
    fn from(msg: M) -> Self {
        Envelope::Message(msg)
    }
}

pub struct UpdateContext<'a, M> {
    pub outbox: &'a mut Vec<M>,
    pub now: std::time::Instant,
}

impl<'a, M> UpdateContext<'a, M> {
    pub fn new(outbox: &'a mut Vec<M>) -> Self {
        Self {
            outbox,
            now: std::time::Instant::now(),
        }
    }

    pub fn send(&mut self, msg: M) {
        self.outbox.push(msg);
    }

    pub fn send_all(&mut self, msgs: impl IntoIterator<Item = M>) {
        self.outbox.extend(msgs);
    }
}
