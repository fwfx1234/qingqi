//! 文件列表右键菜单

use gpui::prelude::*;
use gpui::*;
use qingqi_ui::{theme, theme_mode, ui};
use qingqi_ui::ui::glass;

use super::FileEntryRow;

pub fn render_file_context_menu(
    handle: Entity<super::SshView>,
    entry: Option<FileEntryRow>,
    position: Point<Pixels>,
) -> impl IntoElement {
    let dark = theme_mode::is_dark();
    let backdrop = handle.clone();
    let menu_x = position.x.max(px(8.0));
    let menu_y = position.y.max(px(8.0));

    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .child(
            div()
                .id("file-menu-backdrop")
                .size_full()
                .absolute()
                .bg(hsla(0.0, 0.0, 0.0, 0.001))
                .on_click({
                    let h = backdrop.clone();
                    move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                        h.update(cx, |v, cx| v.close_file_context_menu(cx));
                    }
                }),
        )
        .child(
            div()
                .absolute()
                .top(menu_y)
                .left(menu_x)
                .w(px(176.0))
                .rounded(theme::radius_md())
                .border_1()
                .border_color(glass::border(dark))
                .bg(theme::semantic().bg_elevated)
                .shadow_lg()
                .overflow_hidden()
                .flex()
                .flex_col()
                .py(px(4.0))
                .children(menu_items(handle, entry, dark)),
        )
}

fn menu_items(
    handle: Entity<super::SshView>,
    entry: Option<FileEntryRow>,
    dark: bool,
) -> Vec<AnyElement> {
    let mut items: Vec<AnyElement> = Vec::new();

    if let Some(ref e) = entry {
        if e.is_dir || e.is_parent {
            items.push(
                menu_item(handle.clone(), 0, "open", "打开", false, dark, Some(e.clone()))
                    .into_any_element(),
            );
        }
    }

    items.push(
        menu_item(handle.clone(), 1, "refresh", "刷新", false, dark, entry.clone())
            .into_any_element(),
    );

    if entry.is_none() {
        items.push(
            menu_item(handle.clone(), 2, "mkdir", "新建文件夹", false, dark, None)
                .into_any_element(),
        );
    }

    if let Some(ref e) = entry {
        if !e.is_dir && !e.is_parent {
            items.push(
                menu_item(handle, 3, "download", "下载", false, dark, Some(e.clone()))
                    .into_any_element(),
            );
        }
    }

    items
}

fn menu_item(
    handle: Entity<super::SshView>,
    idx: u32,
    action: &'static str,
    label: &'static str,
    danger: bool,
    dark: bool,
    entry: Option<FileEntryRow>,
) -> impl IntoElement {
    let h = handle.clone();
    div()
        .id(("file-menu", idx))
        .h(px(28.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .text_size(theme::font_size_body())
        .text_color(if danger {
            ui::danger()
        } else {
            ui::text_primary()
        })
        .cursor_pointer()
        .hover(|s| s.bg(glass::hover_bg(dark)))
        .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            h.update(cx, |v, cx| {
                match action {
                    "open" => {
                        if let Some(ref e) = entry {
                            v.open_file_entry(e, cx);
                        }
                    }
                    "refresh" => v.refresh_file_tree(cx),
                    "mkdir" => v.create_directory_in_cwd(cx),
                    "download" => {
                        if let Some(ref e) = entry {
                            v.download_file_entry(e, cx);
                        }
                    }
                    _ => {}
                }
                v.close_file_context_menu(cx);
            });
        })
        .child(label)
}
