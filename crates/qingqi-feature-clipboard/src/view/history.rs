use super::*;
use std::sync::Arc;

use gpui::{UniformListScrollHandle, hsla, uniform_list};
use gpui_component::{
    Icon, IconName, Sizable, Size as ComponentSize,
    scroll::{Scrollbar, ScrollbarShow},
};

pub(super) fn keyboard_filters() -> [ClipboardFilter; 5] {
    [
        ClipboardFilter::All,
        ClipboardFilter::Pinned,
        ClipboardFilter::Text,
        ClipboardFilter::Image,
        ClipboardFilter::Files,
    ]
}

pub(super) fn history_page(
    handle: Entity<ClipboardView>,
    items: Arc<Vec<ClipboardRecord>>,
    selected: usize,
    query: &str,
    current_filter: ClipboardFilter,
    history_scroll: UniformListScrollHandle,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(12.0))
                .pt(px(10.0))
                .pb(px(6.0))
                .child(render_filter_tabs(handle.clone(), current_filter, dark)),
        )
        .child(history_filter_divider(dark))
        .child(div().flex_1().min_h(px(0.0)).px(px(4.0)).pb(px(6.0)).child(
            history_list(
                handle.clone(),
                items,
                selected,
                query,
                history_scroll,
                dark,
            ),
        ))
}

pub(super) fn search_field(query_input: Entity<TextInput>, _dark: bool) -> gpui::Div {
    div()
        .min_w(px(0.0))
        .h(px(30.0))
        .rounded(px(6.0))
        .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.45))
        .px(px(10.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            Icon::new(IconName::Search)
                .with_size(ComponentSize::Small)
                .text_color(theme::semantic().text_placeholder),
        )
        .child(div().flex_1().child(query_input.into_any_element()))
}

pub(super) fn preview_panel(
    selected_record: Option<ClipboardRecord>,
    preview_input: Entity<TextInput>,
    wrap_enabled: bool,
    panel: Entity<ClipboardView>,
    dark: bool,
) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(if let Some(item) = selected_record {
            let kind_label = match item.kind {
                history_store::ClipboardItemKind::Text => {
                    if !item.badge.is_empty() {
                        item.badge.clone()
                    } else {
                        String::from("文本")
                    }
                }
                history_store::ClipboardItemKind::Image => String::from("图片"),
                history_store::ClipboardItemKind::Files => {
                    let count = item.parsed_file_paths().len();
                    if count > 0 {
                        format!("{} 个文件", count)
                    } else {
                        String::from("文件")
                    }
                }
            };
            let kind_color = match item.kind {
                history_store::ClipboardItemKind::Text => match item.badge_kind() {
                    history_store::ClipboardBadgeKind::Link => theme::semantic().success,
                    history_store::ClipboardBadgeKind::Json => {
                        ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue)
                    }
                    history_store::ClipboardBadgeKind::Other => theme::semantic().text_secondary,
                },
                history_store::ClipboardItemKind::Image => theme::semantic().warning,
                history_store::ClipboardItemKind::Files => theme::semantic().danger,
            };

            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_none()
                        .px(px(10.0))
                        .pt(px(14.0))
                        .pb(px(10.0))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .px(px(6.0))
                                .h(px(20.0))
                                .rounded(px(4.0))
                                .bg(theme::rgba_with_alpha(kind_color, 0.12))
                                .flex()
                                .items_center()
                                .text_size(px(10.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(kind_color)
                                .child(kind_label),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::semantic().text_secondary)
                                .child(item.created_at.clone()),
                        )
                        .child(div().flex_1())
                        .child({
                            let panel_toggle = panel.clone();
                            let btn_text = if wrap_enabled { "自动换行" } else { "不换行" };
                            div()
                                .px(px(6.0))
                                .h(px(20.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .text_size(px(10.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme::semantic().text_secondary)
                                .bg(theme::rgba_with_alpha(theme::semantic().text_secondary, 0.08))
                                .cursor_pointer()
                                .hover(|s| s.bg(theme::rgba_with_alpha(
                                    theme::semantic().text_secondary,
                                    0.14,
                                )))
                                .on_mouse_down(gpui::MouseButton::Left, move |_event, _, cx| {
                                    panel_toggle.update(cx, |panel, cx| {
                                        panel.toggle_preview_wrap(cx);
                                    });
                                })
                                .child(btn_text)
                        })
                        .child(if item.pinned {
                            div()
                                .flex()
                                .items_center()
                                .gap(px(3.0))
                                .text_size(px(10.0))
                                .text_color(ui::accent_color(
                                    qingqi_plugin::plugin_spec::PluginAccent::Blue,
                                ))
                                .child(
                                    Icon::new(IconName::Star)
                                        .with_size(ComponentSize::Small)
                                        .text_color(ui::accent_color(
                                            qingqi_plugin::plugin_spec::PluginAccent::Blue,
                                        )),
                                )
                                .child("已置顶")
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .flex_col()
                        .min_h(px(0.0))
                        .child(preview_content(item, preview_input, dark)),
                )
                .into_any_element()
        } else {
            preview_empty(dark).into_any_element()
        })
}

fn history_filter_divider(_dark: bool) -> impl IntoElement {
    div()
        .w_full()
        .mt(px(6.0))
        .mb(px(4.0))
        .h(px(1.0))
        .bg(ui::border_light())
}

fn render_filter_tabs(
    handle: Entity<ClipboardView>,
    active: ClipboardFilter,
    _dark: bool,
) -> impl IntoElement {
    let tabs: Vec<gpui::AnyElement> = keyboard_filters()
        .into_iter()
        .enumerate()
        .map(|(idx, filter)| {
            let is_active = active == filter;
            let h = handle.clone();
            let el = div()
                .id(("clipboard-filter-tab", idx))
                .h(px(24.0))
                .px(px(10.0))
                .rounded(px(5.0))
                .bg(if is_active {
                    theme::rgba_with_alpha(
                        ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue),
                        0.12,
                    )
                } else {
                    theme::rgba_with_alpha(theme::semantic().bg_elevated, 0.35)
                })
                .border_1()
                .border_color(if is_active {
                    gpui::Hsla::from(ui::accent_color(
                        qingqi_plugin::plugin_spec::PluginAccent::Blue,
                    ))
                } else {
                    ui::border_light()
                })
                .text_color(if is_active {
                    ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue)
                } else {
                    theme::semantic().text_secondary
                })
                .text_size(px(10.0))
                .font_weight(if is_active {
                    gpui::FontWeight::SEMIBOLD
                } else {
                    gpui::FontWeight::NORMAL
                })
                .hover(move |style| {
                    style
                        .bg(if !is_active {
                            theme::semantic().bg_hover.into()
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .cursor_pointer()
                })
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    let _ = cx.update_entity(&h, |panel, cx| {
                        panel.set_filter_async(filter, cx);
                        cx.notify();
                    });
                })
                .flex()
                .items_center()
                .justify_center()
                .child(filter.label())
                .into_any_element();
            el
        })
        .collect();

    div().flex().items_center().gap(px(4.0)).children(tabs)
}

fn history_list(
    handle: Entity<ClipboardView>,
    items: Arc<Vec<ClipboardRecord>>,
    selected: usize,
    query: &str,
    scroll: UniformListScrollHandle,
    dark: bool,
) -> impl IntoElement {
    let is_empty = items.is_empty();

    div().size_full().flex().flex_col().child(if is_empty {
        empty_state_text(query, dark, true).into_any_element()
    } else {
        history_virtual_list(handle, items, selected, scroll, dark).into_any_element()
    })
}

fn history_virtual_list(
    handle: Entity<ClipboardView>,
    items: Arc<Vec<ClipboardRecord>>,
    selected: usize,
    scroll: UniformListScrollHandle,
    dark: bool,
) -> impl IntoElement {
    let item_count = items.len();
    div()
        .relative()
        .size_full()
        .child(
            uniform_list(
                "clipboard-history-list",
                item_count,
                move |range, _window, cx| {
                    let _ = cx.update_entity(&handle, |panel, cx| {
                        panel.maybe_prefetch_history(range.end, cx);
                    });
                    range
                        .map(|index| {
                            let item = items[index].clone();
                            history_row(handle.clone(), item, index, index == selected, dark)
                        })
                        .collect::<Vec<_>>()
                },
            )
            .track_scroll(scroll.clone())
            .size_full(),
        )
        .child(Scrollbar::vertical(&scroll).scrollbar_show(ScrollbarShow::Scrolling))
}

fn empty_state_text(query: &str, _dark: bool, is_empty: bool) -> impl IntoElement {
    let message = if is_empty && query.trim().is_empty() {
        "暂无剪贴板记录"
    } else {
        "没有匹配记录"
    };
    let hint = if is_empty && query.trim().is_empty() {
        "复制内容后将自动出现在这里"
    } else {
        "尝试其他关键词或筛选"
    };

    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .child(
            Icon::new(IconName::Copy)
                .with_size(ComponentSize::Large)
                .text_color(theme::semantic().text_placeholder),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::semantic().text_placeholder)
                .child(message),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme::semantic().text_placeholder)
                .child(hint),
        )
}

fn history_row(
    handle: Entity<ClipboardView>,
    item: ClipboardRecord,
    index: usize,
    selected: bool,
    dark: bool,
) -> impl IntoElement {
    let title = history_item_title(&item);
    let meta = history_item_meta(&item);
    let pinned = item.pinned;
    let icon_surface = if selected {
        theme::rgba_with_alpha(
            ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue),
            0.15,
        )
    } else {
        theme::rgba_with_alpha(theme::semantic().bg_elevated, 0.5)
    };
    let row_bg = if selected {
        theme::rgba_with_alpha(
            ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue),
            0.06,
        )
    } else {
        hsla(0.0, 0.0, 0.0, 0.0)
    };
    let pin_handle = handle.clone();
    let delete_handle = handle.clone();

    div()
        .id(("clipboard-row", item.id as u64))
        .w_full()
        .h(px(56.0))
        .flex_none()
        .p(px(6.0))
        .rounded(px(6.0))
        .bg(row_bg)
        .hover(move |style| {
            style
                .bg(if selected {
                    theme::rgba_with_alpha(
                        ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue),
                        0.08,
                    )
                } else {
                    theme::semantic().bg_hover.into()
                })
                .cursor_pointer()
        })
        .on_click(move |event, window, cx| {
            let _ = cx.update_entity(&handle, |panel, cx| {
                panel.select(index, cx);
                if event.click_count() >= 2 {
                    panel.copy_selected(cx);
                }
                cx.notify();
            });
            if event.click_count() >= 2 {
                window.defer(cx, |window, _cx| window.remove_window());
            }
        })
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(history_row_media(&item, icon_surface, dark))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .text_size(px(12.0))
                                .line_height(px(16.0))
                                .line_clamp(1)
                                .text_color(theme::semantic().text_primary)
                                .child(title),
                        )
                        .children(pinned.then(|| {
                            Icon::new(IconName::Star)
                                .with_size(ComponentSize::Small)
                                .text_color(ui::accent_color(
                                    qingqi_plugin::plugin_spec::PluginAccent::Blue,
                                ))
                                .into_any_element()
                        })),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .line_height(px(13.0))
                        .text_color(theme::semantic().text_secondary)
                        .child(meta),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(2.0))
                .child(
                    div()
                        .id(("clipboard-row-pin", index))
                        .size(px(24.0))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .hover(|s| s.bg(theme::semantic().bg_hover).cursor_pointer())
                        .child(
                            Icon::new(if pinned {
                                IconName::Star
                            } else {
                                IconName::StarOff
                            })
                            .with_size(ComponentSize::Small)
                            .text_color(if pinned {
                                theme::semantic().text_primary
                            } else {
                                theme::semantic().text_placeholder
                            }),
                        )
                        .on_click(move |_event, _, cx| {
                            let _ = cx.update_entity(&pin_handle, |panel, cx| {
                                panel.select(index, cx);
                                panel.toggle_selected_pin(cx);
                                cx.notify();
                            });
                            cx.stop_propagation();
                        }),
                )
                .child(
                    div()
                        .id(("clipboard-row-delete", index))
                        .size(px(24.0))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .hover(|s| s.bg(theme::semantic().bg_hover).cursor_pointer())
                        .child(
                            Icon::new(IconName::Delete)
                                .with_size(ComponentSize::Small)
                                .text_color(theme::semantic().text_placeholder),
                        )
                        .on_click(move |_event, _, cx| {
                            let _ = cx.update_entity(&delete_handle, |panel, cx| {
                                panel.select(index, cx);
                                panel.delete_selected(cx);
                                cx.notify();
                            });
                            cx.stop_propagation();
                        }),
                ),
        )
}

fn history_row_media(
    item: &ClipboardRecord,
    icon_surface: gpui::Hsla,
    dark: bool,
) -> impl IntoElement {
    if item.kind == history_store::ClipboardItemKind::Image {
        return div()
            .size(px(36.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(ui::border_light())
            .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.7))
            .overflow_hidden()
            .child(
                img(PathBuf::from(item.content.clone()))
                    .object_fit(ObjectFit::Cover)
                    .size_full()
                    .with_fallback(move || {
                        icon_label("IMG", icon_surface, theme::semantic().warning)
                            .into_any_element()
                    })
                    .into_any_element(),
            )
            .into_any_element();
    }

    icon_label(
        history_item_icon(item),
        icon_surface,
        history_item_icon_color(item, dark),
    )
    .into_any_element()
}

fn icon_label(label: &'static str, surface: gpui::Hsla, color: gpui::Rgba) -> impl IntoElement {
    div()
        .size(px(36.0))
        .rounded(px(6.0))
        .bg(surface)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(color)
        .child(label)
}

fn history_item_title(item: &ClipboardRecord) -> String {
    if item.preview.is_empty() {
        return String::from("(空内容)");
    }
    item.preview.clone()
}

fn history_item_meta(item: &ClipboardRecord) -> String {
    let mut parts = Vec::new();
    if item.pinned {
        parts.push(String::from("已置顶"));
    }
    parts.push(item.created_at.clone());
    parts.push(match item.kind {
        history_store::ClipboardItemKind::Text => {
            let mut labels: Vec<&str> = Vec::new();
            if !item.badge.is_empty() {
                labels.push(item.badge.as_str());
            }
            if history_store::contains_sensitive(&item.content) {
                labels.push("敏感");
            }
            if labels.is_empty() {
                history_store::text_stats(&item.content)
            } else {
                labels.join(" · ")
            }
        }
        history_store::ClipboardItemKind::Image => String::from("图片剪贴板"),
        history_store::ClipboardItemKind::Files => {
            let paths = item.parsed_file_paths();
            if paths.is_empty() {
                String::from("文件列表")
            } else {
                format!("文件 · {} 个", paths.len())
            }
        }
    });
    parts.join(" · ")
}

fn history_item_icon(item: &ClipboardRecord) -> &'static str {
    match item.kind {
        history_store::ClipboardItemKind::Text => match item.badge_kind() {
            history_store::ClipboardBadgeKind::Link => "URL",
            history_store::ClipboardBadgeKind::Json => "JSN",
            history_store::ClipboardBadgeKind::Other => "TXT",
        },
        history_store::ClipboardItemKind::Image => "IMG",
        history_store::ClipboardItemKind::Files => "FIL",
    }
}

fn history_item_icon_color(item: &ClipboardRecord, _dark: bool) -> gpui::Rgba {
    match item.kind {
        history_store::ClipboardItemKind::Text => match item.badge_kind() {
            history_store::ClipboardBadgeKind::Link => theme::semantic().success,
            history_store::ClipboardBadgeKind::Json => {
                ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue)
            }
            history_store::ClipboardBadgeKind::Other => theme::semantic().text_secondary,
        },
        history_store::ClipboardItemKind::Image => theme::semantic().warning,
        history_store::ClipboardItemKind::Files => theme::semantic().danger,
    }
}

fn preview_content(
    item: ClipboardRecord,
    preview_input: Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    match item.kind {
        history_store::ClipboardItemKind::Image => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                img(PathBuf::from(item.content))
                    .object_fit(ObjectFit::Contain)
                    .size_full()
                    .with_fallback(move || {
                        preview_unavailable("图片文件不可用", dark).into_any_element()
                    })
                    .into_any_element(),
            )
            .into_any_element(),
        _ => preview_text(preview_input, dark).into_any_element(),
    }
}

fn preview_text(preview_input: Entity<TextInput>, _dark: bool) -> impl IntoElement {
    div()
        .size_full()
        .pl(px(10.0))
        .pr(px(4.0))
        .pt(px(2.0))
        .pb(px(8.0))
        .text_color(theme::semantic().text_body)
        .child(preview_input)
}

fn preview_empty(_dark: bool) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_col()
        .gap(px(10.0))
        .child(
            Icon::new(IconName::Copy)
                .with_size(ComponentSize::Large)
                .text_color(theme::semantic().text_placeholder),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(theme::semantic().text_placeholder)
                .child("选择一条记录以查看详情"),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::semantic().text_placeholder)
                .child("Ctrl+C 复制 · ↑↓ 导航 · Enter 粘贴"),
        )
}

fn preview_unavailable(message: &'static str, _dark: bool) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(theme::semantic().text_placeholder)
        .child(message)
}
