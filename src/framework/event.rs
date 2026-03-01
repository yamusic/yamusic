use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserEvent {
    Key(KeyEvent),
    Mouse(MouseEventData),
    Paste(String),
    Resize { width: u16, height: u16 },
    FocusGained,
    FocusLost,
    Tick,
    Init,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEventData {
    pub column: u16,
    pub row: u16,
    pub kind: MouseEventKind,
    pub button: Option<MouseButton>,
    pub modifiers: KeyModifiers,
}

impl From<MouseEvent> for MouseEventData {
    fn from(event: MouseEvent) -> Self {
        let button = match event.kind {
            MouseEventKind::Down(btn) | MouseEventKind::Up(btn) | MouseEventKind::Drag(btn) => {
                Some(btn)
            }
            _ => None,
        };

        Self {
            column: event.column,
            row: event.row,
            kind: event.kind,
            button,
            modifiers: event.modifiers,
        }
    }
}

impl UserEvent {
    pub fn key(event: KeyEvent) -> Self {
        UserEvent::Key(event)
    }

    pub fn mouse(event: MouseEvent) -> Self {
        UserEvent::Mouse(MouseEventData::from(event))
    }

    pub fn resize(width: u16, height: u16) -> Self {
        UserEvent::Resize { width, height }
    }

    pub fn is_key(&self, code: KeyCode) -> bool {
        matches!(self, UserEvent::Key(k) if k.code == code)
    }

    pub fn is_key_with_mods(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        matches!(self, UserEvent::Key(k) if k.code == code && k.modifiers == modifiers)
    }

    pub fn is_char(&self, c: char) -> bool {
        matches!(self, UserEvent::Key(k) if k.code == KeyCode::Char(c))
    }

    pub fn has_ctrl(&self) -> bool {
        match self {
            UserEvent::Key(k) => k.modifiers.contains(KeyModifiers::CONTROL),
            UserEvent::Mouse(m) => m.modifiers.contains(KeyModifiers::CONTROL),
            _ => false,
        }
    }

    pub fn has_alt(&self) -> bool {
        match self {
            UserEvent::Key(k) => k.modifiers.contains(KeyModifiers::ALT),
            UserEvent::Mouse(m) => m.modifiers.contains(KeyModifiers::ALT),
            _ => false,
        }
    }

    pub fn has_shift(&self) -> bool {
        match self {
            UserEvent::Key(k) => k.modifiers.contains(KeyModifiers::SHIFT),
            UserEvent::Mouse(m) => m.modifiers.contains(KeyModifiers::SHIFT),
            _ => false,
        }
    }

    pub fn key_code(&self) -> Option<KeyCode> {
        match self {
            UserEvent::Key(k) => Some(k.code),
            _ => None,
        }
    }

    pub fn char(&self) -> Option<char> {
        match self {
            UserEvent::Key(k) => match k.code {
                KeyCode::Char(c) => Some(c),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn is_left_click(&self) -> bool {
        matches!(
            self,
            UserEvent::Mouse(MouseEventData {
                kind: MouseEventKind::Down(MouseButton::Left),
                ..
            })
        )
    }

    pub fn is_right_click(&self) -> bool {
        matches!(
            self,
            UserEvent::Mouse(MouseEventData {
                kind: MouseEventKind::Down(MouseButton::Right),
                ..
            })
        )
    }

    pub fn is_scroll_up(&self) -> bool {
        matches!(
            self,
            UserEvent::Mouse(MouseEventData {
                kind: MouseEventKind::ScrollUp,
                ..
            })
        )
    }

    pub fn is_scroll_down(&self) -> bool {
        matches!(
            self,
            UserEvent::Mouse(MouseEventData {
                kind: MouseEventKind::ScrollDown,
                ..
            })
        )
    }

    pub fn mouse_pos(&self) -> Option<(u16, u16)> {
        match self {
            UserEvent::Mouse(m) => Some((m.column, m.row)),
            _ => None,
        }
    }

    pub fn is_quit(&self) -> bool {
        match self {
            UserEvent::Key(k) => {
                k.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('q'))
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub description: String,
}

impl KeyBinding {
    pub fn new(code: KeyCode, description: impl Into<String>) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::NONE,
            description: description.into(),
        }
    }

    pub fn with_modifiers(mut self, modifiers: KeyModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    pub fn ctrl(mut self) -> Self {
        self.modifiers |= KeyModifiers::CONTROL;
        self
    }

    pub fn alt(mut self) -> Self {
        self.modifiers |= KeyModifiers::ALT;
        self
    }

    pub fn shift(mut self) -> Self {
        self.modifiers |= KeyModifiers::SHIFT;
        self
    }

    pub fn matches(&self, event: &UserEvent) -> bool {
        match event {
            UserEvent::Key(k) => k.code == self.code && k.modifiers == self.modifiers,
            _ => false,
        }
    }

    pub fn format(&self) -> String {
        let mut parts = Vec::new();

        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("Alt");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("Shift");
        }

        let key = match self.code {
            KeyCode::Char(c) => c.to_uppercase().to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Delete => "Delete".to_string(),
            KeyCode::Left => "←".to_string(),
            KeyCode::Right => "→".to_string(),
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PgUp".to_string(),
            KeyCode::PageDown => "PgDn".to_string(),
            KeyCode::F(n) => format!("F{}", n),
            _ => "?".to_string(),
        };

        parts.push(&key);
        parts.join("+")
    }
}

#[derive(Debug, Clone, Default)]
pub struct KeyBindings {
    bindings: Vec<(KeyBinding, String)>,
}

impl KeyBindings {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    pub fn add(&mut self, binding: KeyBinding, action: impl Into<String>) {
        self.bindings.push((binding, action.into()));
    }

    pub fn find(&self, event: &UserEvent) -> Option<&str> {
        for (binding, action) in &self.bindings {
            if binding.matches(event) {
                return Some(action);
            }
        }
        None
    }

    pub fn iter(&self) -> impl Iterator<Item = &(KeyBinding, String)> {
        self.bindings.iter()
    }
}
