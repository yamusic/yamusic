use std::{
    any::Any,
    collections::HashMap,
    fmt::Debug,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use async_trait::async_trait;
use ratatui::{Frame, layout::Rect};

use crate::framework::{event::UserEvent, id::ComponentId};

#[derive(Debug, Clone, Default)]
pub enum Action<M = ()> {
    #[default]
    None,
    Redraw,
    Spawn(SpawnRequest),
    Emit(M),
    Batch(Vec<Action<M>>),
    Navigate(String),
    Back,
    Focus,
    Blur,
    Quit,
}

impl<M> Action<M> {
    pub fn needs_redraw(&self) -> bool {
        matches!(self, Action::Redraw | Action::Batch(_))
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Action::None)
    }

    pub fn and(self, other: Action<M>) -> Action<M> {
        match (self, other) {
            (Action::None, other) => other,
            (this, Action::None) => this,
            (Action::Batch(mut a), Action::Batch(b)) => {
                a.extend(b);
                Action::Batch(a)
            }
            (Action::Batch(mut a), other) => {
                a.push(other);
                Action::Batch(a)
            }
            (this, Action::Batch(mut b)) => {
                b.insert(0, this);
                Action::Batch(b)
            }
            (this, other) => Action::Batch(vec![this, other]),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpawnRequest {
    pub group: String,
    pub description: String,
}

impl SpawnRequest {
    pub fn new(group: impl Into<String>) -> Self {
        Self {
            group: group.into(),
            description: String::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }
}

pub struct ComponentContext {
    pub focused: bool,
    pub visible: bool,
    pub frame: u64,
    pub delta_ms: u64,
}

impl Default for ComponentContext {
    fn default() -> Self {
        Self {
            focused: false,
            visible: true,
            frame: 0,
            delta_ms: 16,
        }
    }
}

pub struct ComponentCore {
    pub id: ComponentId,
    dirty: AtomicBool,
    focused: AtomicBool,
    visible: AtomicBool,
    mounted: AtomicBool,
    frame: AtomicU64,
    last_tick_ms: AtomicU64,
}

impl ComponentCore {
    pub fn new(id: ComponentId) -> Self {
        Self {
            id,
            dirty: AtomicBool::new(true),
            focused: AtomicBool::new(false),
            visible: AtomicBool::new(true),
            mounted: AtomicBool::new(false),
            frame: AtomicU64::new(0),
            last_tick_ms: AtomicU64::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            ),
        }
    }

    pub fn id(&self) -> &ComponentId {
        &self.id
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }

    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Release);
    }

    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Release);
    }

    pub fn is_focused(&self) -> bool {
        self.focused.load(Ordering::Acquire)
    }

    pub fn set_focused(&self, focused: bool) {
        self.focused.store(focused, Ordering::Release);
        if focused {
            self.mark_dirty();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }

    pub fn set_visible(&self, visible: bool) {
        let was_visible = self.visible.swap(visible, Ordering::AcqRel);
        if was_visible != visible {
            self.mark_dirty();
        }
    }

    pub fn is_mounted(&self) -> bool {
        self.mounted.load(Ordering::Acquire)
    }

    pub fn set_mounted(&self, mounted: bool) {
        self.mounted.store(mounted, Ordering::Release);
    }

    pub fn frame(&self) -> u64 {
        self.frame.load(Ordering::Relaxed)
    }

    pub fn tick_frame(&self) -> u64 {
        self.frame.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn context(&self) -> ComponentContext {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let last_ms = self.last_tick_ms.swap(now_ms, Ordering::Relaxed);
        let delta_ms = if last_ms > 0 {
            now_ms.saturating_sub(last_ms)
        } else {
            16
        };

        ComponentContext {
            focused: self.is_focused(),
            visible: self.is_visible(),
            frame: self.frame(),
            delta_ms,
        }
    }
}

impl Clone for ComponentCore {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            dirty: AtomicBool::new(self.dirty.load(Ordering::Relaxed)),
            focused: AtomicBool::new(self.focused.load(Ordering::Relaxed)),
            visible: AtomicBool::new(self.visible.load(Ordering::Relaxed)),
            mounted: AtomicBool::new(self.mounted.load(Ordering::Relaxed)),
            frame: AtomicU64::new(self.frame.load(Ordering::Relaxed)),
            last_tick_ms: AtomicU64::new(self.last_tick_ms.load(Ordering::Relaxed)),
        }
    }
}

impl Debug for ComponentCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentCore")
            .field("id", &self.id)
            .field("dirty", &self.is_dirty())
            .field("focused", &self.is_focused())
            .field("visible", &self.is_visible())
            .field("mounted", &self.is_mounted())
            .finish()
    }
}

#[async_trait]
pub trait Component: Send + Sync {
    type Message: Clone + Send + 'static;

    fn core(&self) -> &ComponentCore;

    fn core_mut(&mut self) -> &mut ComponentCore;

    fn id(&self) -> &ComponentId {
        self.core().id()
    }

    fn view(&self, frame: &mut Frame, area: Rect);

    fn handle_event(&mut self, event: &UserEvent) -> Action<Self::Message> {
        let _ = event;
        Action::None
    }

    async fn on_mount(&mut self, outbox: &mut Vec<Self::Message>) {
        let _ = outbox;
    }

    async fn on_unmount(&mut self) {}

    fn on_focus(&mut self) {}

    fn on_blur(&mut self) {}

    fn on_tick(&mut self) -> Action<Self::Message> {
        Action::None
    }

    fn needs_tick(&self) -> bool {
        false
    }
}

#[async_trait]
pub trait AnyComponent: Send + Sync {
    fn id(&self) -> &ComponentId;

    fn is_dirty(&self) -> bool;

    fn mark_dirty(&self);

    fn clear_dirty(&self);

    fn is_focused(&self) -> bool;

    fn set_focused(&self, focused: bool);

    fn is_visible(&self) -> bool;

    fn set_visible(&self, visible: bool);

    fn is_mounted(&self) -> bool;

    fn set_mounted(&self, mounted: bool);

    fn view(&self, frame: &mut Frame, area: Rect);

    fn handle_event(&mut self, event: &UserEvent) -> Box<dyn Any + Send>;

    async fn mount(&mut self, outbox: &mut Vec<Box<dyn Any + Send>>);

    async fn unmount(&mut self);

    fn focus(&mut self);

    fn blur(&mut self);

    fn tick(&mut self) -> Box<dyn Any + Send>;

    fn needs_tick(&self) -> bool;

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct ComponentNode<C: Component> {
    component: C,
}

impl<C: Component + 'static> ComponentNode<C> {
    pub fn new(component: C) -> Self {
        Self { component }
    }

    pub fn inner(&self) -> &C {
        &self.component
    }

    pub fn inner_mut(&mut self) -> &mut C {
        &mut self.component
    }

    pub fn into_inner(self) -> C {
        self.component
    }
}

#[async_trait]
impl<C: Component + 'static> AnyComponent for ComponentNode<C>
where
    C::Message: 'static,
{
    fn id(&self) -> &ComponentId {
        self.component.id()
    }

    fn is_dirty(&self) -> bool {
        self.component.core().is_dirty()
    }

    fn mark_dirty(&self) {
        self.component.core().mark_dirty();
    }

    fn clear_dirty(&self) {
        self.component.core().clear_dirty();
    }

    fn is_focused(&self) -> bool {
        self.component.core().is_focused()
    }

    fn set_focused(&self, focused: bool) {
        self.component.core().set_focused(focused);
    }

    fn is_visible(&self) -> bool {
        self.component.core().is_visible()
    }

    fn set_visible(&self, visible: bool) {
        self.component.core().set_visible(visible);
    }

    fn is_mounted(&self) -> bool {
        self.component.core().is_mounted()
    }

    fn set_mounted(&self, mounted: bool) {
        self.component.core().set_mounted(mounted);
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if self.component.core().is_visible() {
            self.component.view(frame, area);
        }
    }

    fn handle_event(&mut self, event: &UserEvent) -> Box<dyn Any + Send> {
        let action = self.component.handle_event(event);
        Box::new(action)
    }

    async fn mount(&mut self, outbox: &mut Vec<Box<dyn Any + Send>>) {
        self.component.core().set_mounted(true);
        let mut typed_outbox: Vec<C::Message> = Vec::new();
        self.component.on_mount(&mut typed_outbox).await;
        for msg in typed_outbox {
            outbox.push(Box::new(msg));
        }
    }

    async fn unmount(&mut self) {
        self.component.core().set_mounted(false);
        self.component.on_unmount().await;
    }

    fn focus(&mut self) {
        self.component.core().set_focused(true);
        self.component.on_focus();
    }

    fn blur(&mut self) {
        self.component.core().set_focused(false);
        self.component.on_blur();
    }

    fn tick(&mut self) -> Box<dyn Any + Send> {
        self.component.core().tick_frame();
        let action = self.component.on_tick();
        Box::new(action)
    }

    fn needs_tick(&self) -> bool {
        self.component.needs_tick()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct Registry {
    components: HashMap<ComponentId, Box<dyn AnyComponent>>,
    focused_id: Option<ComponentId>,
    render_order: Vec<ComponentId>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
            focused_id: None,
            render_order: Vec::new(),
        }
    }

    pub fn is_mounted(&self, id: &ComponentId) -> bool {
        self.components.contains_key(id)
    }

    pub async fn mount<M: Send + 'static>(
        &mut self,
        component: Box<dyn AnyComponent>,
        outbox: &mut Vec<M>,
    ) {
        let id = component.id().clone();
        self.render_order.push(id.clone());
        self.components.insert(id.clone(), component);

        if let Some(comp) = self.components.get_mut(&id) {
            let mut any_outbox: Vec<Box<dyn Any + Send>> = Vec::new();
            comp.mount(&mut any_outbox).await;

            for msg in any_outbox {
                if let Ok(typed) = msg.downcast::<M>() {
                    outbox.push(*typed);
                }
            }
        }
    }

    pub async fn unmount(&mut self, id: &ComponentId) {
        if let Some(mut comp) = self.components.remove(id) {
            comp.unmount().await;
        }
        self.render_order.retain(|i| i != id);

        if self.focused_id.as_ref() == Some(id) {
            self.focused_id = None;
        }
    }

    pub fn get(&self, id: &ComponentId) -> Option<&dyn AnyComponent> {
        self.components.get(id).map(|c| c.as_ref())
    }

    pub fn get_mut<'a>(&'a mut self, id: &ComponentId) -> Option<&'a mut (dyn AnyComponent + 'a)> {
        self.components
            .get_mut(id)
            .map(|c| &mut **c as &mut (dyn AnyComponent + 'a))
    }

    pub fn get_typed<C: Component + 'static>(&self, id: &ComponentId) -> Option<&C> {
        self.components
            .get(id)
            .and_then(|c| c.as_any().downcast_ref::<ComponentNode<C>>())
            .map(|node| node.inner())
    }

    pub fn get_typed_mut<C: Component + 'static>(&mut self, id: &ComponentId) -> Option<&mut C> {
        self.components
            .get_mut(id)
            .and_then(|c| c.as_any_mut().downcast_mut::<ComponentNode<C>>())
            .map(|node| node.inner_mut())
    }

    pub fn set_focus(&mut self, id: &ComponentId) {
        if let Some(old_id) = &self.focused_id
            && let Some(comp) = self.components.get_mut(old_id)
        {
            comp.blur();
        }

        self.focused_id = Some(id.clone());
        if let Some(comp) = self.components.get_mut(id) {
            comp.focus();
        }
    }

    pub fn clear_focus(&mut self) {
        if let Some(old_id) = self.focused_id.take()
            && let Some(comp) = self.components.get_mut(&old_id)
        {
            comp.blur();
        }
    }

    pub fn focused_id(&self) -> Option<&ComponentId> {
        self.focused_id.as_ref()
    }

    pub fn dispatch_event<M: 'static>(&mut self, event: &UserEvent) -> Option<Action<M>> {
        if let Some(id) = &self.focused_id.clone()
            && let Some(comp) = self.components.get_mut(id)
        {
            let action = comp.handle_event(event);
            if let Ok(typed) = action.downcast::<Action<M>>() {
                return Some(*typed);
            }
        }
        None
    }

    pub fn mark_all_dirty(&self) {
        for comp in self.components.values() {
            comp.mark_dirty();
        }
    }

    pub fn dirty_components(&self) -> Vec<ComponentId> {
        self.components
            .iter()
            .filter(|(_, c)| c.is_dirty())
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn render(&self, frame: &mut Frame, areas: &HashMap<ComponentId, Rect>) {
        for id in &self.render_order {
            if let Some(comp) = self.components.get(id) {
                if comp.is_visible()
                    && let Some(area) = areas.get(id)
                {
                    comp.view(frame, *area);
                }
                comp.clear_dirty();
            }
        }
    }

    pub fn render_one(&self, id: &ComponentId, frame: &mut Frame, area: Rect) {
        if let Some(comp) = self.components.get(id) {
            if comp.is_visible() {
                comp.view(frame, area);
            }
            comp.clear_dirty();
        }
    }

    pub fn tick<M: 'static>(&mut self) -> Vec<Action<M>> {
        let mut actions = Vec::new();

        for comp in self.components.values_mut() {
            if comp.needs_tick() {
                let action = comp.tick();
                if let Ok(typed) = action.downcast::<Action<M>>()
                    && !typed.is_none()
                {
                    actions.push(*typed);
                }
            }
        }

        actions
    }

    pub fn ids(&self) -> Vec<ComponentId> {
        self.render_order.clone()
    }

    pub fn len(&self) -> usize {
        self.components.len()
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}
