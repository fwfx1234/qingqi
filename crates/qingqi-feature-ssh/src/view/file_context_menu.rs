//! 文件列表右键菜单（gpui-component PopupMenu）

use gpui::Entity;
use gpui_component::menu::{PopupMenu, PopupMenuItem};

use super::{FileEntryRow, SshView};

#[derive(Clone, Copy)]
enum FileMenuAction {
    Open,
    Edit,
    Download,
    Rename,
    Delete,
    Refresh,
    UploadFile,
    UploadFolder,
    Mkdir,
}

/// 根据右键目标构建菜单（列表区域唯一入口）
pub fn build(
    menu: PopupMenu,
    target: Option<FileEntryRow>,
    handle: Entity<SshView>,
) -> PopupMenu {
    match target {
        Some(entry) => entry_menu(menu, entry, handle),
        None => blank_menu(menu, handle),
    }
}

/// 文件/目录项右键菜单
pub fn entry_menu(menu: PopupMenu, entry: FileEntryRow, handle: Entity<SshView>) -> PopupMenu {
    let mut menu = menu;
    if entry.is_dir || entry.is_parent {
        menu = menu.item(action_item(
            "打开",
            handle.clone(),
            Some(entry.clone()),
            FileMenuAction::Open,
        ));
        if entry.is_dir && !entry.is_parent {
            menu = menu.item(action_item(
                "下载",
                handle.clone(),
                Some(entry.clone()),
                FileMenuAction::Download,
            ));
        }
    } else {
        menu = menu
            .item(action_item(
                "编辑",
                handle.clone(),
                Some(entry.clone()),
                FileMenuAction::Edit,
            ))
            .item(action_item(
                "下载",
                handle.clone(),
                Some(entry.clone()),
                FileMenuAction::Download,
            ));
    }
    menu.separator()
        .item(action_item(
            "重命名",
            handle.clone(),
            Some(entry.clone()),
            FileMenuAction::Rename,
        ))
        .when(!entry.is_parent, |menu| {
            menu.item(action_item(
                "删除",
                handle.clone(),
                Some(entry.clone()),
                FileMenuAction::Delete,
            ))
        })
        .separator()
        .item(action_item(
            "刷新",
            handle,
            Some(entry),
            FileMenuAction::Refresh,
        ))
}

/// 空白区域右键菜单
pub fn blank_menu(menu: PopupMenu, handle: Entity<SshView>) -> PopupMenu {
    menu.item(action_item(
        "上传文件",
        handle.clone(),
        None,
        FileMenuAction::UploadFile,
    ))
    .item(action_item(
        "上传文件夹",
        handle.clone(),
        None,
        FileMenuAction::UploadFolder,
    ))
    .item(action_item(
        "新建文件夹",
        handle.clone(),
        None,
        FileMenuAction::Mkdir,
    ))
    .separator()
    .item(action_item("刷新", handle, None, FileMenuAction::Refresh))
}

fn action_item(
    label: &'static str,
    handle: Entity<SshView>,
    entry: Option<FileEntryRow>,
    action: FileMenuAction,
) -> PopupMenuItem {
    PopupMenuItem::new(label).on_click(move |_, _, cx| {
        handle.update(cx, |view, cx| {
            dispatch(view, cx, action, entry.as_ref());
        });
    })
}

fn dispatch(
    view: &mut SshView,
    cx: &mut gpui::Context<SshView>,
    action: FileMenuAction,
    entry: Option<&FileEntryRow>,
) {
    match action {
        FileMenuAction::Open => {
            if let Some(entry) = entry {
                view.open_file_entry(entry, cx);
            }
        }
        FileMenuAction::Edit => {
            if let Some(entry) = entry {
                view.open_file_editor(entry, cx);
            }
        }
        FileMenuAction::Download => {
            if let Some(entry) = entry {
                view.download_file_entry(entry, cx);
            }
        }
        FileMenuAction::Rename => {
            if let Some(entry) = entry {
                view.open_file_rename(entry, cx);
            }
        }
        FileMenuAction::Delete => {
            if let Some(entry) = entry {
                view.delete_file_entry(entry, cx);
            }
        }
        FileMenuAction::Refresh => view.refresh_file_tree(cx),
        FileMenuAction::UploadFile => view.pick_and_upload_files(cx),
        FileMenuAction::UploadFolder => view.pick_and_upload_folder(cx),
        FileMenuAction::Mkdir => view.create_directory_in_cwd(cx),
    }
}

trait PopupMenuExt {
    fn when(self, condition: bool, f: impl FnOnce(Self) -> Self) -> Self
    where
        Self: Sized;
}

impl PopupMenuExt for PopupMenu {
    fn when(self, condition: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if condition {
            f(self)
        } else {
            self
        }
    }
}
