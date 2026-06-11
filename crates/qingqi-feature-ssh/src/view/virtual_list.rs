//! uniform_list 统一封装

use std::ops::Range;

use gpui::*;

/// 占满父级剩余高度的纵向虚拟列表（父级需有明确高度，如 `flex_1` + `min_h(0)`）
pub fn vertical(
    id: impl Into<ElementId>,
    count: usize,
    scroll: UniformListScrollHandle,
    build: impl Fn(Range<usize>, &mut Window, &mut App) -> Vec<AnyElement> + 'static,
) -> impl IntoElement {
    uniform_list(id, count, build)
        .track_scroll(scroll)
        .size_full()
}

/// 固定高度的纵向虚拟列表
pub fn vertical_fixed(
    id: impl Into<ElementId>,
    count: usize,
    height: Pixels,
    scroll: UniformListScrollHandle,
    build: impl Fn(Range<usize>, &mut Window, &mut App) -> Vec<AnyElement> + 'static,
) -> impl IntoElement {
    div().h(height).w_full().child(
        uniform_list(id, count, build)
            .track_scroll(scroll)
            .size_full(),
    )
}
