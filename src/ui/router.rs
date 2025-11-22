use crate::event::events::Event;
use crate::ui::context::AppContext;
use crate::ui::state::AppState;
use crate::ui::traits::{Action, View};
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

pub struct Router {
    pub stack: Vec<Box<dyn View>>,
    pub overlay: Option<Box<dyn View>>,
}

impl Router {
    pub fn new(initial_view: Box<dyn View>) -> Self {
        Self {
            stack: vec![initial_view],
            overlay: None,
        }
    }

    pub fn push(&mut self, view: Box<dyn View>) {
        self.stack.push(view);
    }

    pub fn pop(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }

    pub fn replace(&mut self, view: Box<dyn View>) {
        self.stack.pop();
        self.stack.push(view);
    }

    pub fn set_overlay(&mut self, view: Box<dyn View>) {
        self.overlay = Some(view);
    }

    pub fn clear_overlay(&mut self) {
        self.overlay = None;
    }

    pub fn has_overlay(&self) -> bool {
        self.overlay.is_some()
    }

    pub fn active_view(&mut self) -> Option<&mut Box<dyn View>> {
        if self.overlay.is_some() {
            self.overlay.as_mut()
        } else {
            self.stack.last_mut()
        }
    }

    pub fn active_view_mut(&mut self) -> Option<&mut Box<dyn View>> {
        if self.overlay.is_some() {
            self.overlay.as_mut()
        } else {
            self.stack.last_mut()
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, state: &AppState, ctx: &AppContext) {
        if let Some(overlay) = &mut self.overlay {
            overlay.render(f, area, state, ctx);
        } else if let Some(view) = self.stack.last_mut() {
            view.render(f, area, state, ctx);
        }
    }

    pub async fn handle_input(
        &mut self,
        key: KeyEvent,
        state: &AppState,
        ctx: &AppContext,
    ) -> Option<Action> {
        if let Some(overlay) = &mut self.overlay {
            overlay.handle_input(key, state, ctx).await
        } else if let Some(view) = self.stack.last_mut() {
            view.handle_input(key, state, ctx).await
        } else {
            None
        }
    }

    pub async fn on_event(&mut self, event: &Event, ctx: &AppContext) {
        for view in &mut self.stack {
            view.on_event(event, ctx).await;
        }

        if let Some(overlay) = &mut self.overlay {
            overlay.on_event(event, ctx).await;
        }
    }
}
