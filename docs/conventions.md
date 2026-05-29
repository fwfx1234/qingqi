# Qingqi 工程约定 (Conventions)

> 本文是从 0 定的**硬约定**，目标只有两个：**写出好代码** 与 **高性能 UI**。
> 与 [architecture.md](architecture.md)（目标架构/设计）配套——架构讲"系统长什么样"，
> 本文讲"代码怎么写才一致、才快"。冲突时：架构边界以 architecture.md 为准，编码规则以本文为准。
>
> 用词：**必须 / 禁止 / 应当 / 可以**。带 ✅/❌ 的是正反例。

---

## 1. 分层与依赖方向（项目级）

四层，依赖**严格单向**，禁止反向或环：

```text
platform   OS 封装（clipboard/apps/shell/hotkey/tray）—— 不依赖任何上层
core       契约与纯逻辑（Plugin/Command/Manifest/打分/存储/快捷键）—— 只依赖 std/gpui
features   内建插件 —— 依赖 core + platform，禁止依赖其它 feature
app        装配层（启动器/窗口/主题）—— 依赖 core/features/platform
```

铁规则：
- **core 禁止依赖 features / platform 的实现细节**，禁止依赖 GPUI 渲染树以外的 UI。
- **feature 之间禁止互相依赖**。需要共享 → 抽到 core 或经 `BuildCx` 注入共享服务。
- **platform 禁止依赖 feature UI**。
- 跨层只通过 **trait + owned 数据类型**，不传具体类型、不传 `&'static str`（见架构 §3.2）。

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
- `Session` 概念被架构的 `PluginView`/`WindowView`/`InlineView`/`ListView` 取代。

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
        view.vm = ViewModel::build(&data);
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
- **优先 push 不轮询**：用 `cx.observe`/`cx.subscribe`/事件总线 + revision 驱动刷新（见架构 §6.3），
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

## 7. 错误处理与日志

### 7.1 错误
- 跨边界/可恢复错误用 **`anyhow::Result`** + `?` 传播；需要分支判断的错误才定义具体 `enum`(`thiserror`)。
- **禁止 `unwrap()`/`expect()`** 出现在非测试、非"已证明的不变量"代码里。锁、IO、解析、查找都要处理。
- 错误要**带上下文**：`.with_context(|| format!("open db {path}"))`，不要吞成 `?` 后无信息。
- 失败要么向上传，要么 log 后**优雅降级**（返回空/默认），禁止静默丢弃后续行为异常。

### 7.2 日志（`tracing`）
- 级别语义固定：
  - `error`：功能失败、需要关注（含 catch 到的 panic）。
  - `warn`：可恢复异常、降级、过期/丢弃。
  - `info`：少量关键生命周期事件（启动、注册完成）。
  - `debug`：开发期诊断（窗口步骤、耗时）。
  - `trace`：极细粒度，默认关。
- **用结构化字段**，不要把变量拼进字符串：✅ `tracing::warn!(plugin_id, error = %e, "open failed")`。
- **禁止在 render、热循环、每帧路径打日志。**

---

## 8. 测试约定

- **service/store/model 必须可不启动 GPUI 测试**（事实来源在这层，逻辑在这层）。
- 必测：命令匹配/打分、命令缓存失效、解析/校验、存储迁移、service 快照、job 状态机、
  `PluginView` 路由与 `manifest.mode` 一致性。
- GPUI 视图层改动可不强求广测，但**禁止削弱 service/store 测试**。
- 测试命名 `behaviour_under_condition`，一个测试一个断言意图。

---

## 9. 提交前检查门槛（必须全过）

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

## 10. 速查清单（Do / Don't）

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
