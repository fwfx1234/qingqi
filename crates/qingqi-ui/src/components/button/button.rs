use gpui::*;
use std::rc::Rc;

use super::variant::*;

pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<String>,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
    variant: ButtonVariant,
    size: ButtonSize,
    custom_variant: Option<ButtonCustomVariant>,
    disabled: bool,
    selected: bool,
    loading: bool,
    tooltip: Option<SharedString>,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            icon: None,
            prefix: None,
            suffix: None,
            variant: ButtonVariant::Primary,
            size: ButtonSize::Medium,
            custom_variant: None,
            disabled: false,
            selected: false,
            loading: false,
            tooltip: None,
            on_click: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self { self.label = Some(label.into()); self }
    pub fn icon(mut self, icon: impl Into<String>) -> Self { self.icon = Some(icon.into()); self }
    pub fn variant(mut self, v: ButtonVariant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: ButtonSize) -> Self { self.size = s; self }
    pub fn custom(mut self, v: ButtonCustomVariant) -> Self { self.custom_variant = Some(v); self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }
    pub fn selected(mut self, s: bool) -> Self { self.selected = s; self }
    pub fn loading(mut self, l: bool) -> Self { self.loading = l; self }
    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = crate::token::tokens(cx);
        let height = button_height(self.size);
        let padding = button_padding(self.size);
        let font_size = button_font_size(self.size);

        let (bg, fg, border) = if let Some(custom) = &self.custom_variant {
            (custom.color, custom.foreground, custom.border)
        } else {
            button_colors(self.variant, cx)
        };

        let mut btn = div()
            .id(self.id.clone())
            .h(height)
            .px(padding.left)
            .flex()
            .items_center()
            .gap_1p5()
            .rounded(px(8.0))
            .bg(if self.selected { token.surface_active } else { bg })
            .border_1()
            .border_color(if self.selected { token.accent } else { border })
            .text_size(font_size)
            .text_color(fg)
            .font_weight(FontWeight::MEDIUM);

        if !self.disabled {
            btn = btn.hover(|s| s.bg(token.surface_hover).cursor_pointer());
        } else {
            btn = btn.opacity(0.5).cursor_not_allowed();
        }

        if let Some(handler) = self.on_click.clone() {
            btn = btn.on_click(move |event, window, cx| {
                handler(event, window, cx);
            });
        }

        if let Some(prefix) = self.prefix {
            btn = btn.child(prefix);
        } else if let Some(icon) = &self.icon {
            btn = btn.child(icon.clone());
        }

        if let Some(label) = &self.label {
            btn = btn.child(label.clone());
        }

        if let Some(suffix) = self.suffix {
            btn = btn.child(suffix);
        }

        btn
    }
}
