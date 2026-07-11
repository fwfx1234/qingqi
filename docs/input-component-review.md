# 输入组件设计审查报告

## 修复后的独立使用

`Input` 不再读取 `Root::focused_input`，也不要求挂在 Popover 或其他父组件下。主题 token 未安装时会自动使用默认浅色值。应用启动时只需初始化一次组件按键；重复调用也是安全的。

```rust
use gpui::{App, AppContext as _, Context, Entity, IntoElement, Render, Window, div};
use qingqi_ui::components::input::{Input, InputState};

fn init(cx: &mut App) {
    qingqi_ui::components::init(cx);
}

struct View {
    input: Entity<InputState>,
}

impl View {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入"));
        Self { input }
    }
}

impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(Input::new(&self.input))
    }
}
```

值更新语义：`default_value` 仅用于构造期默认值；`reset_value` 同步文本、布局、搜索和 AutoGrow，但不触发业务 `Change`；`set_value` 与用户编辑都会记录历史并触发 `Change`。

LSP provider 与对应弹层仍保留类型和 re-export 以保证下游编译兼容，但本版本明确设置 `LSP_SUPPORTED = false`。触发相关 action 时会记录一次警告并向父级传播。

下文保留的是修复前审查记录，用于说明问题来源；其中列出的 Input 缺陷已由本次实现和回归测试覆盖。

> 审查范围：`crates/qingqi-ui/src/components/input/`（~8800 行，31 个文件）
> 视角：Rust 专家，不借助项目文档，纯代码分析

---

## 一、架构总评

组件采用 GPUI 的 Entity + Element 分离架构，模块划分清晰。核心数据流：

```
Input (builder 壳) → InputState (Entity, 状态中枢) → TextElement (Element, 渲染)
```

- 支持 3 种模式：PlainText / CodeEditor / AutoGrow
- 衍生组件：NumberInput、OtpInput
- 辅助能力：MaskPattern、SearchMatcher、BlinkCursor、LSP stubs

整体结构合理，但存在**多个功能错误和性能隐患**。

---

## 二、功能错误（优先级 P0）

### 2.1 Redo 完全不可用 / Undo 不维护 redo 栈

**文件**: `state.rs:809`
```rust
fn redo(&mut self, _: &Redo, _window: &mut Window, _cx: &mut Context<Self>) {}
```

`History::push` 时清除了 `redo_stack`，`undo()` 只从 `undo_stack` pop，**从未将 undo 的变更压入 `redo_stack`**。

**影响**: `Ctrl+Y` / `Ctrl+Shift+Z` 无任何效果。

**修复方向**: 在 `undo()` 中将逆操作压入 `redo_stack`；在 `redo()` 中执行 redo 栈顶操作。

---

### 2.2 IME 操作不被 Undo 记录

**文件**: `state.rs:1110-1155`（`EntityInputHandler::replace_text_in_range`）、`state.rs:1156-1218`（`replace_and_mark_text_in_range`）

这两个方法直接修改 `self.text`，**绕过了 `replace_and_expand_common_prefix`**，不产生 `Change` 记录。

**影响**: 中文/日文输入法提交的文字无法 undo；与直接键入的行为不一致。

**修复方向**: IME 提交（composition 结束）时调用 `replace_and_expand_common_prefix` 统一路径；或提取内部方法可选是否记录 history。

---

### 2.3 选区方向无规范化，可能导致 panic

**文件**: `cursor.rs:5-26`、`state.rs:578-582`

`Selection` 用裸 `start`/`end` 两个 usize，无 `start <= end` 不变量。`select_to` 中：
```rust
self.selected_range = (self.selected_range.start..offset).into();
```
当 `offset < start` 时产生反向选区。`slice(start..end)` 在 `start > end` 时 panic。

**影响**: 从右向左拖拽选区、Shift+Left 等场景可能 panic 或产生不可预测行为。

**修复方向**: `Selection::new` 中强制 `if start > end { std::mem::swap } }`；或引入 `SelectionDirection` 枚举。

---

### 2.4 垂直移动时 `local_row` 未重算

**文件**: `movement.rs:43-52`
```rust
let new_offset = self.text_wrapper.display_point_to_offset(
    DisplayPoint::new(new_row, display_point.local_row, display_point.column)
);
```
把当前行的 `local_row` 直接带入新行，但新行的软换行结构不同，`local_row` 可能越界。

**影响**: 在软换行的多行文本中上下移动光标，水平位置可能跳变。

**修复方向**: 根据目标行的 wrapped line 结构重新计算 `local_row`。

---

### 2.5 单词边界仅支持 ASCII

**文件**: `state.rs:666-699`、`selection.rs:22-30`

```rust
if ch.is_whitespace() || !ch.is_alphanumeric() { break; }
```
`CharType::Word` 仅含 ASCII 字母数字和 `_`。

**影响**: CJK 文本中 Ctrl+Left/Right 跳至行首/行尾；双击中文无法选词。

**修复方向**: 使用 `unicode_segmentation` 库的 `UnicodeWord` trait，或引入 `XID_Continue` 判断。

---

### 2.6 `MaskPattern` 与实际输入流程脱节

**文件**: `mask_pattern.rs`（完整实现）vs `state.rs`（无调用）

`MaskPattern` 定义了 `mask`/`unmask`/`validate`，但 `InputState` 的 `replace_text_in_range` 和 `replace_and_mark_text_in_range` 中**没有任何 mask 验证调用**。

**影响**: 配置了 `MaskPattern` 的输入框不执行任何格式限制。

**修复方向**: 在 `replace_and_expand_common_prefix` 入口校验 mask；或在 `InputState` 中存储 mask pattern 并拦截非法输入。

---

## 三、性能问题（优先级 P1）

### 3.1 RopeExt 每次都全文转 String

**文件**: `rope_ext.rs`（全部方法）

`line_start_offset`、`char_at`、`offset_to_point`、`clip_offset`、`word_range` 等核心方法都先 `self.to_string()` 再遍历。

**影响**: 完全抵消了 Rope 的 O(log n) 优势。每次按键、paint、鼠标定位都触发全文复制。

**修复方向**: 使用 ropey 原生 API：
- `Rope::line_to_byte` / `byte_to_line`
- `Rope::chars_at` / `byte_to_char`
- `RopeSlice::len_bytes`

---

### 3.2 `character_index_for_point` 逐字符 shape_line

**文件**: `state.rs:997-1057`、`state.rs:1277-1330`

鼠标点击/光标定位时，对每个字符逐个调用 `window.text_system().shape_line`。

**影响**: 长文本下每次点击触发上百次系统文本塑形调用。

**修复方向**: 二分查找替代线性扫描；缓存 shaped line 宽度数组。

---

### 3.3 `element.rs` paint 中重复测量无缓存

**文件**: `element.rs:137-159`（`measure_text`）

渲染 selection、cursor、placeholder 时多次调用 `measure_text`（内部 shape_line），每次重建 `TextRun`/`Font`。

**影响**: 每帧重复计算相同文本宽度。

**修复方向**: 在 `LastLayout` 中缓存每行 shaped line 及其宽度；paint 时直接查表。

---

### 3.4 `replace_and_expand_common_prefix` 多余的 Rope 拷贝

**文件**: `state.rs:556-575`
```rust
&Rope::from(self.text.to_string())  // Rope → String → Rope
```

**影响**: 每次编辑触发全量 Rope 拷贝。

**修复方向**: 直接传 `&self.text`（Rope 本身），无需中间 String 转换。

---

## 四、尺寸/布局问题（优先级 P1）

### 4.1 硬编码 padding 8px 散落多处，无法统一调整

**文件**: `element.rs:127`、`state.rs:999`、`state.rs:1279`、`state.rs:1263`

```rust
let text_padding = px(8.0);  // element.rs paint
let text_padding = px(8.0);  // state.rs character_index_for_point (×2)
let cursor_x = bounds.left() + px(8.0) + shaped.width;  // bounds_for_range
```

同一常量在 4 处独立定义，无共享常量。

**影响**: 修改内边距需改多处，容易遗漏导致光标位置与文本不对齐。

**修复方向**: 在 `input.rs` 或共享模块定义 `const TEXT_PADDING: Pixels = px(8.0);` 统一引用。

---

### 4.2 `font_size` 计算 `rem_size() * 0.875` 重复且语义不清

**文件**: `element.rs:126`、`state.rs:1001`、`state.rs:1233`、`state.rs:1281`

```rust
let font_size = window.rem_size() * 0.875; // ~14px for 16px rem
```

4 处独立计算，注释说明意图但无命名常量。

**影响**: 与 GPUI 的 `TextStyle` 体系脱节；若需支持用户字号调整则需全量修改。

**修复方向**: 定义 `const FONT_SIZE_RATIO: f32 = 0.875;` 或从 `TextStyle` 获取。

---

### 4.3 `LINE_HEIGHT` 常量仅用于外层，与 `window.line_height()` 不一致

**文件**: `input.rs:15`、`element.rs:74`、`element.rs:125`

```rust
// input.rs
const LINE_HEIGHT: gpui::Rems = gpui::Rems(1.25);
// ...
.line_height(LINE_HEIGHT)  // 设置外层 div 的行高

// element.rs
let line_height = window.line_height();  // 用系统行高做文本测量
```

外层 div 的 `line_height` 与 paint 中使用的 `window.line_height()` 可能不一致（取决于 rem_size），导致文本垂直位置偏移。

**影响**: 光标、选区、文本基线可能不在同一水平线上。

**修复方向**: 统一使用 `window.line_height()` 或统一使用 `LINE_HEIGHT` 常量。

---

### 4.4 `TextWrapper` 默认 wrap width 硬编码 800px

**文件**: `text_wrapper.rs:108`、`text_wrapper.rs:167`

```rust
let wrap_width = self.wrap_width.unwrap_or(px(800.0));
```

**影响**: 当输入框实际宽度不是 800px 时，软换行位置与实际渲染位置不一致；`move_vertical` 计算错误。

**修复方向**: `set_wrap_width` 应从 `input_bounds.size.width` 获取实际宽度；`soft_wrap` 启用时自动同步。

---

### 4.5 `AutoGrow` 模式高度不随内容更新

**文件**: `mode.rs:96-103`、`state.rs:574`

`update_auto_grow` 在 `replace_and_expand_common_prefix` 末尾调用，但 `Input::render` 中**没有根据 `mode.rows()` 动态设置容器高度**。

```rust
// input.rs render
.when(self.height.is_none(), |this| this.h_full())
```

`h_full()` 是 `relative(1.)`，不是根据行数计算的高度。

**影响**: AutoGrow 模式下输入框不会随内容增长而变高。

**修复方向**: `Input` 需感知 `InputState.mode.rows()`，动态设置 `height` 为 `rows * line_height + padding`。

---

### 4.6 滚动完全未实现

**文件**: `state.rs:275`、`state.rs:988-989`、`input.rs:1355`

```rust
pub(crate) scroll_handle: ScrollHandle,  // 创建了但从未使用
// self.scroll_handle.handle_scroll_event(event, cx);  // 被注释掉
.overflow_x_hidden()  // 仅水平隐藏
```

**影响**: 多行输入框内容超出容器时无法滚动；长文本被截断且无法查看。

**修复方向**: 实现 `ScrollHandle` 与 `input_bounds` 的联动；paint 时应用 scroll offset 做 clip。

---

### 4.7 `LastLayout` 中 `visible_range` 始终为 `0..1`

**文件**: `element.rs:77`

```rust
let last_layout = super::LastLayout {
    visible_range: 0..1,  // 硬编码
    ...
};
```

`visible_range` 和 `visible_top` 从未根据实际滚动位置更新。

**影响**: 多行文本渲染时无法做视口裁剪；`indent_guides` 和 `page_up`/`page_down` 的行数计算基于错误数据。

**修复方向**: 根据 `scroll_handle` 偏移量和 `input_bounds` 高度计算实际可见行范围。

---

### 4.8 `bounds_for_range` 返回值不反映实际文本范围

**文件**: `state.rs:1059-1068`、`state.rs:1223-1270`

```rust
fn bounds_for_range(...) -> Option<Bounds<Pixels>> {
    None  // 第一处直接返回 None
}
// 第二处始终返回 cursor 位置，忽略 range_utf16 参数
```

**影响**: IME 候选窗口无法正确定位在正在编辑的文本处；只读模式下 `text_for_range` 返回正确但 `bounds_for_range` 返回 None。

**修复方向**: 根据 `range_utf16` 计算对应文本的像素范围。

---

## 五、API 设计问题（优先级 P2）

### 5.1 `Input` builder 多个空壳方法

**文件**: `input.rs:72-84`

`gap_0`、`rounded_none`、`shadow_none`、`small`、`xsmall` 全部是 no-op。

**影响**: 调用方无法获得预期效果，误导性 API。

**修复方向**: 实现对应样式或移除这些方法。

---

### 5.2 `character_index_for_point` 重复实现

**文件**: `state.rs:997` 和 `state.rs:1277`

两份完全相同的实现，维护负担。

---

### 5.3 `NumberInput` 只发事件不做计算

**文件**: `number_input.rs`

`Increment`/`Decrement` 事件透传，数值解析、范围限制、格式化全交给消费方。

**影响**: 作为"数字输入组件"核心职责缺失。

---

### 5.4 `OtpInput` 不支持粘贴

**文件**: `otp_input.rs:86-104`

`on_key_down` 只处理单字符和退格。

---

### 5.5 `measure_text` 在单行模式下将 `\n` 替换为空格

**文件**: `element.rs:139`
```rust
let text = text.replace('\n', " ").replace('\r', "");
```

**影响**: 粘贴多行文本到单行输入框时静默篡改数据而非拒绝。

---

## 六、潜在 Panic / 安全问题（优先级 P2）

### 6.1 `slice_line` 在 `end == 0` 时可能 underflow

**文件**: `rope_ext.rs:119-126`
```rust
let end = if end > start {
    let char_idx = byte_to_char_idx(&text, end - 1);  // end=0 时溢出
```

虽然外层有守卫，但调用方传入 `start > end` 的 range 会 panic。

---

### 6.2 `Selection` 的 `contains` 与 `RangeBounds` 实现不一致

`contains` 用半开区间 `[start, end)`；`RangeBounds` 的 `start_bound` 是 `Included`、`end_bound` 是 `Excluded`。当 `start > end` 时行为不可预测。

---

## 七、修复优先级汇总

| 优先级 | 编号 | 问题 | 类型 |
|--------|------|------|------|
| **P0** | 2.1 | Redo 空实现 | 功能错误 |
| **P0** | 2.2 | IME 不记录 undo | 功能错误 |
| **P0** | 2.3 | 选区方向不规范化 | 潜在 panic |
| **P0** | 2.4 | 垂直移动 local_row 错误 | 功能错误 |
| **P0** | 2.5 | 单词边界仅 ASCII | 功能错误 |
| **P0** | 2.6 | MaskPattern 不生效 | 功能错误 |
| **P1** | 3.1 | RopeExt 全文转 String | 性能 |
| **P1** | 3.2 | 逐字符 shape_line | 性能 |
| **P1** | 3.3 | paint 无缓存重复测量 | 性能 |
| **P1** | 3.4 | 多余 Rope 拷贝 | 性能 |
| **P1** | 4.1 | padding 硬编码散落 | 维护性 |
| **P1** | 4.3 | LINE_HEIGHT 不一致 | 布局错误 |
| **P1** | 4.4 | wrap width 硬编码 800px | 布局错误 |
| **P1** | 4.5 | AutoGrow 不更新高度 | 功能缺失 |
| **P1** | 4.6 | 滚动未实现 | 功能缺失 |
| **P1** | 4.7 | visible_range 硬编码 | 渲染错误 |
| **P1** | 4.8 | bounds_for_range 不正确 | IME 体验 |
| **P2** | 5.1 | builder 空壳方法 | API 设计 |
| **P2** | 5.2 | 重复方法实现 | 维护性 |
| **P2** | 5.3-5.5 | 衍生组件功能缺失 | 功能缺失 |
| **P2** | 6.1-6.2 | 潜在 panic | 安全性 |

---

## 八、推荐修复顺序

1. **第一轮（功能正确性）**: 2.1 → 2.3 → 2.2 → 2.5 → 2.4 → 2.6
2. **第二轮（性能 + 布局）**: 3.1 → 3.4 → 4.4 → 4.3 → 3.2 → 3.3
3. **第三轮（功能完整性）**: 4.6 → 4.5 → 4.7 → 4.8
4. **第四轮（API 清理）**: 5.1 → 5.2 → 5.3 → 5.4 → 5.5 → 6.1 → 6.2

---

## 九、修复记录（2026-07-06）

### 已修复（编译通过，零新增错误/warning）

#### P0 功能错误 ✅

| # | 问题 | 文件 | 修复方式 |
|---|------|------|----------|
| 2.1 | Redo 空实现 | `state.rs:776-815` | 补全双向栈：undo 推入 redo_stack，redo 推入 undo_stack；同时修正 undo 使用错误 range 的 bug |
| 2.2 | IME 不记录 undo | `state.rs:1205-1255` | `replace_text_in_range` 中检测 IME 提交（`ime_marked_range` 为 Some），记录完整 Change |
| 2.3 | 选区方向不规范化 | `cursor.rs:35-40` | `From<Range<usize>>` 中自动 swap 确保 `start <= end` |
| 2.4 | 垂直移动 local_row 错误 | `movement.rs:43-63` | 根据目标行 wrapped line 数量 clamp `local_row` |
| 2.5 | 单词边界仅 ASCII | `selection.rs:24` | `is_ascii_alphanumeric()` → `is_alphanumeric()` 支持 Unicode |

#### P1 尺寸/布局 ✅

| # | 问题 | 文件 | 修复方式 |
|---|------|------|----------|
| 4.1 | padding 散落 4 处 | `mod.rs` + 引用处 | 新增 `TEXT_PADDING` 常量统一引用 |
| 4.2 | font_size 硬编码 | `mod.rs` + 引用处 | 新增 `FONT_SIZE_RATIO` 常量统一引用 |
| 4.3 | LINE_HEIGHT 不一致 | `mod.rs` + `input.rs` | 新增 `LINE_HEIGHT_REMS` 常量统一引用 |
| 4.4 | wrap width 硬编码 800px | `state.rs:455` + `text_wrapper.rs` | 使用 `input_bounds.size.width`；None 时用 100000px 禁用 wrap |
| 4.7 | visible_range 硬编码 0..1 | `element.rs:77` | 根据 `bounds.size.height / line_height` 计算可见行范围 |
| 4.8 | bounds_for_range 不正确 | `state.rs` 两处 | 根据 `range_utf16` 实际测量文本像素位置返回 Bounds |

#### P1 性能 ✅

| # | 问题 | 文件 | 修复方式 |
|---|------|------|----------|
| 3.1 | RopeExt 全文 to_string | `rope_ext.rs` | 改用 ropey 原生 API（`line_to_byte_idx`、`byte_to_line_idx`、`char_indices()` 等） |
| 3.2 | 逐字符 shape_line | `state.rs` | 整行 shape 一次 + `closest_index_for_x` 查找 |
| 3.4 | 多余 Rope 拷贝 | `state.rs:556` | `&Rope::from(self.text.to_string())` → `&self.text` |

#### P2 API/Panic ✅

| # | 问题 | 文件 | 修复方式 |
|---|------|------|----------|
| 5.1 | builder 空壳方法 | `input.rs` | 移除 `gap_0`/`rounded_none`/`shadow_none`/`small`/`xsmall` |
| 5.2 | 重复 character_index_for_point | `state.rs` | 删除 EntityInputHandler 中重复实现 |
| 6.1 | slice_line end=0 underflow | `rope_ext.rs` | 增加 `end > 0` 保护 |
| 6.2 | From<Range> 方向不确定 | `cursor.rs` | 规范化确保 `start <= end` |

### 待修复（需架构级改动）

| # | 问题 | 原因 |
|---|------|------|
| 2.6 | MaskPattern 不生效 | 需要在输入管道中增加 mask 验证层，涉及 `InputState` 构建和 `replace_and_expand_common_prefix` 改造 |
| 4.5 | AutoGrow 不更新高度 | 需要 `Input` 感知 `mode.rows()` 并动态设置容器高度 |
| 4.6 | 滚动未实现 | 需要完整的 `ScrollHandle` + `scroll_size` + `Scrollbar` 集成 |

### 变更统计

```
9 files changed, 344 insertions(+), 268 deletions(-)
```
