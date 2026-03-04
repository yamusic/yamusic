pub mod component;
pub mod event;
pub mod id;
pub mod model;
pub mod reactive;
pub mod resources;
pub mod runtime;
pub mod tasks;
pub mod theme;
pub use reactive as signals;

pub use component::{Action, AnyComponent, Component, ComponentCore, ComponentNode, Registry};
pub use event::{KeyBinding, KeyBindings, MouseEventData, UserEvent};
pub use id::ComponentId;
pub use model::Model;
pub use reactive::{ReadSignal, Signal};
pub use resources::{PaginatedResource, Resource, ResourceBuilder, ResourceState};
pub use runtime::{RegistryHandle, Runtime, RuntimeBuilder, RuntimeConfig, RuntimeMessage};
pub use tasks::{DebouncedTask, GroupId, TaskId, TaskManager, TaskScope, ThrottledTask};
pub use theme::{Theme, ThemeColor, ThemeConfig, ThemeStyles};

pub mod prelude {
    pub use super::{
        Action, Component, ComponentCore, ComponentId, ComponentNode, KeyBinding, Model,
        ReadSignal, Registry, Resource, ResourceState, Runtime, Signal, TaskManager, Theme,
        ThemeColor, UserEvent,
    };
}
