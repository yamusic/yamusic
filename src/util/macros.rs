#[macro_export]
macro_rules! keymap {
    ($evt:expr, $($key:pat_param $(| $($mods:ident)-+)? => $action:expr),+ $(,)?) => {
        match $evt.code {
            $(
                $key => {
                    if keymap!(@check_mods $evt, $($($mods),+)?) {
                        $action
                    }
                },
            )+
            _ => {}
        }
    };
    (@check_mods $evt:expr,) => { true };
    (@check_mods $evt:expr, $($mods:ident),+) => {
        $evt.modifiers == ($(event::KeyModifiers::$mods)|+)
    };
}
