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
    query_input: Entity<TextInput>,
    selected_record: Option<ClipboardRecord>,
    item_count: usize,
    current_filter: ClipboardFilter,
    status_text: String,
    history_scroll: UniformListScrollHandle,
    preview_input: Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .overflow_hidden()
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .h_full()
                .flex()
                .flex_col()
                .bg(theme::semantic().bg_subtle)
                .child(history_top_bar(
                    handle.clone(),
                    query_input,
                    item_count,
                    dark,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.0))
                        .flex()
                        .flex_col()
                        .child(div().px(px(16.0)).pt(px(16.0)).child(render_filter_tabs(
                            handle.clone(),
                            current_filter,
                            dark,
                        )))
                        .child(history_filter_divider(dark))
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(0.0))
                                .px(px(12.0))
                                .pb(px(14.0))
                                .child(history_list(
                                    handle.clone(),
                                    items,
                                    selected,
                                    query,
                                    history_scroll,
                                    dark,
                                )),
                        ),
                ),
        )
        .child(
            div()
                .w(px(430.0))
                .min_w(px(430.0))
                .h_full()
                .border_l_1()
                .border_color(theme::semantic().border_default)
                .bg(theme::semantic().bg_page)
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.0))
                        .px(px(24.0))
                        .pt(px(24.0))
                        .pb(px(14.0))
                        .child(detail_panel(
                            handle,
                            selected_record,
                            status_text,
                            preview_input,
                            dark,
                        )),
                ),
        )
}

fn history_top_bar(
    handle: Entity<ClipboardView>,
    query_input: Entity<TextInput>,
    item_count: usize,
    dark: bool,
) -> impl IntoElement {
    div()
        .h(px(62.0))
        .px(px(18.0))
        .border_b_1()
        .border_color(theme::semantic().border_default)
        .bg(theme::semantic().bg_page)
        .flex()
        .items_center()
        .child(div().w(px(220.0)))
        .child(
            div()
                .flex_1()
                .flex()
                .justify_center()
                .child(search_field(query_input, dark).w(px(326.0))),
        )
        .child(
            div()
                .w(px(220.0))
                .flex()
                .items_center()
                .justify_end()
                .gap(px(10.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(theme::semantic().text_secondary)
                        .child(format!("{item_count} 条")),
                )
                .child(top_bar_icon_button(handle, dark)),
        )
}

fn top_bar_icon_button(handle: Entity<ClipboardView>, _dark: bool) -> impl IntoElement {
    div()
        .id("clipboard-open-settings")
        .size(px(30.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::semantic().border_default)
        .bg(theme::semantic().bg_elevated)
        .flex()
        .items_center()
        .justify_center()
        .hover(|style| style.bg(theme::semantic().row_hover).cursor_pointer())
        .child(
            Icon::new(IconName::Settings)
                .with_size(ComponentSize::Small)
                .text_color(theme::semantic().text_secondary),
        )
        .on_click(move |_, _, cx| {
            let _ = cx.update_entity(&handle, |panel, cx| {
                panel.set_tab(ClipboardTab::Settings);
                cx.notify();
            });
        })
}

fn history_filter_divider(_dark: bool) -> impl IntoElement {
    div()
        .w_full()
        .mt(px(14.0))
        .mb(px(10.0))
        .h(px(1.0))
        .bg(theme::semantic().border_default)
}

fn search_field(query_input: Entity<TextInput>, _dark: bool) -> gpui::Div {
    div()
        .min_w(px(0.0))
        .h(px(32.0))
        .rounded(px(9.0))
        .border_1()
        .border_color(theme::semantic().border_default)
        .bg(theme::semantic().bg_elevated)
        .px(px(10.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            Icon::new(IconName::Search)
                .with_size(ComponentSize::Medium)
                .text_color(theme::semantic().text_placeholder),
        )
        .child(div().flex_1().child(query_input.into_any_element()))
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
                .h(px(26.0))
                .px(px(10.0))
                .rounded(px(6.0))
                .bg(if is_active {
                    theme::rgba_with_alpha(
                        ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue),
                        0.12,
                    )
                } else {
                    theme::semantic().bg_elevated.into()
                })
                .border_1()
                .border_color(if is_active {
                    ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue)
                } else {
                    theme::semantic().border_default
                })
                .text_color(if is_active {
                    ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue)
                } else {
                    theme::semantic().text_secondary
                })
                .text_size(px(12.0))
                .font_weight(if is_active {
                    gpui::FontWeight::SEMIBOLD
                } else {
                    gpui::FontWeight::NORMAL
                })
                .hover(move |style| {
                    style
                        .bg(if !is_active {
                            theme::semantic().row_hover.into()
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

    div().flex().items_center().gap(px(6.0)).children(tabs)
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
    let (title, subtitle) = if is_empty && query.trim().is_empty() {
        ("暂无剪贴板记录", "复制一段文本后，这里会开始积累历史")
    } else {
        ("没有匹配记录", "换个关键词，或者切换到其他筛选标签")
    };

    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .child(
            div()
                .size(px(48.0))
                .rounded(px(12.0))
                .bg(theme::rgba_with_alpha(
                    ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue),
                    0.10,
                ))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(16.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(ui::accent_color(
                    qingqi_plugin::plugin_spec::PluginAccent::Blue,
                ))
                .child("空"),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic().text_primary)
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::semantic().text_secondary)
                .child(subtitle),
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
    let subtitle = history_item_meta(&item);
    let pinned = item.pinned;
    let icon_surface = if selected {
        theme::rgba_with_alpha(
            ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue),
            0.12,
        )
    } else {
        theme::semantic().bg_elevated.into()
    };
    let row_bg = if selected {
        theme::rgba_with_alpha(
            ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue),
            0.06,
        )
    } else {
        hsla(0.0, 0.0, 0.0, 0.0)
    };
    let title_color = theme::semantic().text_primary;
    let pin_handle = handle.clone();
    let delete_handle = handle.clone();

    div()
        .id(("clipboard-row", item.id as u64))
        .w_full()
        .h(px(86.0))
        .flex_none()
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(row_bg)
        .hover(move |style| {
            style
                .bg(if selected {
                    theme::rgba_with_alpha(
                        ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue),
                        0.08,
                    )
                } else {
                    theme::semantic().row_hover.into()
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
        .gap(px(10.0))
        .child(history_row_media(&item, icon_surface, dark))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(px(18.0))
                        .line_clamp(2)
                        .text_color(title_color)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .line_height(px(15.0))
                        .text_color(theme::semantic().text_secondary)
                        .child(subtitle),
                ),
        )
        .child(row_icon_button(
            if pinned {
                IconName::Star
            } else {
                IconName::StarOff
            },
            "clipboard-row-pin",
            dark,
            move |_, _, cx| {
                let _ = cx.update_entity(&pin_handle, |panel, cx| {
                    panel.select(index, cx);
                    panel.toggle_selected_pin(cx);
                    cx.notify();
                });
            },
        ))
        .child(row_icon_button(
            IconName::Delete,
            "clipboard-row-delete",
            dark,
            move |_, _, cx| {
                let _ = cx.update_entity(&delete_handle, |panel, cx| {
                    panel.select(index, cx);
                    panel.delete_selected(cx);
                    cx.notify();
                });
            },
        ))
}

fn row_icon_button(
    icon: IconName,
    id: &'static str,
    _dark: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .size(px(28.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::semantic().border_default)
        .bg(theme::semantic().bg_elevated)
        .flex()
        .items_center()
        .justify_center()
        .hover(|style| style.bg(theme::semantic().row_hover).cursor_pointer())
        .child(
            Icon::new(icon)
                .with_size(ComponentSize::Small)
                .text_color(theme::semantic().text_secondary),
        )
        .on_click(move |event, window, cx| {
            on_click(event, window, cx);
            cx.stop_propagation();
        })
}

fn history_row_media(
    item: &ClipboardRecord,
    icon_surface: gpui::Hsla,
    dark: bool,
) -> impl IntoElement {
    if item.kind == history_store::ClipboardItemKind::Image {
        return div()
            .size(px(42.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(theme::semantic().border_default)
            .bg(theme::semantic().bg_elevated)
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
        .size(px(42.0))
        .rounded(px(8.0))
        .bg(surface)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
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

fn preview_text_for_record(item: &ClipboardRecord) -> String {
    match item.kind {
        history_store::ClipboardItemKind::Files => {
            let paths = item.parsed_file_paths();
            if paths.is_empty() {
                item.preview.clone()
            } else {
                paths.join("\n")
            }
        }
        _ => item.content.clone(),
    }
}

fn preview_text(preview_input: Entity<TextInput>, _dark: bool) -> impl IntoElement {
    div()
        .size_full()
        .px(px(14.0))
        .py(px(12.0))
        .text_color(theme::semantic().text_regular)
        .child(preview_input)
}

fn preview_empty(dark: bool) -> impl IntoElement {
    preview_unavailable("选择一条记录", dark)
}

fn preview_unavailable(message: &'static str, _dark: bool) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(theme::semantic().text_placeholder)
        .child(message)
}

fn detail_panel(
    handle: Entity<ClipboardView>,
    selected_record: Option<ClipboardRecord>,
    _status_text: String,
    preview_input: Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    let has_selected = selected_record.is_some();
    let is_pinned = selected_record.as_ref().is_some_and(|item| item.pinned);

    let panel = div().size_full().flex().flex_col().gap(px(10.0)).child(
        div()
            .flex_1()
            .min_h(px(0.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(theme::semantic().border_default)
            .bg(theme::semantic().bg_surface)
            .overflow_hidden()
            .child(
                selected_record
                    .map(|item| preview_content(item, preview_input, dark).into_any_element())
                    .unwrap_or_else(|| preview_empty(dark).into_any_element()),
            ),
    );

    if has_selected {
        panel.child(detail_actions(handle, is_pinned, dark))
    } else {
        panel
    }
}

fn detail_actions(handle: Entity<ClipboardView>, is_pinned: bool, dark: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(detail_action_button("复制", dark, {
            let handle = handle.clone();
            move |_, _, cx| {
                let _ = cx.update_entity(&handle, |panel, cx| {
                    panel.copy_selected(cx);
                    cx.notify();
                });
            }
        }))
        .child(detail_action_button(
            if is_pinned { "取消置顶" } else { "置顶" },
            dark,
            {
                let handle = handle.clone();
                move |_, _, cx| {
                    let _ = cx.update_entity(&handle, |panel, cx| {
                        panel.toggle_selected_pin(cx);
                        cx.notify();
                    });
                }
            },
        ))
        .child(detail_action_button("删除", dark, {
            let handle = handle.clone();
            move |_, _, cx| {
                let _ = cx.update_entity(&handle, |panel, cx| {
                    panel.delete_selected(cx);
                    cx.notify();
                });
            }
        }))
}

fn detail_action_button(
    label: &'static str,
    _dark: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(label)
        .h(px(28.0))
        .px(px(12.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::semantic().border_default)
        .bg(theme::semantic().bg_surface)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(theme::semantic().text_primary)
        .hover(|style| style.bg(theme::semantic().row_hover).cursor_pointer())
        .child(label)
        .on_click(on_click)
}
