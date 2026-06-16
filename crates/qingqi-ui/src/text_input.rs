//! 基于 `gpui-component::input` 的文本输入封装，样式对齐 Qingqi UI。

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, Hsla, IntoElement, Render,
    SharedString, Styled, Subscription, Window, px,
};
use gpui_component::input::{Input, InputEvent, InputState, SelectAll};

use crate::ui;

#[derive(Clone, Copy, Debug)]
pub struct TextInputStyle {
    pub height: f32,
    pub font_size: f32,
    pub padding: f32,
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            height: 38.0,
            font_size: 13.0,
            padding: 8.0,
        }
    }
}

/// 包装 `gpui-component` 输入框，保留 Qingqi 侧常用配置 API。
pub struct TextInput {
    focus_handle: FocusHandle,
    state: Option<Entity<InputState>>,
    placeholder: SharedString,
    cached_text: String,
    draw_chrome: bool,
    style: TextInputStyle,
    multiline: bool,
    read_only: bool,
    monospace: bool,
    fill_height: bool,
    soft_wrap: bool,
    pending_soft_wrap: Option<bool>,
    text_color: Option<Hsla>,
    placeholder_color: Option<Hsla>,
    pending_value: Option<String>,
    pending_placeholder: Option<SharedString>,
    pending_select_all: bool,
    pending_recreate: bool,
    change_subscription: Option<Subscription>,
}

impl TextInput {
    pub fn new(
        cx: &mut Context<Self>,
        placeholder: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            state: None,
            placeholder: placeholder.into(),
            cached_text: value.into().to_string(),
            draw_chrome: true,
            style: TextInputStyle::default(),
            multiline: false,
            read_only: false,
            monospace: false,
            fill_height: false,
            soft_wrap: true,
            pending_soft_wrap: None,
            text_color: None,
            placeholder_color: None,
            pending_value: None,
            pending_placeholder: None,
            pending_select_all: false,
            pending_recreate: true,
            change_subscription: None,
        }
    }

    /// 键盘绑定由 `gpui_component::init` 注册，此处保留兼容调用。
    pub fn register_bindings(_cx: &mut App) {}

    pub fn state(&self) -> Option<&Entity<InputState>> {
        self.state.as_ref()
    }

    pub fn set_style(&mut self, style: TextInputStyle, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    pub fn set_multiline(&mut self, multiline: bool, cx: &mut Context<Self>) {
        if self.multiline == multiline {
            return;
        }
        self.multiline = multiline;
        self.pending_recreate = true;
        cx.notify();
    }

    pub fn set_chrome(&mut self, draw_chrome: bool, cx: &mut Context<Self>) {
        self.draw_chrome = draw_chrome;
        cx.notify();
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let placeholder = placeholder.into();
        self.placeholder = placeholder.clone();
        self.pending_placeholder = Some(placeholder);
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    pub fn set_monospace(&mut self, monospace: bool, cx: &mut Context<Self>) {
        self.monospace = monospace;
        cx.notify();
    }

    pub fn set_fill_height(&mut self, fill: bool, cx: &mut Context<Self>) {
        self.fill_height = fill;
        cx.notify();
    }

    pub fn set_soft_wrap(&mut self, wrap: bool, cx: &mut Context<Self>) {
        self.soft_wrap = wrap;
        self.pending_soft_wrap = Some(wrap);
        cx.notify();
    }

    pub fn set_text_colors(
        &mut self,
        text_color: impl Into<Hsla>,
        placeholder_color: impl Into<Hsla>,
        cx: &mut Context<Self>,
    ) {
        self.text_color = Some(text_color.into());
        self.placeholder_color = Some(placeholder_color.into());
        cx.notify();
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        let text = text.into();
        self.cached_text = text.clone();
        self.pending_value = Some(text);
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text(String::new(), cx);
    }

    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        self.pending_select_all = true;
        cx.notify();
    }

    pub fn text(&self) -> String {
        self.cached_text.clone()
    }

    /// 读取输入框当前值；子窗口渲染时优先从 `InputState` 取值，避免 `cached_text` 滞后。
    pub fn current_text(&self, cx: &App) -> String {
        self.state
            .as_ref()
            .map(|state| state.read(cx).value().to_string())
            .unwrap_or_else(|| self.cached_text.clone())
    }

    fn ensure_state(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if self.pending_recreate || self.state.is_none() {
            let text = self.cached_text.clone();
            let placeholder = self.placeholder.clone();
            let multiline = self.multiline;
            let state = cx.new(|cx| {
                let state = InputState::new(window, cx)
                    .placeholder(placeholder)
                    .default_value(text);
                if multiline {
                    state.multi_line(true)
                } else {
                    state
                }
            });
            self.state = Some(state.clone());
            self.pending_recreate = false;
            self.change_subscription = None;
            self.ensure_change_subscription(cx);
            state
        } else {
            self.ensure_change_subscription(cx);
            self.state.clone().expect("input state initialized")
        }
    }

    fn ensure_change_subscription(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        if self.change_subscription.is_some() {
            return;
        }
        self.change_subscription = Some(cx.subscribe(&state, |this, state, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.cached_text = state.read(cx).value().to_string();
                cx.notify();
            }
        }));
    }

    fn sync_pending(
        &mut self,
        state: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(placeholder) = self.pending_placeholder.take() {
            state.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx);
            });
        }
        if let Some(value) = self.pending_value.take() {
            self.cached_text = value.clone();
            state.update(cx, |input, cx| {
                input.set_value(value, window, cx);
            });
        }
        if self.pending_select_all {
            self.pending_select_all = false;
            let focus = state.read(cx).focus_handle(cx);
            window.focus(&focus);
            window.dispatch_action(Box::new(SelectAll), cx);
        }
        if let Some(wrap) = self.pending_soft_wrap.take() {
            state.update(cx, |input, cx| {
                input.set_soft_wrap(wrap, window, cx);
            });
        }
    }

    fn build_input(&self, state: &Entity<InputState>) -> Input {
        let style = self.style;
        let mut input = Input::new(state).w_full().h_full();

        input = input
            .appearance(self.draw_chrome)
            .bordered(self.draw_chrome)
            .focus_bordered(self.draw_chrome)
            .disabled(self.read_only)
            .text_size(px(style.font_size));

        if self.draw_chrome {
            input = input
                .bg(ui::bg_surface())
                .rounded(px(8.0))
                .border_color(ui::border_light());
        }

        if self.monospace {
            input = input.font_family("Menlo");
        }

        if let Some(color) = self.text_color {
            input = input.text_color(color);
        }

        if style.padding > 0.0 {
            let pad = px(style.padding);
            input = input.px(pad).py(pad);
        }

        let _ = self.placeholder_color;
        input
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state
            .as_ref()
            .map(|state| state.read(cx).focus_handle(cx))
            .unwrap_or_else(|| self.focus_handle.clone())
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.ensure_state(window, cx);
        self.sync_pending(&state, window, cx);
        let mut input = self.build_input(&state).w_full();
        if self.fill_height {
            input = input.h_full();
        } else {
            let height = if self.multiline {
                self.style.height.max(40.0)
            } else {
                self.style.height
            };
            input = input.h(px(height));
        }
        input
    }
}
