use super::shared::header_action_button;
use super::*;
use std::rc::Rc;

use gpui::{ListSizingBehavior, Size as GpuiSize};
use gpui_component::{Icon, IconName, VirtualListScrollHandle, scroll::Scrollbar, v_virtual_list};

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
    handle: Entity<ClipboardPanel>,
    items: Vec<ClipboardRecord>,
    selected: usize,
    query: &str,
    query_input: Entity<TextInput>,
    selected_record: Option<ClipboardRecord>,
    item_count: usize,
    status_text: String,
    history_scroll: VirtualListScrollHandle,
    preview_file_scroll: VirtualListScrollHandle,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_h(px(0.0))
        .relative()
        .overflow_hidden()
        .child(
            div()
                .relative()
                .size_full()
                .flex()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex()
                        .flex_col()
                        .child(history_left_header(
                            handle.clone(),
                            item_count,
                            query_input,
                            status_text.clone(),
                            dark,
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(0.0))
                                .px(px(8.0))
                                .pt(px(4.0))
                                .pb(px(10.0))
                                .child(history_list(
                                    handle.clone(),
                                    items,
                                    selected,
                                    query,
                                    history_scroll,
                                    dark,
                                )),
                        ),
                )
                .child(detail_panel(
                    handle,
                    selected_record,
                    status_text,
                    preview_file_scroll,
                    dark,
                )),
        )
}

fn history_left_header(
    handle: Entity<ClipboardPanel>,
    item_count: usize,
    query_input: Entity<TextInput>,
    status_text: String,
    dark: bool,
) -> impl IntoElement {
    div()
        .pt(px(14.0))
        .pb(px(12.0))
        .px(px(16.0))
        .border_b_1()
        .border_color(theme::token("color-border-default", dark))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .h(px(32.0))
                        .rounded(px(9.0))
                        .border_1()
                        .border_color(theme::token("color-border-default", dark))
                        .bg(theme::token("color-bg-elevated", dark))
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            Icon::new(IconName::Search)
                                .size_4()
                                .text_color(theme::token("color-text-placeholder", dark)),
                        )
                        .child(div().flex_1().child(query_input.into_any_element())),
                )
                .child(header_action_button(
                    "clipboard-open-settings",
                    dark,
                    "⚙",
                    {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |panel, cx| {
                                panel.set_tab(ClipboardTab::Settings);
                                cx.notify();
                            });
                        }
                    },
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::token("color-text-secondary", dark))
                        .child(status_text),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::token("color-text-placeholder", dark))
                        .child(format!("{item_count} 条")),
                ),
        )
}

fn history_list(
    handle: Entity<ClipboardPanel>,
    items: Vec<ClipboardRecord>,
    selected: usize,
    query: &str,
    scroll: VirtualListScrollHandle,
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
    handle: Entity<ClipboardPanel>,
    items: Vec<ClipboardRecord>,
    selected: usize,
    scroll: VirtualListScrollHandle,
    dark: bool,
) -> impl IntoElement {
    let item_count = items.len();
    let item_sizes = Rc::new(vec![GpuiSize::new(px(0.0), px(73.0)); item_count]);
    div()
        .relative()
        .size_full()
        .child(
            v_virtual_list(
                handle.clone(),
                "clipboard-history-list",
                item_sizes,
                move |_, range, _, _| {
                    range
                        .map(|index| {
                            let item = items[index].clone();
                            history_row(handle.clone(), item, index, index == selected, dark)
                        })
                        .collect::<Vec<_>>()
                },
            )
            .track_scroll(&scroll)
            .with_sizing_behavior(ListSizingBehavior::Infer)
            .size_full(),
        )
        .child(Scrollbar::vertical(&scroll))
}

fn empty_state_text(query: &str, dark: bool, is_empty: bool) -> impl IntoElement {
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
                .bg(theme::rgba_with_alpha(theme::launcher_accent(dark), 0.10))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(16.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::launcher_accent(dark))
                .child("空"),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::token("color-text-primary", dark))
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::token("color-text-secondary", dark))
                .child(subtitle),
        )
}

fn history_row(
    handle: Entity<ClipboardPanel>,
    item: ClipboardRecord,
    index: usize,
    selected: bool,
    dark: bool,
) -> impl IntoElement {
    let title = history_item_title(&item);
    let subtitle = history_item_meta(&item);
    let icon_surface = if selected {
        theme::rgba_with_alpha(theme::launcher_accent(dark), 0.12)
    } else {
        theme::token("color-bg-elevated", dark).into()
    };
    let row_bg = if selected {
        theme::rgba_with_alpha(theme::launcher_accent(dark), 0.06)
    } else {
        hsla(0.0, 0.0, 0.0, 0.0)
    };
    let title_color = theme::token("color-text-primary", dark);

    div()
        .id(("clipboard-row", item.id as u64))
        .w_full()
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(row_bg)
        .hover(move |style| {
            style
                .bg(if selected {
                    theme::rgba_with_alpha(theme::launcher_accent(dark), 0.08)
                } else {
                    theme::token("color-row-hover", dark).into()
                })
                .cursor_pointer()
        })
        .on_click(move |event, window, cx| {
            let _ = cx.update_entity(&handle, |panel, cx| {
                panel.select(index);
                if event.click_count() >= 2 {
                    panel.copy_selected(cx);
                }
                cx.notify();
            });
            if event.click_count() >= 2 {
                window.remove_window();
            }
        })
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .size(px(32.0))
                .rounded(px(8.0))
                .bg(icon_surface)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(history_item_icon_color(&item, dark))
                .child(history_item_icon(&item)),
        )
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
                        .text_color(theme::token("color-text-secondary", dark))
                        .child(subtitle),
                ),
        )
        .when(item.pinned, |el| {
            el.child(
                div()
                    .px(px(6.0))
                    .h(px(18.0))
                    .rounded(px(4.0))
                    .bg(theme::rgba_with_alpha(
                        theme::accent_color(theme::ThemeAccent::Amber),
                        0.12,
                    ))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(9.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme::accent_color(theme::ThemeAccent::Amber))
                    .child("PIN"),
            )
        })
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
            let badges = history_store::text_badges(&item.content);
            if badges.is_empty() {
                history_store::text_stats(&item.content)
            } else {
                badges.join(" · ")
            }
        }
        history_store::ClipboardItemKind::Image => String::from("图片剪贴板"),
        history_store::ClipboardItemKind::Files => {
            let paths = history_store::parse_file_paths(&item.content);
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
        history_store::ClipboardItemKind::Text => {
            if item.badge == "链接" {
                "URL"
            } else if item.badge == "JSON" {
                "JSN"
            } else {
                "TXT"
            }
        }
        history_store::ClipboardItemKind::Image => "IMG",
        history_store::ClipboardItemKind::Files => "FIL",
    }
}

fn history_item_icon_color(item: &ClipboardRecord, dark: bool) -> gpui::Rgba {
    match item.kind {
        history_store::ClipboardItemKind::Text => {
            if item.badge == "链接" {
                theme::token("color-success", dark)
            } else if item.badge == "JSON" {
                theme::launcher_accent(dark)
            } else {
                theme::token("color-text-secondary", dark)
            }
        }
        history_store::ClipboardItemKind::Image => theme::token("color-warning", dark),
        history_store::ClipboardItemKind::Files => theme::token("color-danger", dark),
    }
}

fn detail_preview_card(
    handle: Entity<ClipboardPanel>,
    item: ClipboardRecord,
    preview_file_scroll: VirtualListScrollHandle,
    dark: bool,
) -> impl IntoElement {
    match item.kind {
        history_store::ClipboardItemKind::Image => {
            image_detail_preview(item, dark).into_any_element()
        }
        history_store::ClipboardItemKind::Files => {
            file_detail_preview(handle, item, preview_file_scroll, dark).into_any_element()
        }
        _ => text_detail_preview(item, dark).into_any_element(),
    }
}

fn image_detail_preview(item: ClipboardRecord, dark: bool) -> impl IntoElement {
    let badges = image_detail_badges(&item);
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(image_preview_box(PathBuf::from(item.content), dark))
        .child(detail_meta_footer(badges, item.created_at, dark))
}

fn text_detail_preview(item: ClipboardRecord, dark: bool) -> impl IntoElement {
    let badges = text_detail_badges(&item);
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(text_preview_box(item.content.clone(), dark))
        .child(detail_meta_footer(badges, item.created_at, dark))
}

fn file_detail_preview(
    handle: Entity<ClipboardPanel>,
    item: ClipboardRecord,
    preview_file_scroll: VirtualListScrollHandle,
    dark: bool,
) -> impl IntoElement {
    let paths = history_store::parse_file_paths(&item.content);
    let badges = file_detail_badges(&item, &paths);
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(file_preview_box(handle, &paths, preview_file_scroll, dark))
        .child(detail_meta_footer(badges, item.created_at, dark))
}

fn detail_meta_footer(badges: Vec<String>, created_at: String, dark: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .items_center()
                .flex_wrap()
                .gap(px(6.0))
                .children(
                    badges
                        .into_iter()
                        .map(|label| detail_mini_badge(label, dark)),
                ),
        )
        .child(
            div()
                .text_size(px(11.0))
                .line_height(px(16.0))
                .text_color(theme::token("color-text-secondary", dark))
                .child(created_at),
        )
}

fn image_detail_badges(item: &ClipboardRecord) -> Vec<String> {
    let mut badges = item
        .preview
        .split('·')
        .map(str::trim)
        .filter(|part| !part.is_empty() && *part != "图片剪贴板")
        .map(String::from)
        .collect::<Vec<_>>();
    if item.pinned {
        badges.insert(0, String::from("已置顶"));
    }
    if badges.is_empty() {
        badges.push(String::from("图片"));
    }
    badges.truncate(4);
    badges
}

fn text_detail_badges(item: &ClipboardRecord) -> Vec<String> {
    let mut badges = history_store::text_badges(&item.content)
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    if item.pinned {
        badges.insert(0, String::from("已置顶"));
    }
    badges.push(history_store::text_stats(&item.content));
    badges.truncate(4);
    badges
}

fn file_detail_badges(item: &ClipboardRecord, paths: &[String]) -> Vec<String> {
    let mut badges = Vec::new();
    if item.pinned {
        badges.push(String::from("已置顶"));
    }
    badges.push(format!("{} 个文件", paths.len()));
    // Show file extensions as badges
    let mut extensions: Vec<String> = paths
        .iter()
        .filter_map(|p| {
            std::path::Path::new(p)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_uppercase())
        })
        .collect();
    extensions.sort();
    extensions.dedup();
    for ext in extensions.iter().take(3) {
        badges.push(ext.clone());
    }
    badges.truncate(4);
    badges
}

fn image_preview_box(path: PathBuf, dark: bool) -> impl IntoElement {
    div()
        .w_full()
        .flex_1()
        .min_h(px(220.0))
        .rounded(px(10.0))
        .bg(theme::token("color-bg-surface", dark))
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .child(
            img(path)
                .object_fit(ObjectFit::Contain)
                .size_full()
                .with_fallback(move || {
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.0))
                        .text_color(theme::token("color-text-placeholder", dark))
                        .child("图片文件不可用")
                        .into_any_element()
                })
                .into_any_element(),
        )
}

fn text_preview_box(content: String, dark: bool) -> impl IntoElement {
    div()
        .id("clipboard-text-preview-scroll")
        .w_full()
        .flex_1()
        .min_h(px(0.0))
        .p(px(14.0))
        .rounded(px(10.0))
        .bg(theme::token("color-bg-surface", dark))
        .overflow_y_scroll()
        .font_family("SF Mono")
        .text_size(px(11.0))
        .line_height(px(17.0))
        .text_color(theme::token("color-text-regular", dark))
        .child(content)
}

fn file_preview_box(
    handle: Entity<ClipboardPanel>,
    paths: &[String],
    scroll: VirtualListScrollHandle,
    dark: bool,
) -> impl IntoElement {
    let states = history_store::file_path_states(paths);

    div()
        .id("clipboard-file-preview-scroll")
        .w_full()
        .flex_1()
        .min_h(px(0.0))
        .p(px(10.0))
        .rounded(px(10.0))
        .bg(theme::token("color-bg-surface", dark))
        .overflow_hidden()
        .when(states.is_empty(), |el| {
            el.child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.0))
                    .text_color(theme::token("color-text-placeholder", dark))
                    .child("文件记录不包含有效路径"),
            )
        })
        .when(!states.is_empty(), |el| {
            let item_sizes = Rc::new(vec![GpuiSize::new(px(0.0), px(56.0)); states.len()]);
            el.child(
                div()
                    .relative()
                    .size_full()
                    .child(
                        v_virtual_list(
                            handle.clone(),
                            "clipboard-file-preview-list",
                            item_sizes,
                            move |_, range, _, _| {
                                range
                                    .map(|index| {
                                        let state = states[index].clone();
                                        file_preview_row(handle.clone(), state, index, dark)
                                    })
                                    .collect::<Vec<_>>()
                            },
                        )
                        .track_scroll(&scroll)
                        .with_sizing_behavior(ListSizingBehavior::Infer)
                        .size_full(),
                    )
                    .child(Scrollbar::vertical(&scroll)),
            )
        })
}

fn file_preview_row(
    handle: Entity<ClipboardPanel>,
    state: history_store::FilePathState,
    index: usize,
    dark: bool,
) -> impl IntoElement {
    let exists = state.can_reveal();
    let kind_label = if state.is_dir { "目录" } else { "文件" };
    let status_label = if exists { kind_label } else { "已不存在" };
    let name_color = if exists {
        theme::token("color-text-primary", dark)
    } else {
        theme::token("color-text-placeholder", dark)
    };
    let path_color = theme::token("color-text-placeholder", dark);
    let badge_color = if exists {
        theme::launcher_accent(dark)
    } else {
        theme::token("color-danger", dark)
    };
    let badge_bg = theme::rgba_with_alpha(badge_color, if exists { 0.10 } else { 0.14 });
    let display_name = state.display_name.clone();
    let raw_path = state.path.clone();
    let click_path = state.path.clone();

    let row = div()
        .id(("clipboard-file-row", index))
        .w_full()
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .min_w(px(0.0))
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(name_color)
                        .line_clamp(1)
                        .child(display_name),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .line_height(px(15.0))
                        .text_color(path_color)
                        .line_clamp(1)
                        .child(raw_path),
                ),
        )
        .child(
            div()
                .px(px(6.0))
                .h(px(18.0))
                .rounded(px(4.0))
                .bg(badge_bg)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(badge_color)
                .child(status_label),
        );

    if exists {
        row.hover(|style| {
            style
                .bg(theme::token("color-row-hover", dark))
                .cursor_pointer()
        })
        .on_click(move |_, _, cx| {
            let path = click_path.clone();
            let _ = cx.update_entity(&handle, |panel, cx| {
                panel.reveal_path_in_finder(&path, cx);
                cx.notify();
            });
        })
    } else {
        row
    }
}

fn detail_empty_state(dark: bool) -> impl IntoElement {
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
                .bg(theme::rgba_with_alpha(theme::launcher_accent(dark), 0.10))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(16.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::launcher_accent(dark))
                .child("预"),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::token("color-text-primary", dark))
                .child("选择一条记录"),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::token("color-text-secondary", dark))
                .child("查看内容预览和操作"),
        )
}

fn detail_mini_badge(label: String, dark: bool) -> impl IntoElement {
    div()
        .h(px(18.0))
        .px_1()
        .rounded(px(4.0))
        .bg(theme::rgba_with_alpha(theme::launcher_accent(dark), 0.08))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .text_color(theme::token("color-text-secondary", dark))
        .child(label)
        .into_any_element()
}

fn detail_panel(
    handle: Entity<ClipboardPanel>,
    selected: Option<ClipboardRecord>,
    _status_text: String,
    preview_file_scroll: VirtualListScrollHandle,
    dark: bool,
) -> impl IntoElement {
    let is_pinned = selected.as_ref().map_or(false, |r| r.pinned);
    let has_selected = selected.is_some();
    let is_files = selected
        .as_ref()
        .map_or(false, |r| r.kind == history_store::ClipboardItemKind::Files);
    div()
        .w(px(420.0))
        .h_full()
        .p(px(14.0))
        .bg(theme::token("color-bg-subtle", dark))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .rounded(px(12.0))
                .border_1()
                .border_color(theme::token("color-border-default", dark))
                .bg(theme::token("color-bg-page", dark))
                .p(px(12.0))
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(
                    selected
                        .clone()
                        .map(|item| {
                            detail_preview_card(
                                handle.clone(),
                                item,
                                preview_file_scroll.clone(),
                                dark,
                            )
                            .into_any_element()
                        })
                        .unwrap_or_else(|| detail_empty_state(dark).into_any_element()),
                ),
        )
        .when(is_files && has_selected, |el| {
            let record = selected.clone().unwrap();
            let paths = history_store::parse_file_paths(&record.content);
            let states = history_store::file_path_states(&paths);
            let total = states.len();
            let existing = states.iter().filter(|state| state.can_reveal()).count();
            let has_existing = existing > 0;
            let has_open_target = states
                .iter()
                .any(history_store::FilePathState::has_actionable_target);
            el.child(file_path_status(existing, total, dark))
                .child(file_detail_action_row(
                    handle.clone(),
                    has_existing,
                    has_open_target,
                    dark,
                ))
        })
        .child(detail_actions(handle, has_selected, is_pinned, dark))
}

fn detail_actions(
    handle: Entity<ClipboardPanel>,
    has_selected: bool,
    is_pinned: bool,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .when(has_selected, |el| {
            el.child(detail_action_button("复制", dark, {
                let handle = handle.clone();
                move |_, window, cx| {
                    let _ = cx.update_entity(&handle, |panel, cx| {
                        panel.copy_selected(cx);
                        cx.notify();
                    });
                    window.remove_window();
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
        })
}

fn detail_action_button(
    label: &'static str,
    dark: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(label)
        .h(px(26.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .bg(theme::token("color-bg-surface", dark))
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .hover(|style| {
            style
                .bg(theme::token("color-row-hover", dark))
                .cursor_pointer()
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(theme::token("color-text-secondary", dark))
        .child(label)
        .on_click(on_click)
}

fn file_detail_action_row(
    handle: Entity<ClipboardPanel>,
    has_existing: bool,
    has_open_target: bool,
    dark: bool,
) -> impl IntoElement {
    let reveal_handle = handle.clone();
    let open_handle = handle.clone();
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(file_action_button(
            "在访达中显示",
            "clipboard-file-reveal",
            has_existing,
            dark,
            move |_, _, cx| {
                let _ = cx.update_entity(&reveal_handle, |panel, cx| {
                    panel.reveal_first_existing_in_finder(cx);
                    cx.notify();
                });
            },
        ))
        .child(file_action_button(
            "打开目录",
            "clipboard-file-open-dir",
            has_open_target,
            dark,
            move |_, _, cx| {
                let _ = cx.update_entity(&open_handle, |panel, cx| {
                    panel.open_selected_parent_dir(cx);
                    cx.notify();
                });
            },
        ))
}

fn file_action_button(
    label: &'static str,
    button_id: &'static str,
    enabled: bool,
    dark: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let color = if enabled {
        theme::launcher_accent(dark)
    } else {
        theme::token("color-text-placeholder", dark)
    };
    div()
        .id(button_id)
        .h(px(26.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .bg(if enabled {
            theme::rgba_with_alpha(theme::launcher_accent(dark), 0.10)
        } else {
            theme::rgba_with_alpha(theme::token("color-bg-surface", dark), 0.50)
        })
        .border_1()
        .border_color(if enabled {
            theme::launcher_accent(dark)
        } else {
            theme::token("color-border-default", dark)
        })
        .hover(move |style| {
            if enabled {
                style
                    .bg(theme::rgba_with_alpha(theme::launcher_accent(dark), 0.18))
                    .cursor_pointer()
            } else {
                style
            }
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(color)
        .child(label)
        .on_click(move |event, window, cx| {
            if enabled {
                on_click(event, window, cx);
            }
        })
}

fn file_path_status(existing: usize, total: usize, dark: bool) -> impl IntoElement {
    let text = if total == 0 {
        String::from("无文件路径")
    } else if existing == 0 {
        String::from("所有文件路径已不存在")
    } else if existing == total {
        format!("{total} 个路径都存在")
    } else {
        format!("{existing}/{total} 个路径存在")
    };
    let color = if total == 0 || existing == 0 {
        theme::token("color-text-placeholder", dark)
    } else {
        theme::token("color-text-secondary", dark)
    };
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(div().text_size(px(11.0)).text_color(color).child(text))
}
