use gpui::*;

pub struct Popover {
    pub id: ElementId,
    pub anchor: Corner,
    pub content: Option<Box<dyn Fn(&mut AnyElement, &mut Window, &mut App) -> AnyElement>>,
    pub open: bool,
    pub on_open_change: Option<Box<dyn Fn(&bool, &mut Window, &mut App)>>,
    pub trigger: Option<Box<dyn FnOnce(&mut Window, &mut App) -> AnyElement>>,
    pub appearance_value: bool,
}

impl Popover {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into(), anchor: Corner::TopLeft, content: None, open: false, on_open_change: None, trigger: None, appearance_value: true }
    }

    pub fn anchor(mut self, a: Corner) -> Self { self.anchor = a; self }

    pub fn content<R>(mut self, c: impl Fn(&mut AnyElement, &mut Window, &mut App) -> R + 'static) -> Self
    where R: IntoElement + 'static {
        self.content = Some(Box::new(move |s, w, c2| c(s, w, c2).into_any_element()));
        self
    }

    pub fn appearance(mut self, appearance: bool) -> Self { self.appearance_value = appearance; self }

    pub fn open(mut self, open: bool) -> Self { self.open = open; self }

    pub fn on_open_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_change = Some(Box::new(f));
        self
    }

    pub fn trigger(mut self, t: impl IntoElement + 'static) -> Self {
        self.trigger = Some(Box::new(move |_, _| t.into_any_element()));
        self
    }
}

impl IntoElement for Popover {
    type Element = gpui::AnyElement;
    fn into_element(self) -> Self::Element {
        div().id(self.id).into_any_element()
    }
}
