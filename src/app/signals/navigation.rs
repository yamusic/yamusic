use crate::app::actions::Route;
use crate::framework::reactive::{Get, Memo, Set, Signal, With, batch, memo, signal};
use im::Vector;

#[derive(Clone)]
pub struct NavigationSignals {
    pub current_route: Signal<Route>,

    pub history: Signal<Vector<Route>>,

    pub overlay: Signal<Option<Route>>,

    pub sidebar_visible: Signal<bool>,

    pub focused_id: Signal<Option<String>>,

    pub can_go_back: Memo<bool>,
}

impl NavigationSignals {
    pub fn new() -> Self {
        let history = signal::<Vector<Route>>(Vector::new());

        let can_go_back = memo({
            let history = history.clone();
            move |_| history.with(|h| !h.is_empty())
        });

        Self {
            current_route: signal(Route::Home),
            history,
            overlay: signal(None),
            sidebar_visible: signal(true),
            focused_id: signal(None),
            can_go_back,
        }
    }

    pub fn navigate(&self, route: Route) {
        batch(|| {
            let current = Get::get(&self.current_route);
            self.history.update(|h| h.push_back(current));
            Set::set(&self.current_route, route);
        });
    }

    pub fn back(&self) -> bool {
        let mut went_back = false;
        self.history.update(|h| {
            if let Some(prev) = h.pop_back() {
                Set::set(&self.current_route, prev);
                went_back = true;
            }
        });
        went_back
    }

    pub fn show_overlay(&self, route: Route) {
        Set::set(&self.overlay, Some(route));
    }

    pub fn dismiss_overlay(&self) {
        Set::set(&self.overlay, None);
    }

    pub fn is_route(&self, route: &Route) -> bool {
        &Get::get(&self.current_route) == route
    }

    pub fn set_route(&self, route: Route) {
        Set::set(&self.current_route, route);
    }

    pub fn has_overlay(&self) -> bool {
        With::with(&self.overlay, |o| o.is_some())
    }
}

impl Default for NavigationSignals {
    fn default() -> Self {
        Self::new()
    }
}
