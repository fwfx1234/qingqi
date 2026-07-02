use gpui::{App, KeyBinding, actions};

actions!(
    ui_text_input,
    [
        Backspace, Delete, Enter, Escape,
        Left, Right, SelectLeft, SelectRight,
        SelectAll, Home, End,
        ShowCharacterPalette, Paste, Cut, Copy,
    ]
);

pub fn init(cx: &mut App) {
    use std::sync::OnceLock;
    static DONE: OnceLock<()> = OnceLock::new();
    if DONE.set(()).is_err() { return; }

    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("UITextInput")),
        KeyBinding::new("delete", Delete, Some("UITextInput")),
        KeyBinding::new("enter", Enter, Some("UITextInput")),
        KeyBinding::new("escape", Escape, Some("UITextInput")),
        KeyBinding::new("left", Left, Some("UITextInput")),
        KeyBinding::new("right", Right, Some("UITextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("UITextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("UITextInput")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-a", SelectAll, Some("UITextInput")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-a", SelectAll, Some("UITextInput")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", Paste, Some("UITextInput")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", Paste, Some("UITextInput")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", Copy, Some("UITextInput")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", Copy, Some("UITextInput")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-x", Cut, Some("UITextInput")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-x", Cut, Some("UITextInput")),
        KeyBinding::new("home", Home, Some("UITextInput")),
        KeyBinding::new("end", End, Some("UITextInput")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("UITextInput")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-alt-space", ShowCharacterPalette, Some("UITextInput")),
    ]);
}
