use crate::ui::tui;

pub fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = tui::Tui::restore();
        hook(panic_info);
    }));
}
