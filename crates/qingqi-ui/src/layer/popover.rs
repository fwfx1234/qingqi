use gpui::*;

pub struct Popover {
    pub id: ElementId,
    pub anchor: Corner,
    pub content: Option<Box<dyn Fn(&mut Window, &mut App) -> AnyElement>>,
}

impl Popover {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into(), anchor: Corner::TopLeft, content: None }
    }

    pub fn anchor(mut self, a: Corner) -> Self { self.anchor = a; self }

    pub fn content(mut self, c: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        self.content = Some(Box::new(c));
        self
    }
}
