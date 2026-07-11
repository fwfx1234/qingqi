//! Tree component — local replacement for qingqi-ui::tree.

use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    App, Context, FocusHandle, IntoElement, ParentElement, Render, SharedString, Window, div,
    prelude::*, uniform_list,
};

use super::list::ListItem;
use super::styled::StyledExt;

#[derive(Clone)]
pub struct TreeItem {
    pub id: SharedString,
    pub label: SharedString,
    pub children: Vec<TreeItem>,
    state: Rc<RefCell<TreeItemState>>,
}

struct TreeItemState {
    expanded: bool,
    disabled: bool,
}

impl TreeItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            state: Rc::new(RefCell::new(TreeItemState {
                expanded: false,
                disabled: false,
            })),
        }
    }
    pub fn child(mut self, child: TreeItem) -> Self {
        self.children.push(child);
        self
    }
    pub fn children(mut self, children: impl IntoIterator<Item = TreeItem>) -> Self {
        self.children.extend(children);
        self
    }
    pub fn expanded(self, expanded: bool) -> Self {
        self.state.borrow_mut().expanded = expanded;
        self
    }
    pub fn disabled(self, disabled: bool) -> Self {
        self.state.borrow_mut().disabled = disabled;
        self
    }
    pub fn is_folder(&self) -> bool {
        self.children.len() > 0
    }
    pub fn is_disabled(&self) -> bool {
        self.state.borrow().disabled
    }
    pub fn is_expanded(&self) -> bool {
        self.state.borrow().expanded
    }
}

#[derive(Clone)]
pub struct TreeEntry {
    item: TreeItem,
    depth: usize,
}

impl TreeEntry {
    pub fn item(&self) -> &TreeItem {
        &self.item
    }
    pub fn depth(&self) -> usize {
        self.depth
    }
    pub fn is_folder(&self) -> bool {
        self.item.is_folder()
    }
    pub fn is_expanded(&self) -> bool {
        self.item.is_expanded()
    }
    pub fn is_disabled(&self) -> bool {
        self.item.is_disabled()
    }
}

fn flatten(items: &[TreeItem], depth: usize, out: &mut Vec<TreeEntry>) {
    for item in items {
        out.push(TreeEntry {
            item: item.clone(),
            depth,
        });
        if item.is_expanded() {
            flatten(&item.children, depth + 1, out);
        }
    }
}

pub struct TreeState {
    _focus_handle: FocusHandle,
    entries: Vec<TreeEntry>,
    selected_ix: Option<usize>,
    render_item_cell:
        Rc<RefCell<Option<Rc<dyn Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> ListItem>>>>,
}

impl TreeState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            selected_ix: None,
            _focus_handle: cx.focus_handle(),
            entries: Vec::new(),
            render_item_cell: Rc::new(RefCell::new(None)),
        }
    }

    pub fn items(mut self, items: impl Into<Vec<TreeItem>>) -> Self {
        let items = items.into();
        let mut entries = Vec::new();
        flatten(&items, 0, &mut entries);
        self.entries = entries;
        self
    }

    pub fn set_selected_index(&mut self, ix: Option<usize>, cx: &mut Context<Self>) {
        self.selected_ix = ix;
        cx.notify();
    }

    fn on_entry_click(&mut self, ix: usize, _window: &mut Window, cx: &mut Context<Self>) {
        self.selected_ix = Some(ix);
        cx.notify();
    }

    pub(crate) fn attach_renderer<F>(&self, f: F)
    where
        F: Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> ListItem + 'static,
    {
        *self.render_item_cell.borrow_mut() = Some(Rc::new(f));
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_ix
    }

    pub fn set_items(&mut self, items: impl Into<Vec<TreeItem>>, cx: &mut Context<Self>) {
        let items = items.into();
        let mut entries = Vec::new();
        flatten(&items, 0, &mut entries);
        self.entries = entries;
        cx.notify();
    }
}

impl Render for TreeState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let render_item_cell = self.render_item_cell.clone();
        let entity = cx.entity().clone();

        div().id("tree-state").size_full().relative().child(
            uniform_list("entries", self.entries.len(), {
                move |visible_range: Range<usize>, window, cx| {
                    // First, collect what we need from the entity without holding the borrow
                    let (entries_snapshot, selected) = {
                        let st = entity.read(cx);
                        // We only clone the entries in the visible range
                        let visible_entries: Vec<TreeEntry> = visible_range
                            .clone()
                            .map(|ix| st.entries[ix].clone())
                            .collect();
                        (visible_entries, st.selected_ix)
                    };
                    // Now we can call the user's render function with &mut cx
                    let render_item = render_item_cell.borrow();
                    let mut items: Vec<ListItem> = Vec::with_capacity(visible_range.len());
                    for (offset, entry) in entries_snapshot.into_iter().enumerate() {
                        let ix = visible_range.start + offset;
                        let is_selected = Some(ix) == selected;
                        let item = if let Some(ref f) = *render_item {
                            (f)(ix, &entry, is_selected, window, cx)
                        } else {
                            ListItem::new(ix)
                        };
                        items.push(item);
                    }
                    items
                }
            })
            .flex_grow()
            .size_full(),
        )
    }
}

pub fn tree<R>(state: &gpui::Entity<TreeState>, render_item: R) -> Tree
where
    R: Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> ListItem + 'static,
{
    Tree {
        entity: state.clone(),
        _render_item: Rc::new(render_item),
    }
}

pub struct Tree {
    entity: gpui::Entity<TreeState>,
    _render_item: Rc<dyn Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> ListItem>,
}

impl IntoElement for Tree {
    type Element = gpui::AnyElement;
    fn into_element(self) -> Self::Element {
        div()
            .id("tree")
            .child(self.entity.clone())
            .into_any_element()
    }
}

impl Styled for Tree {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        panic!("Cannot style Tree at top level")
    }
}
