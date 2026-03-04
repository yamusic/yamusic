use std::collections::HashMap;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::actions::Route;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Esc,
    Enter,
    Backspace,
    Tab,
    BackTab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
}

pub fn normalize(ev: KeyEvent) -> Option<Key> {
    if ev.modifiers.contains(KeyModifiers::CONTROL)
        && let KeyCode::Char(c) = ev.code
    {
        return Some(Key::Ctrl(c));
    }
    match ev.code {
        KeyCode::Char(c) => Some(Key::Char(c)),
        KeyCode::Esc => Some(Key::Esc),
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::Tab => Some(Key::Tab),
        KeyCode::BackTab => Some(Key::BackTab),
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
        KeyCode::Left => Some(Key::Left),
        KeyCode::Right => Some(Key::Right),
        KeyCode::Home => Some(Key::Home),
        KeyCode::End => Some(Key::End),
        KeyCode::PageUp => Some(Key::PageUp),
        KeyCode::PageDown => Some(Key::PageDown),
        KeyCode::F(n) => Some(Key::F(n)),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum KeyState {
    #[default]
    Idle,
    Prefix(Key),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Current,
    Selected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackIntent {
    Toggle,
    Next,
    Previous,
    SeekForward(u64),
    SeekBackward(u64),
    VolumeUp(u8),
    VolumeDown(u8),
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,
    Like(Target),
    Dislike(Target),
    StartWave(Target),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewIntent {
    Like,
    Dislike,
    QueueAll,
    PlayAllNext,
    StartWave,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueIntent {
    Add,
    PlayNext,
    Remove,
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectIntent {
    ToggleBassBoost,
    ToggleTrebleBoost,
    ToggleChorus,
    ToggleReverb,
    ToggleLowpass,
    ToggleHighpass,
    ToggleBandpass,
    ToggleNotch,
    ToggleDcBlock,
    ToggleEqPreset(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum NavigationIntent {
    Go(Route),
    Back,
    NextTab,
    PrevTab,
    ShowOverlay(Route),
    DismissOverlay,
    ScrollTop,
    ScrollBottom,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    Quit,
    Playback(PlaybackIntent),
    Navigate(NavigationIntent),
    View(ViewIntent),
    Queue(QueueIntent),
    Effect(EffectIntent),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeySeq(Vec<Key>);

impl KeySeq {
    pub fn single(k: Key) -> Self {
        Self(vec![k])
    }
    pub fn chord(first: Key, second: Key) -> Self {
        Self(vec![first, second])
    }
}

pub struct Keymap {
    bindings: HashMap<KeySeq, Intent>,
}

impl Keymap {
    fn new(bindings: HashMap<KeySeq, Intent>) -> Self {
        Self { bindings }
    }

    fn lookup(&self, seq: &KeySeq) -> Option<Intent> {
        self.bindings.get(seq).cloned()
    }

    fn is_prefix(&self, key: &Key) -> bool {
        self.bindings
            .keys()
            .any(|s| s.0.len() > 1 && s.0.first() == Some(key))
    }
}

pub struct KeyResolver {
    state: KeyState,
    keymap: Keymap,
}

impl KeyResolver {
    pub fn new() -> Self {
        Self {
            state: KeyState::Idle,
            keymap: build_global_keymap(),
        }
    }

    pub fn state(&self) -> &KeyState {
        &self.state
    }

    pub fn peek_prefix(&self) -> Option<char> {
        match &self.state {
            KeyState::Prefix(Key::Char(c)) => Some(*c),
            _ => None,
        }
    }

    pub fn advance(&mut self, key: &Key) -> Option<Intent> {
        match self.state.clone() {
            KeyState::Idle => {
                if let Some(intent) = self.keymap.lookup(&KeySeq::single(key.clone())) {
                    return Some(intent);
                }
                if self.keymap.is_prefix(key) {
                    self.state = KeyState::Prefix(key.clone());
                }
                None
            }
            KeyState::Prefix(prefix) => {
                self.state = KeyState::Idle;
                if *key == Key::Esc {
                    return None;
                }
                self.keymap.lookup(&KeySeq::chord(prefix, key.clone()))
            }
        }
    }

    pub fn reset(&mut self) {
        self.state = KeyState::Idle;
    }
}

impl Default for KeyResolver {
    fn default() -> Self {
        Self::new()
    }
}

fn build_global_keymap() -> Keymap {
    use EffectIntent;
    use Intent::*;
    use Key::*;
    use NavigationIntent::*;
    use PlaybackIntent::*;
    use QueueIntent;
    use Target::*;
    use ViewIntent;

    Keymap::new(HashMap::from([
        (KeySeq::single(Ctrl('c')), Quit),
        (KeySeq::single(Ctrl('q')), Quit),
        (KeySeq::single(Char(' ')), Playback(Toggle)),
        (KeySeq::single(Char('.')), Playback(Next)),
        (KeySeq::single(Char(',')), Playback(Previous)),
        (KeySeq::single(Char('+')), Playback(VolumeUp(5))),
        (KeySeq::single(Char('=')), Playback(VolumeUp(5))),
        (KeySeq::single(Char('-')), Playback(VolumeDown(5))),
        (KeySeq::single(Char('m')), Playback(ToggleMute)),
        (KeySeq::single(Char('s')), Playback(ToggleShuffle)),
        (KeySeq::single(Char('r')), Playback(CycleRepeat)),
        (KeySeq::single(Char('<')), Playback(SeekBackward(10))),
        (KeySeq::single(Char('>')), Playback(SeekForward(10))),
        (KeySeq::single(Char('f')), Playback(Like(Selected))),
        (KeySeq::single(Char('d')), Playback(Dislike(Selected))),
        (KeySeq::chord(Char('c'), Char('f')), Playback(Like(Current))),
        (
            KeySeq::chord(Char('c'), Char('d')),
            Playback(Dislike(Current)),
        ),
        (KeySeq::chord(Char('v'), Char('f')), View(ViewIntent::Like)),
        (
            KeySeq::chord(Char('v'), Char('d')),
            View(ViewIntent::Dislike),
        ),
        (
            KeySeq::chord(Char('v'), Char('q')),
            View(ViewIntent::QueueAll),
        ),
        (
            KeySeq::chord(Char('v'), Char('n')),
            View(ViewIntent::PlayAllNext),
        ),
        (
            KeySeq::chord(Char('c'), Char('w')),
            Playback(StartWave(Current)),
        ),
        (KeySeq::single(Char('w')), Playback(StartWave(Selected))),
        (
            KeySeq::chord(Char('v'), Char('w')),
            View(ViewIntent::StartWave),
        ),
        (KeySeq::chord(Char('g'), Char('g')), Navigate(ScrollTop)),
        (
            KeySeq::chord(Char('g'), Char('y')),
            Navigate(ShowOverlay(Route::Lyrics)),
        ),
        (
            KeySeq::chord(Char('g'), Char('q')),
            Navigate(Go(Route::Queue)),
        ),
        (
            KeySeq::chord(Char('g'), Char('e')),
            Navigate(ShowOverlay(Route::Effects)),
        ),
        (KeySeq::single(Char('G')), Navigate(ScrollBottom)),
        (KeySeq::single(Tab), Navigate(NextTab)),
        (KeySeq::single(BackTab), Navigate(PrevTab)),
        (KeySeq::single(Esc), Navigate(Back)),
        (KeySeq::single(Char('/')), Navigate(Go(Route::Search))),
        (KeySeq::single(Char('2')), Navigate(Go(Route::Home))),
        (KeySeq::single(Char('3')), Navigate(Go(Route::Liked))),
        (KeySeq::single(Char('4')), Navigate(Go(Route::Playlists))),
        (KeySeq::chord(Char('q'), Char('a')), Queue(QueueIntent::Add)),
        (
            KeySeq::chord(Char('q'), Char('n')),
            Queue(QueueIntent::PlayNext),
        ),
        (
            KeySeq::chord(Char('q'), Char('d')),
            Queue(QueueIntent::Remove),
        ),
        (
            KeySeq::chord(Char('q'), Char('c')),
            Queue(QueueIntent::Clear),
        ),
        (
            KeySeq::chord(Char('e'), Char('b')),
            Effect(EffectIntent::ToggleBassBoost),
        ),
        (
            KeySeq::chord(Char('e'), Char('t')),
            Effect(EffectIntent::ToggleTrebleBoost),
        ),
        (
            KeySeq::chord(Char('e'), Char('c')),
            Effect(EffectIntent::ToggleChorus),
        ),
        (
            KeySeq::chord(Char('e'), Char('r')),
            Effect(EffectIntent::ToggleReverb),
        ),
        (
            KeySeq::chord(Char('e'), Char('l')),
            Effect(EffectIntent::ToggleLowpass),
        ),
        (
            KeySeq::chord(Char('e'), Char('h')),
            Effect(EffectIntent::ToggleHighpass),
        ),
        (
            KeySeq::chord(Char('e'), Char('p')),
            Effect(EffectIntent::ToggleBandpass),
        ),
        (
            KeySeq::chord(Char('e'), Char('n')),
            Effect(EffectIntent::ToggleNotch),
        ),
        (
            KeySeq::chord(Char('e'), Char('d')),
            Effect(EffectIntent::ToggleDcBlock),
        ),
        (
            KeySeq::chord(Char('e'), Char('1')),
            Effect(EffectIntent::ToggleEqPreset("vocal".into())),
        ),
        (
            KeySeq::chord(Char('e'), Char('2')),
            Effect(EffectIntent::ToggleEqPreset("bass".into())),
        ),
        (
            KeySeq::chord(Char('e'), Char('3')),
            Effect(EffectIntent::ToggleEqPreset("acoustic".into())),
        ),
        (
            KeySeq::chord(Char('e'), Char('4')),
            Effect(EffectIntent::ToggleEqPreset("rock".into())),
        ),
    ]))
}
