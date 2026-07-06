//! Profile 右键菜单（qingqi-ui PopupMenu）

use gpui::Entity;
use qingqi_ui::layer::context_menu::{PopupMenu, PopupMenuItem};

#[derive(Clone, Copy)]
enum ProfileAction {
    Connect,
    Edit,
    Delete,
}

pub fn profile_menu(menu: PopupMenu, profile_id: i64, handle: Entity<super::SshView>) -> PopupMenu {
    menu.item(action_item(
        "连接",
        handle.clone(),
        profile_id,
        ProfileAction::Connect,
    ))
    .item(action_item(
        "编辑",
        handle.clone(),
        profile_id,
        ProfileAction::Edit,
    ))
    .item(action_item(
        "删除",
        handle,
        profile_id,
        ProfileAction::Delete,
    ))
}

fn action_item(
    label: &'static str,
    handle: Entity<super::SshView>,
    profile_id: i64,
    action: ProfileAction,
) -> PopupMenuItem {
    PopupMenuItem::new(label).on_click(move |_, _, cx| {
        handle.update(cx, |view, cx| match action {
            ProfileAction::Connect => view.connect_profile(profile_id, cx),
            ProfileAction::Edit => view.open_profile_editor(Some(profile_id), cx),
            ProfileAction::Delete => view.delete_profile(profile_id, cx),
        });
    })
}
