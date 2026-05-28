use gpui::{App, KeyBinding, actions};

actions!(qingqi, [Quit, OpenLauncher, OpenClipboard]);

pub fn register_in_app_bindings(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
}
