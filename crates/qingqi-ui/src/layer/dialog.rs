use gpui::*;
use std::rc::Rc;

#[derive(Clone)]
pub struct Dialog {
    pub title: Option<SharedString>,
    pub content: Option<Rc<AnyElement>>,
    pub width: Pixels,
    pub overlay: bool,
    pub overlay_closable: bool,
}

impl Dialog {
    pub fn new() -> Self {
        Self {
            title: None,
            content: None,
            width: px(400.0),
            overlay: true,
            overlay_closable: true,
        }
    }

    pub fn title(mut self, t: impl Into<SharedString>) -> Self { self.title = Some(t.into()); self }
    pub fn content(mut self, c: impl IntoElement) -> Self { self.content = Some(Rc::new(c.into_any_element())); self }
    pub fn width(mut self, w: Pixels) -> Self { self.width = w; self }
}

#[derive(Clone)]
pub struct ActiveDialog {
    pub dialog: Dialog,
    pub focus_handle: FocusHandle,
}
