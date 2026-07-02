use gpui::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement { Top, Bottom, Left, Right }

#[derive(Clone)]
pub struct Sheet {
    pub placement: Placement,
    pub size: Pixels,
    pub title: Option<SharedString>,
}

impl Sheet {
    pub fn new(placement: Placement) -> Self {
        Self { placement, size: px(400.0), title: None }
    }
    pub fn size(mut self, s: Pixels) -> Self { self.size = s; self }
    pub fn title(mut self, t: impl Into<SharedString>) -> Self { self.title = Some(t.into()); self }
}

#[derive(Clone)]
pub struct ActiveSheet {
    pub sheet: Sheet,
    pub focus_handle: FocusHandle,
}
