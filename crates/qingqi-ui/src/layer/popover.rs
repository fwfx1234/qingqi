use gpui::*;

pub struct Popover {
    pub id: ElementId,
    pub anchor: Corner,
}

impl Popover {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into(), anchor: Corner::TopLeft }
    }
    pub fn anchor(mut self, a: Corner) -> Self { self.anchor = a; self }
}
