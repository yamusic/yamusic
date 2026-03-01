use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use flume::{Receiver, Sender};
use ratatui::{Frame, layout::Rect};
use tokio::sync::Mutex;

use crate::framework::{
    component::{Action, AnyComponent, Registry},
    event::UserEvent,
    id::ComponentId,
    tasks::TaskManager,
};

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub target_fps: u32,
    pub adaptive_fps: bool,
    pub min_frame_time_ms: u64,
    pub max_frame_time_ms: u64,
    pub auto_cleanup_tasks: bool,
    pub task_cleanup_interval_ms: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            target_fps: 60,
            adaptive_fps: true,
            min_frame_time_ms: 16,
            max_frame_time_ms: 1000,
            auto_cleanup_tasks: true,
            task_cleanup_interval_ms: 1000,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RuntimeMessage<M> {
    Event(UserEvent),
    Action(Action<M>),
    App(M),
    Tick,
    Quit,
    Redraw,
}

pub trait Model: Send + Sync {
    type Message: Clone + Send + 'static;

    fn update(&mut self, msg: Self::Message) -> Option<Self::Message>;

    fn tick_rate(&self) -> Duration {
        Duration::from_millis(100)
    }

    fn before_render(&mut self) {}

    fn after_render(&mut self) {}

    fn should_quit(&self) -> bool {
        false
    }
}

pub type RegistryHandle = Arc<Mutex<Registry>>;

pub struct Runtime<M: Model> {
    pub model: M,
    registry: RegistryHandle,
    tasks: Arc<TaskManager>,
    config: RuntimeConfig,
    message_rx: Receiver<RuntimeMessage<M::Message>>,
    message_tx: Sender<RuntimeMessage<M::Message>>,
    needs_redraw: bool,
    last_render: Instant,
    last_cleanup: Instant,
    layout_cache: HashMap<ComponentId, Rect>,
    running: bool,
}

impl<M: Model + 'static> Runtime<M> {
    pub fn new(model: M) -> Self {
        Self::with_config(model, RuntimeConfig::default())
    }

    pub fn with_config(model: M, config: RuntimeConfig) -> Self {
        let (message_tx, message_rx) = flume::unbounded();

        Self {
            model,
            registry: Arc::new(Mutex::new(Registry::new())),
            tasks: Arc::new(TaskManager::new()),
            config,
            message_rx,
            message_tx,
            needs_redraw: true,
            last_render: Instant::now(),
            last_cleanup: Instant::now(),
            layout_cache: HashMap::new(),
            running: true,
        }
    }

    pub fn registry_handle(&self) -> RegistryHandle {
        Arc::clone(&self.registry)
    }

    pub fn tasks(&self) -> &Arc<TaskManager> {
        &self.tasks
    }

    pub fn message_sender(&self) -> Sender<RuntimeMessage<M::Message>> {
        self.message_tx.clone()
    }

    pub fn send(&self, msg: RuntimeMessage<M::Message>) {
        let _ = self.message_tx.send(msg);
    }

    pub fn send_app(&self, msg: M::Message) {
        let _ = self.message_tx.send(RuntimeMessage::App(msg));
    }

    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }

    pub fn is_running(&self) -> bool {
        self.running && !self.model.should_quit()
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn set_layout(&mut self, id: ComponentId, area: Rect) {
        self.layout_cache.insert(id, area);
    }

    pub fn clear_layouts(&mut self) {
        self.layout_cache.clear();
    }

    pub async fn dispatch_message(&mut self, msg: M::Message) {
        if let Some(response) = self.model.update(msg) {
            self.send_app(response);
        }
        self.needs_redraw = true;
    }

    pub async fn dispatch_event(&mut self, event: UserEvent) -> Option<Action<M::Message>> {
        let mut registry = self.registry.lock().await;
        registry.dispatch_event(&event)
    }

    pub async fn process_pending(&mut self) {
        while let Ok(msg) = self.message_rx.try_recv() {
            match msg {
                RuntimeMessage::Event(event) => {
                    if let Some(action) = self.dispatch_event(event).await {
                        self.handle_action(action).await;
                    }
                }
                RuntimeMessage::Action(action) => {
                    self.handle_action(action).await;
                }
                RuntimeMessage::App(msg) => {
                    self.dispatch_message(msg).await;
                }
                RuntimeMessage::Tick => {
                    let actions = {
                        let mut registry = self.registry.lock().await;
                        registry.tick::<M::Message>()
                    };
                    for action in actions {
                        self.handle_action(action).await;
                    }
                }
                RuntimeMessage::Quit => {
                    self.running = false;
                }
                RuntimeMessage::Redraw => {
                    self.needs_redraw = true;
                }
            }
        }

        if self.config.auto_cleanup_tasks
            && self.last_cleanup.elapsed().as_millis() as u64
                >= self.config.task_cleanup_interval_ms
        {
            self.tasks.cleanup();
            self.last_cleanup = Instant::now();
        }
    }

    async fn handle_action(&mut self, action: Action<M::Message>) {
        match action {
            Action::None => {}
            Action::Redraw => {
                self.needs_redraw = true;
            }
            Action::Spawn(request) => {
                tracing::debug!("Spawn request: {:?}", request);
            }
            Action::Emit(msg) => {
                self.dispatch_message(msg).await;
            }
            Action::Batch(actions) => {
                for action in actions {
                    Box::pin(self.handle_action(action)).await;
                }
            }
            Action::Navigate(route) => {
                tracing::debug!("Navigate to: {}", route);
            }
            Action::Back => {
                tracing::debug!("Navigate back");
            }
            Action::Focus => {}
            Action::Blur => {
                let mut registry = self.registry.lock().await;
                registry.clear_focus();
            }
            Action::Quit => {
                self.running = false;
            }
        }
    }

    pub fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    pub async fn render(&mut self, frame: &mut Frame<'_>) {
        self.model.before_render();

        let registry = self.registry.lock().await;
        registry.render(frame, &self.layout_cache);
        drop(registry);

        self.model.after_render();
        self.needs_redraw = false;
        self.last_render = Instant::now();
    }

    pub async fn render_with<F>(&mut self, frame: &mut Frame<'_>, layout_fn: F)
    where
        F: FnOnce(&mut Frame<'_>, &Registry) -> HashMap<ComponentId, Rect>,
    {
        self.model.before_render();

        let registry = self.registry.lock().await;
        let layouts = layout_fn(frame, &registry);
        registry.render(frame, &layouts);
        drop(registry);

        self.model.after_render();
        self.needs_redraw = false;
        self.last_render = Instant::now();
    }

    pub fn time_until_tick(&self) -> Duration {
        let tick_rate = self.model.tick_rate();
        let elapsed = self.last_render.elapsed();

        if elapsed >= tick_rate {
            Duration::ZERO
        } else {
            tick_rate - elapsed
        }
    }

    pub async fn mount(&mut self, component: Box<dyn AnyComponent>) {
        let mut outbox: Vec<M::Message> = Vec::new();
        let mut registry = self.registry.lock().await;
        registry.mount(component, &mut outbox).await;
        drop(registry);

        for msg in outbox {
            self.dispatch_message(msg).await;
        }
    }

    pub async fn unmount(&mut self, id: &ComponentId) {
        let mut registry = self.registry.lock().await;
        registry.unmount(id).await;
    }

    pub async fn set_focus(&mut self, id: &ComponentId) {
        let mut registry = self.registry.lock().await;
        registry.set_focus(id);
    }

    pub async fn clear_focus(&mut self) {
        let mut registry = self.registry.lock().await;
        registry.clear_focus();
    }
}

pub struct SyncRuntime<M> {
    pub model: M,
    registry: Registry,
    tasks: TaskManager,
    layout_cache: HashMap<ComponentId, Rect>,
    needs_redraw: bool,
}

impl<M> SyncRuntime<M> {
    pub fn new(model: M) -> Self {
        Self {
            model,
            registry: Registry::new(),
            tasks: TaskManager::new(),
            layout_cache: HashMap::new(),
            needs_redraw: true,
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut Registry {
        &mut self.registry
    }

    pub fn tasks(&self) -> &TaskManager {
        &self.tasks
    }

    pub fn tasks_mut(&mut self) -> &mut TaskManager {
        &mut self.tasks
    }

    pub fn set_layout(&mut self, id: ComponentId, area: Rect) {
        self.layout_cache.insert(id, area);
    }

    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }

    pub fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        self.registry.render(frame, &self.layout_cache);
        self.needs_redraw = false;
    }

    pub fn dispatch_event<Msg: 'static>(&mut self, event: &UserEvent) -> Option<Action<Msg>> {
        self.registry.dispatch_event(event)
    }
}

pub struct RuntimeBuilder<M: Model> {
    model: M,
    config: RuntimeConfig,
}

impl<M: Model + 'static> RuntimeBuilder<M> {
    pub fn new(model: M) -> Self {
        Self {
            model,
            config: RuntimeConfig::default(),
        }
    }

    pub fn target_fps(mut self, fps: u32) -> Self {
        self.config.target_fps = fps;
        self.config.min_frame_time_ms = 1000 / fps as u64;
        self
    }

    pub fn adaptive_fps(mut self, enabled: bool) -> Self {
        self.config.adaptive_fps = enabled;
        self
    }

    pub fn auto_cleanup_tasks(mut self, enabled: bool) -> Self {
        self.config.auto_cleanup_tasks = enabled;
        self
    }

    pub fn task_cleanup_interval(mut self, interval: Duration) -> Self {
        self.config.task_cleanup_interval_ms = interval.as_millis() as u64;
        self
    }

    pub fn build(self) -> Runtime<M> {
        Runtime::with_config(self.model, self.config)
    }
}
