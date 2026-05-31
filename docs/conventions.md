# Qingqi 工程约定 (Conventions)

> 本文是从 0 定的**硬约定**，目标只有两个：**写出好代码** 与 **高性能 UI**。
> 与 [workspace-split-guide.md](workspace-split-guide.md)（workspace 拆分主导文档）配套——主导文档讲"系统长什么样"，
> 本文讲"代码怎么写才一致、才快"。冲突时：架构边界以 workspace-split-guide.md 为准，编码规则以本文为准。
>
> 用词：**必须 / 禁止 / 应当 / 可以**。带 ✅/❌ 的是正反例。

---

## 1. 分层与依赖方向（项目级）

当前仓库已完成 workspace 拆分；结构与边界以 [workspace-split-guide.md](workspace-split-guide.md) 为准。依赖必须**严格单向**，禁止反向或环：

```text
platform / qingqi-platform       OS 封装（clipboard/apps/shell/hotkey/tray）—— 不依赖任何上层
core / qingqi-plugin+core        SDK 契约与宿主纯逻辑（Plugin/Command/Manifest/打分/存储/快捷键）
features / qingqi-feature-*      内建插件 —— 依赖 plugin + ui + platform，禁止依赖其它 feature
app / qingqi-app                 GUI 外壳（启动器/窗口/主题）—— 不依赖具体插件
```

铁规则：
- **core 禁止依赖 features / platform 的实现细节**，禁止依赖 GPUI 渲染树以外的 UI。
- **feature 之间禁止互相依赖**。需要共享 → 抽到 `qingqi-plugin`/`qingqi-ui`，或经 `BuildCx` 注入共享服务。
- **platform 禁止依赖 feature UI**。
- 跨层只通过 **trait + owned 数据类型**，不传具体类型、不传 `&'static str`（见 workspace 拆分主导文档 §4）。

---

## 2. 插件内部分层（一个 feature 怎么切）

每个 feature 目录按职责切成固定几层，**职责单一、依赖单向**：

```text
features/<feature>/
  manifest.rs   元数据：id/name/icon/mode/prefixes/window。纯数据，禁止逻辑/IO。
  model.rs      领域类型 & DTO。serde。禁止 GPUI、禁止 IO。
  store.rs      持久化（SQLite）、迁移。禁止 GPUI、禁止业务逻辑、禁止平台调用(DB 除外)。
  service.rs    领域行为、编排、后台 worker。持有 Arc 状态，暴露 snapshot + revision。
                必须可不依赖 GPUI 测试。可用 platform 适配器。
  view(/).rs    GPUI 渲染 + 本地 UI 状态。只读 snapshot/view-model。
                禁止在 render 里 IO/DB/网络/加锁（见 §4）。
  plugin.rs     装配：impl Plugin，声明 descriptor，从 BuildCx 构造 service，open 出 view。
```

feature 内部依赖方向：

```text
plugin ──> service ──> store ──> model
  │           │                    ▲
  │           └──> platform 适配器  │
  └──> view ──> service(snapshot) ──┘   （view 只读 service 快照 + model）
manifest 独立，谁都可读
```

铁规则：
- **view 禁止直接碰 store**（不开 SQLite、不写迁移）。要数据 → 走 service 的 snapshot。
- **store 禁止知道 GPUI / 窗口 / session 状态**。
- **service 是唯一事实来源**：持有状态、跑 worker、给快照、维护便宜的 revision。
- 业务逻辑放 service/store，**先写测试再接 GPUI**。

---

## 3. 文件与命名约定

### 3.1 文件 / 模块
- 文件、模块：`snake_case`。一个文件一个清晰职责，**文件名即职责**（见 §2 那张表）。
- `mod.rs` 只做**模块声明 + 再导出**，禁止塞实现。
- 一个 view 超过一个清晰职责就拆 `view/`：
  ```text
  view/mod.rs       入口实体 + 装配（最小）
  view/vm.rs        view-model（render-ready 数据，见 §4）
  view/action.rs    UI 意图枚举 XxxAction（复杂交互才需要）
  view/sections/*   大页面按区域拆
  view/shared.rs    本 feature 内共享的小组件/样式
  ```

### 3.2 类型命名（**统一，终结现状的 Panel/View/Element 混用**）

| 角色 | 约定名 | 说明 |
|---|---|---|
| 插件装配体 | `XxxPlugin` | impl `Plugin`（取代旧 `XxxRuntime`） |
| GPUI 视图实体 | `XxxView` | `Entity<XxxView>` + `impl Render`。**一个视图一个实体** |
| render-ready 数据 | `XxxViewModel` | 见 §4，放 `view/vm.rs` |
| 领域服务 | `XxxService` | 事实来源 |
| 持久化 | `XxxStore` | |
| 服务快照 | `XxxSnapshot` | service 对外的便宜只读视图 |
| UI 意图 | `XxxAction` | 复杂 view 的事件枚举 |

**禁止**再出现：`XxxPanel`、`XxxElement`（`Rc<RefCell>` 包装壳）、`XxxSession`。
- `Panel` → 一律叫 `View`。
- `Element` 壳是为 `Rc<RefCell>` 服务的，统一 `Entity<T>` 后（§6）**直接消失**——`Entity<XxxView>`
  自身 `impl Render`，不需要包装壳。
- `Session` 概念被主导文档里的 `PluginView`/`WindowView`/`InlineView`/`ListView` 取代。
- 这里的 `XxxViewModel` 只指 feature 内部的 render-ready UI 数据，不是已废弃的 `core/view_model.rs` 声明式协议类型。

### 3.3 命名风格
- 类型 `UpperCamelCase`，函数/变量/字段 `snake_case`，常量 `SCREAMING_SNAKE_CASE`。
- 名字**表角色不表类型**：`elapsed_ms` 不叫 `num`；`launch(app)` 不叫 `do_it`。
- bool 用 `is_/has_/should_/can_` 前缀。
- 时间量带单位：`timeout_ms`、`debounce`。

---

## 4. 高性能 UI 铁律（重点）

GPUI 每次 `cx.notify()` 会重建该实体的元素树。**render 必须是纯、廉价、可被高频调用的函数。**
性能问题 99% 来自违反下面 5 条。

### 铁律 1 — `render` 纯且廉价：禁止 IO / 加锁 / 重计算
render 只允许**从已算好的状态读数据并拼元素**。

```rust
// ❌ 在 render 里查库、加锁、排序、格式化
fn render(&mut self, _w, _cx) -> impl IntoElement {
    let rows = self.store.lock().unwrap().query_all();   // IO + 锁 → 掉帧
    let mut rows = rows; rows.sort_by(by_time);           // 每帧排序
    div().children(rows.iter().map(|r| format_row(r)))    // 每帧格式化 + 分配
}

// ✅ render 只读 view-model
fn render(&mut self, _w, _cx) -> impl IntoElement {
    div().children(self.vm.rows.iter().cloned())          // O(可见行)，无 IO/无锁
}
```

禁止在 render 内：DB/网络/文件 IO、`lock()`、排序/过滤/聚合、正则编译、`format!` 大量字符串、
`Vec`/`String` 大分配、`std::process::Command`。

### 铁律 2 — view-model 模式：数据变了**算一次**，render 只读
状态实体持有一个 render-ready 的 `XxxViewModel`。数据变化时（在更新回调或异步完成处）重算一次并 `notify`。

```rust
struct ClipboardView { vm: ClipboardViewModel, service: Arc<ClipboardService> }

fn on_data_changed(&mut self, cx: &mut Context<Self>) {
    let snap = self.service.snapshot();        // 便宜的 Arc 快照
    self.vm = ClipboardViewModel::build(&snap); // 排序/格式化/截断只在这里发生
    cx.notify();                                // 只标脏自己
}
// render 全程只读 self.vm
```

### 铁律 3 — 大列表必须虚拟化
超过约一屏的列表**禁止**一次性 `children(...)` 全量渲染。用 GPUI 虚拟列表（`uniform_list`）
只渲染可见行；配合分页/增量加载（异步，见 §5）。

```rust
// ✅ 只渲染可见区
uniform_list(cx.entity(), "rows", self.vm.rows.len(), |this, range, _w, _cx| {
    this.vm.rows[range].iter().map(render_row).collect()
})
```

### 铁律 4 — 精准 `notify`，禁止全局重绘
- 实体内部状态变 → `cx.notify()`，**只重绘该实体**。
- 禁止用 `window.refresh()` / app 级重绘来刷普通 UI 或插件数据。仅当"回调已持有当前 window 且
  确需整窗重绘"时才用 `window.refresh()`。
- **实体保持细粒度**：把会独立变化的区域拆成子 `Entity<T>`，一处变化只重绘那一处。

### 铁律 5 — 实体/输入只创建一次
`TextInput`、列表滚动状态、editor 等**在构造时 `cx.new(...)` 创建一次**，存进 state，render 里引用。
**禁止在 render 里 `cx.new`**（会泄漏/抖动/丢状态）。

### 其它高性能习惯
- UI 文本用 **`SharedString`**（`Arc<str>` 背书，clone 廉价），不要每帧 `String::from`/`format!`。
- 传给元素的共享数据用 **`Arc<T>`**，不要深 clone 大结构进 render。
- 服务对 UI 暴露 **`snapshot() -> Arc<Snapshot>`**（便宜 clone）+ **revision**；UI 比对 revision
  决定是否重算 view-model，**render 永不加锁**。

---

## 5. 异步约定

UI 线程只做轻活；重活进后台，结果回主线程应用。

### 5.1 分工
- CPU/IO 重活：`cx.background_executor().spawn(async { ... })`。
- 定时：`background_executor().timer(dur).await`，**禁止** `std::thread::sleep`。
- 回 UI：`async_cx.update(...)` 或 `entity.update(cx, |e, cx| { ...; cx.notify() })`。

```rust
cx.spawn(async move |view, async_cx| {
    let data = async_cx.background_executor()
        .spawn(async move { service.scan() })   // 重活在后台线程
        .await;
    let _ = view.update(async_cx, |view, cx| {  // 回主线程应用
        view.vm = XxxViewModel::build(&data);
        cx.notify();
    });
}).detach();
```

### 5.2 generation guard：丢弃过期结果（必须）
任何由用户输入/动作触发的异步，完成时**必须校验自己仍是最新**，否则丢弃，避免闪烁/错位。

```rust
self.generation = self.generation.wrapping_add(1);
let gen = self.generation;
cx.spawn(async move |view, acx| {
    let r = do_async().await;
    let _ = view.update(acx, |view, cx| {
        if view.generation != gen { return; }   // 已被更新的输入取代 → 丢弃
        view.apply(r); cx.notify();
    });
}).detach();
```

### 5.3 输入去抖（必须）
搜索/即时联想类高频输入**必须去抖**（如 60–120ms）后再触发异步，配合 §5.2 的 generation。

### 5.4 后台循环：单一 owner + 防重复 + 可停
- **一个循环一个 owner**（app 级在 `app/background.rs`；feature 级在其 service/plugin）。
- **防重复启动**：`started: bool` 守卫。
- **可停**：保留 handle 或停止标志，`shutdown()`/`on_close` 时停；`detach()` 仅用于确实生命周期
  绑应用的循环，且仍要有明确 owner。
- **优先 push 不轮询**：用 `cx.observe`/`cx.subscribe`/事件总线 + revision 驱动刷新（见 workspace 拆分主导文档 §4/§5），
  少起常驻 timer。

### 5.5 锁纪律
- **禁止跨 `.await` 持锁**，禁止跨慢 IO/网络/压缩/进程等待/DB 扫描持锁。
- 锁只包**具体可变状态**，不锁整个 service。
- 锁中毒（poison）必须优雅处理（log + 降级），**禁止 `unwrap()` 一个锁**导致连锁 panic。

---

## 6. 状态与所有权

| 场景 | 用 | 禁止 |
|---|---|---|
| 参与渲染/通知的 UI 状态 | `Entity<T>` + `cx.notify()` | `Rc<RefCell<T>>` 当视图状态 |
| 共享服务句柄 | `Arc<Service>` | 把 `Rc<RefCell>` 放进后台/共享服务 |
| 服务内部可变状态 | `Mutex/RwLock` 包**具体字段** | 锁整个 service（除非过渡期） |
| 便宜 revision / worker flag | 原子（`AtomicU64`/`AtomicBool`） | 用锁存计数 |
| 后台→UI 通信 | channel 或 snapshot | 共享可变借用 |
| 仅 GPUI 主线程窗口级临时态 | `Rc<RefCell<_>>`（**唯一允许处**） | —— |

- **统一 `Entity<T>`** 作视图状态：终结现状 `Rc<RefCell<Panel>>` 与 `Entity<Panel>` 并存。
- 默认不可变；需要内部可变才上 cell/lock，且范围最小。

---

## 7. UI 组件与 gpui-component

项目依赖 `gpui-component`（高层控件库，`gpui_component::init(cx)` 已在 `qingqi-app` 的 runtime 初始化一次）。
UI 由三层组成，**交互控件优先 gpui-component，布局容器才用原生 `div()`**：

```text
1. gpui-component           首选：button、tab、form、badge/tag、switch、checkbox、slider、progress、table、编辑器、overlay 等交互控件
2. 项目 ui:: adapter/origin  当默认效果不满足项目视觉时，包 adapter/wrapper，统一 token、圆角、间距、hover/disabled/loading
3. 原生 GPUI div()          布局、容器、一次性简单元素；禁止用 div 手写库已提供的复杂控件
```

### 7.1 选型铁规则
- **先查 gpui-component**：button、tab/segmented、switch/checkbox、slider/progress、badge/tag、form 行、table、编辑器、overlay，默认优先使用组件库。
- **效果不满足就改造，不绕开**：通过主题覆盖、本地 adapter、项目级 wrapper 让组件服从项目 token；禁止因为默认样式不完全匹配就在插件 view 里重新手写一套按钮/chip/form。
- **大列表/表格必须虚拟化**：`gpui_component::table` 或虚拟 list（或 GPUI `uniform_list`）。行数可能
  上百、或单行渲染昂贵 → 一律虚拟化（呼应 §4 铁律 3）。**禁止 `div().children(全量)`。**
- **编辑器类输入**（脚本 / JSON / 日志 / 大文本）→ `gpui_component::input::InputState`（code 模式）。
  普通单行/简单字段可用项目 `qingqi_ui::text_input::TextInput` adapter。**禁止给普通字段套重型 code editor。**
- **简单按钮**：优先 `gpui_component::button::Button` 或项目对它的 adapter。**同一界面禁止混用多种按钮风格。**
- `div()` 只负责布局、容器、一次性简单元素；不为“用上库”把纯布局换成组件，也不为省事手写库已提供的复杂控件。

### 7.2 Root 规则（错用会 panic）
- overlay 类 API——`open_sheet` / `open_dialog` / 通知 / `InputState` 焦点管理——**要求窗口根是
  `gpui_component::Root`**。当前窗口根仍是 `Launcher`/`PluginWindow`，**未 Root 化**。
- **未 Root 化窗口禁止调用上述 overlay API**（`Root::read/update` 会 panic）。
- 未 Root 化时可安全使用：`button`/`tab`/`badge`/`tag`/`checkbox`/`switch`/`slider`/`progress`/
  布局助手 / 本地状态的虚拟 list·table。
- Root 化**按窗口单独做**，会改变 `downcast::<Launcher>()`/`downcast::<PluginWindow>()` 的句柄语义
  （见 workspace 拆分主导文档中的窗口生命周期边界）；**禁止在迁移别的插件时顺手 Root 化**。

### 7.3 内存/状态
- 编辑器/输入实体**构造时建一次**、复用，关闭时清大 buffer（呼应 §4 铁律 5、§5.4）。
- editor 特性（行号/高亮/LSP/补全/markdown/大 undo）**默认全关**，插件确需才逐个开。
- **禁止把 `InputState`/`Button`/`Tab` 等组件 UI 实体存进 service/store**（属 view 层）。

### 7.4 样式归属
gpui-component 必须服从项目主题 token（§8）。组件默认配色/圆角/间距与目标不符时，优先顺序是：

1. 配置组件自带样式参数；
2. 包本地 adapter 验证效果；
3. 两个及以上插件复用同一改造后，抽到项目公共 wrapper（`qingqi-ui` / `qingqi_ui::ui::components`）。

禁止因为默认效果不完全满足目标，就在 feature view 中长期保留第三套手写控件。

> 操作细节（Root 迁移步骤、内存 RSS 测量、组件选型表）见 [gpui-component-guide.md](gpui-component-guide.md)。
> 本节是规范（rules），该指南是操作手册（how-to）。

---

## 8. 主题与样式 token

三层 token 体系，**只准从上往下用，不准跨层**：

```text
palette   (`qingqi-ui::theme`)  原始色阶 slate_*/blue_*/violet_* + 间距 space_*。【内部层，feature 禁用】
语义 token (`qingqi-ui::ui`)     bg_canvas/bg_surface/text_primary/border_light/accent_color/status_color…
组件原语   (`qingqi-ui::ui`)     section_card/row_card/stat_card/ui_button/ui_card/ui_badge/ui_chip…
```

### 8.1 颜色
- **禁止裸 `rgb(0x..)` / `rgba` / hex 字面量**出现在 feature/view。颜色一律走 `ui::` 语义 token；
  缺 token 就去 `ui.rs` **加一个语义 token**，不在调用点硬编码。
- **禁止 feature 直接调 `theme::slate_500()` 等 palette 函数**。palette 是 `ui.rs` 的私有实现细节，
  语义 token 才是边界。
- token 命名表语义不表数值：`text_secondary` 不叫 `gray_400`，`bg_surface` 不叫 `white`。
- 组件原语暴露**语义入参**（如 status 枚举），不要暴露裸 `Rgba`（现状 `status_bar(_, Rgba)` 是反例）。

### 8.2 间距 / 圆角 / 字号
- 间距优先 `theme::space_*`（经 `ui::` 暴露），不要散落魔法数 `px(13.0)`。确需一次性微调可用 `px`，
  **但重复出现就提为 token**。
- 字号/圆角同理：重复值提 token，单点一次性可内联。

### 8.3 字体
- **禁止硬编码 `font_family("PingFang SC")`**（现状 58 处）。要么提供 `ui::font_ui()` 统一取，
  要么**在窗口根容器设一次字体、子元素继承**，调用点不再写字体名。

### 8.4 暗/亮主题
- 明暗由**主题上下文决定**，语义 token 内部读当前主题。**新代码禁止到处穿 `dark: bool`**
  （现状 `plugin_surface(dark)`/`ui_button(…dark…)` 是要收敛的反例）。

### 8.5 原语只留一套
当前 `ui::` 存在两代重叠原语（如 `badge` 与 `ui_badge`、`*_card` 与 `ui_card`）。**择一为准，另一标
legacy，新代码只用选定那套，禁止再造第三套。** 迁移时顺手统一。

---

## 9. 图标与资源

### 9.1 图标来源
- `assets/icons/<name>.svg` —— 第一方手绘 **SVG**，矢量、可着色、任意 DPI 清晰。**首选**。
- `assets/qta/*.png` —— 从 suishou 继承的 QtAwesome 预栅格 PNG（`mdi6.*`/`fa5s.*`）。**legacy**。

### 9.2 规则
- **新图标一律 `assets/icons/<kebab-name>.svg`**，矢量、命名表意（`download.svg` 不叫 `icon1.svg`）。
- **禁止新增 `qta/*.png`**；现存的随重构替换为 SVG。
- 引用集中常量化：各 feature 在 `manifest.rs` 顶部定 `const ICON: &str = "icons/json.svg"`，
  **不要在多处裸写图标路径字符串**。目标演进为 workspace 拆分主导文档 §4 的 `IconRef` 类型。
- SVG 取用走 `ui::icon_element` / `ui::icon_tile`；底层 `platform::svg_icon::rasterize_asset(path, size)`
  **按目标像素（含 DPI 倍数）栅格化**，不要固定位图缩放——保证清晰。
- 图标应为**单色 SVG + tint 着色**（颜色走语义 token），**禁止把颜色烧进资源**。
- 资源定位统一走 `qingqi-ui::assets`（`resolve`/`resolve_string`/`ProjectAssets`），
  **禁止 feature 自己拼绝对路径或判 dev/bundle 分支**。
- App/Tray 图标源文件 `assets/app-icon.svg`/`tray-icon.svg`，PNG 由 `build.rs` 生成；
  **禁止把生成的 `app_icon_*.png` 当源文件手改提交**。

---

## 10. 文案 / i18n（务实，不上重框架）

- 用户可见文案语气一致：简洁、动作名用动词开头。
- **禁止字符串拼接组装句子**（破坏将来 i18n、易错）。带变量用 `format!` 完整模板：✅ `format!("已打开 {name}")`。
- 现阶段文案中文、内联可接受，但**面向用户的字符串集中**放各 feature 的常量/`manifest`，
  不要散落在深层逻辑里——为将来 i18n 留口。
- **开发者向**（日志/错误）与**用户向**（UI 文案）分开：日志可英文 + 结构化字段（§11），UI 文案中文。

---

## 11. 错误处理与日志

### 11.1 错误
- 跨边界/可恢复错误用 **`anyhow::Result`** + `?` 传播；需要分支判断的错误才定义具体 `enum`(`thiserror`)。
- **禁止 `unwrap()`/`expect()`** 出现在非测试、非"已证明的不变量"代码里。锁、IO、解析、查找都要处理。
- 错误要**带上下文**：`.with_context(|| format!("open db {path}"))`，不要吞成 `?` 后无信息。
- 失败要么向上传，要么 log 后**优雅降级**（返回空/默认），禁止静默丢弃后续行为异常。

### 11.2 日志（`tracing`）
- 级别语义固定：
  - `error`：功能失败、需要关注（含 catch 到的 panic）。
  - `warn`：可恢复异常、降级、过期/丢弃。
  - `info`：少量关键生命周期事件（启动、注册完成）。
  - `debug`：开发期诊断（窗口步骤、耗时）。
  - `trace`：极细粒度，默认关。
- **用结构化字段**，不要把变量拼进字符串：✅ `tracing::warn!(plugin_id, error = %e, "open failed")`。
- **禁止在 render、热循环、每帧路径打日志。**

---

## 12. 测试约定

- **service/store/model 必须可不启动 GPUI 测试**（事实来源在这层，逻辑在这层）。
- 必测：命令匹配/打分、命令缓存失效、解析/校验、存储迁移、service 快照、job 状态机、
  `PluginView` 路由与 `manifest.mode` 一致性。
- GPUI 视图层改动可不强求广测，但**禁止削弱 service/store 测试**。
- 测试命名 `behaviour_under_condition`，一个测试一个断言意图。

---

## 13. 提交前检查门槛（必须全过）

```bash
cargo fmt
cargo clippy --all-targets
cargo check
cargo test
```

- `rustfmt` 默认配置，不手调格式。
- clippy 不留**新增**告警（存量另行收敛）。
- 新增编译错误/panic 不可交付。

---

## 14. 速查清单（Do / Don't）

**写 view 时**
- ✅ render 只读 `self.vm`；数据变了在回调/异步里重算一次 + `cx.notify()`
- ✅ 输入/列表实体构造时建一次；大列表 `uniform_list`
- ❌ render 里 IO / `lock()` / 排序格式化 / `cx.new` / `window.refresh()` 刷普通态

**写异步时**
- ✅ 重活进 `background_executor`，回主线程 `update`；generation guard；输入去抖
- ❌ 跨 `.await` 持锁；`thread::sleep`；起常驻 timer 轮询能 push 的东西

**写 service/store 时**
- ✅ service 持 `Arc` + 暴露 `snapshot()`+revision；逻辑先测；锁包具体字段
- ❌ store 碰 GPUI；view 直连 store；`unwrap` 锁/IO

**起名/分层时**
- ✅ `XxxPlugin/XxxView/XxxViewModel/XxxService/XxxStore/XxxSnapshot`；feature 不互相依赖
- ❌ `Panel/Element/Session` 命名；core 依赖 feature；跨层传 `&'static str`

**写 UI / 用 gpui-component 时**
- ✅ 低层优先（div→`ui::`→component）；大列表 table/虚拟化；编辑器才用 `InputState`；overlay 仅 Root 化窗口
- ❌ 普通字段套 `InputState`；未 Root 用 sheet/dialog；组件实体进 service/store；一界面混多种按钮风格

**写样式/图标时**
- ✅ 颜色走 `ui::` 语义 token；复用 `ui::`/component 原语；字体走 token 或根继承；新图标 `icons/*.svg` 矢量
- ❌ 裸 `rgb(0x..)`/hex；直接调 `theme::` palette；硬编码 `font_family`；穿 `dark:bool`；新增 `qta/*.png`
