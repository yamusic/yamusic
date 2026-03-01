use crate::app::terminal::Terminal;

pub fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = Terminal::restore();
        hook(panic_info);
    }));
}
