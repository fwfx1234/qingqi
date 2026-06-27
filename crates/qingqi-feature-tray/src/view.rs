//! 网速托盘弹窗视图。

use std::sync::{Arc, Mutex, mpsc::Receiver};

use gpui::{
    AnyElement, App, ClipboardItem, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Styled, Window, div, hsla, px,
};
use qingqi_plugin::tray::TrayPopupView;
use qingqi_ui::ui::glass;

use crate::{
    model::{NetworkSpeedPopupModel, popup_model},
    service::NetworkSpeedService,
};

pub struct NetworkSpeedPopupView {
    service: Arc<NetworkSpeedService>,
    copied_label: Arc<Mutex<Option<String>>>,
}

impl NetworkSpeedPopupView {
    pub fn new(service: Arc<NetworkSpeedService>) -> Self {
        Self {
            service,
            copied_label: Arc::new(Mutex::new(None)),
        }
    }
}

impl TrayPopupView for NetworkSpeedPopupView {
    fn title(&self) -> Arc<str> {
        "网速详情".into()
    }

    fn subscribe_updates(&mut self) -> Option<Receiver<()>> {
        Some(self.service.subscribe_updates())
    }

    fn render(&mut self, _window: &mut Window, cx: &mut App) -> AnyElement {
        let model = popup_model(
            &self.service.settings(),
            &self.service.snapshot(),
            self.service.public_ip().as_deref(),
            self.service.local_ip().as_deref(),
        );
        let copied_label = self
            .copied_label
            .lock()
            .ok()
            .and_then(|label| label.clone());
        render_popup(
            &model,
            copied_label.as_deref(),
            Arc::clone(&self.copied_label),
            cx,
        )
    }
}

fn render_popup(
    model: &NetworkSpeedPopupModel,
    copied_label: Option<&str>,
    copied_state: Arc<Mutex<Option<String>>>,
    app: &App,
) -> AnyElement {
    let glass_bg = glass::bg(app);
    let panel_border = glass::border(app);
    let divider = glass::divider(app);
    let hover_bg = glass::hover_bg(app);
    let primary_text = hsla(215.0 / 360.0, 0.16, 0.12, 0.96);
    let secondary_text = hsla(215.0 / 360.0, 0.12, 0.36, 0.78);
    let muted_text = hsla(215.0 / 360.0, 0.10, 0.45, 0.62);
    let live_bg = hsla(210.0 / 360.0, 0.80, 0.60, 0.16);

    div()
        .size_full()
        .rounded(px(18.0))
        .bg(glass_bg)
        .border_1()
        .border_color(panel_border)
        .shadow(glass::shadow())
        .overflow_hidden()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .child(header(model, muted_text, live_bg))
                .child(
                    div()
                        .grid()
                        .grid_cols(2)
                        .gap_2()
                        .child(rate_card("上传", "↑", model.upload_rate.clone()))
                        .child(rate_card("下载", "↓", model.download_rate.clone())),
                )
                .child(rows(
                    model,
                    copied_label,
                    divider,
                    primary_text,
                    secondary_text,
                    hover_bg,
                    copied_state,
                )),
        )
        .into_any_element()
}

fn header(
    model: &NetworkSpeedPopupModel,
    muted_text: gpui::Hsla,
    live_bg: gpui::Hsla,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .mb_1()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size(px(9.0))
                        .rounded(px(999.0))
                        .bg(hsla(156.0 / 360.0, 0.70, 0.52, 0.95))
                        .shadow(vec![gpui::BoxShadow {
                            color: hsla(156.0 / 360.0, 0.70, 0.52, 0.24),
                            offset: gpui::point(px(0.0), px(0.0)),
                            blur_radius: px(12.0),
                            spread_radius: px(2.0),
                        }]),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(muted_text)
                                .child(model.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(gpui::FontWeight::NORMAL)
                                .line_height(px(16.0))
                                .child(model.subtitle.clone()),
                        ),
                ),
        )
        .child(
            div()
                .h(px(26.0))
                .px_3()
                .rounded(px(999.0))
                .bg(live_bg)
                .flex()
                .items_center()
                .gap_2()
                .font_family("monospace")
                .text_size(px(11.0))
                .text_color(muted_text)
                .child(div().size(px(6.0)).rounded(px(999.0)).bg(hsla(
                    148.0 / 360.0,
                    0.74,
                    0.56,
                    0.92,
                )))
                .child("LIVE"),
        )
}

fn rate_card(label: &'static str, icon: &'static str, value: String) -> impl IntoElement {
    let primary_text = hsla(215.0 / 360.0, 0.20, 0.14, 0.94);
    let secondary_text = hsla(215.0 / 360.0, 0.12, 0.36, 0.76);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .rounded(px(18.0))
        .bg(hsla(0.0, 0.0, 1.0, 0.08))
        .border_1()
        .border_color(hsla(0.0, 0.0, 1.0, 0.12))
        .px_3()
        .py_2()
        .child(
            div().flex().items_center().justify_between().gap_2().child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size(px(22.0))
                            .rounded(px(999.0))
                            .bg(hsla(0.0, 0.0, 1.0, 0.16))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::NORMAL)
                            .text_color(secondary_text)
                            .child(icon),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(secondary_text)
                            .child(label),
                    ),
            ),
        )
        .child(
            div()
                .font_family("monospace")
                .text_size(px(16.0))
                .line_height(px(17.0))
                .font_weight(gpui::FontWeight::NORMAL)
                .text_color(primary_text)
                .child(value),
        )
}

#[allow(clippy::too_many_arguments)]
fn rows(
    model: &NetworkSpeedPopupModel,
    copied_label: Option<&str>,
    divider: gpui::Hsla,
    primary_text: gpui::Hsla,
    secondary_text: gpui::Hsla,
    hover_bg: gpui::Hsla,
    copied_state: Arc<Mutex<Option<String>>>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_0()
        .rounded(px(18.0))
        .bg(hsla(0.0, 0.0, 1.0, 0.08))
        .border_1()
        .border_color(divider)
        .overflow_hidden()
        .children(model.rows.iter().map(move |row| {
            let copy_value = row.copy_value.clone();
            let row_label = row.label.clone();
            let is_copied = copied_label == Some(row.label.as_str());
            let display_value = if is_copied {
                String::from("已复制")
            } else {
                row.value.clone()
            };
            let mut row_el = div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(divider)
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(secondary_text)
                        .child(row.label.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family("monospace")
                        .font_weight(gpui::FontWeight::NORMAL)
                        .text_color(if is_copied {
                            hsla(156.0 / 360.0, 0.62, 0.36, 0.98)
                        } else {
                            primary_text
                        })
                        .child(display_value),
                );
            if let Some(copy_value) = copy_value {
                let copied_state = Arc::clone(&copied_state);
                let label = row_label.clone();
                row_el = row_el
                    .hover(move |style| style.bg(hover_bg))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy_value.clone()));
                        if let Ok(mut copied_label) = copied_state.lock() {
                            *copied_label = Some(label.clone());
                        }
                        window.refresh();
                    });
            }
            row_el
        }))
}
