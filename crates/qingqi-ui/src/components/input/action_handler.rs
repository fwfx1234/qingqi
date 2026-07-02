use gpui::*;

pub trait TextInputActionHandler: 'static {
    fn value(&self) -> String;
    fn backspace(&mut self, _: &super::keyboard::Backspace, _w: &mut Window, _cx: &mut App) {}
    fn delete(&mut self, _: &super::keyboard::Delete, _w: &mut Window, _cx: &mut App) {}
    fn left(&mut self, _: &super::keyboard::Left, _w: &mut Window, _cx: &mut App) {}
    fn right(&mut self, _: &super::keyboard::Right, _w: &mut Window, _cx: &mut App) {}
    fn select_left(&mut self, _: &super::keyboard::SelectLeft, _w: &mut Window, _cx: &mut App) {}
    fn select_right(&mut self, _: &super::keyboard::SelectRight, _w: &mut Window, _cx: &mut App) {}
    fn select_all(&mut self, _: &super::keyboard::SelectAll, _w: &mut Window, _cx: &mut App) {}
    fn home(&mut self, _: &super::keyboard::Home, _w: &mut Window, _cx: &mut App) {}
    fn end(&mut self, _: &super::keyboard::End, _w: &mut Window, _cx: &mut App) {}
    fn paste(&mut self, _: &super::keyboard::Paste, _w: &mut Window, _cx: &mut App) {}
    fn copy(&mut self, _: &super::keyboard::Copy, _w: &mut Window, _cx: &mut App) {}
    fn cut(&mut self, _: &super::keyboard::Cut, _w: &mut Window, _cx: &mut App) {}
    fn enter(&mut self, _: &super::keyboard::Enter, _w: &mut Window, _cx: &mut App) {}
    fn escape(&mut self, _: &super::keyboard::Escape, _w: &mut Window, _cx: &mut App) {}
    fn show_character_palette(&mut self, _: &super::keyboard::ShowCharacterPalette, _w: &mut Window, _cx: &mut App) {}
    fn on_mouse_down(&mut self, _position: Point<Pixels>, _w: &mut Window, _cx: &mut App) {}
    fn on_mouse_up(&mut self, _event: &MouseUpEvent, _w: &mut Window, _cx: &mut App) {}
    fn on_mouse_move(&mut self, _event: &MouseMoveEvent, _w: &mut Window, _cx: &mut App) {}
}

#[macro_export]
macro_rules! action_handler {
    ($state:expr, $disabled:expr, $action:ty, $method:ident) => {{
        let state = $state.clone();
        let disabled = $disabled;
        move |action: &$action, window: &mut gpui::Window, cx: &mut gpui::App| {
            if disabled { return; }
            let _ = state.update(cx, |s, app| s.$method(action, window, app));
        }
    }};
}
