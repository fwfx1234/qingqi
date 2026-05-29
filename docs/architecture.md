# Qingqi 启动器架构设计

> 本文是 Qingqi 的**权威架构设计文档**，描述目标架构（target architecture），不是对当前
> 代码的描述。当前代码与本文不一致的地方，以本文为准，按第 13 章的迁移路线逐步收敛。
>
> 配套文档：[conventions.md](conventions.md) —— 编码约定（分层/命名/异步/高性能 UI）。
> 本文讲"系统长什么样"，conventions 讲"代码怎么写"。

## 0. 一句话定位

Qingqi 是一个 **Rust + GPUI 的本地启动器**：唤起 → 搜索 → 回车，**快速启动 App 和插件**。
插件有三种形态——**内联(Inline) / 列表(List) / 独立窗口(Window)**。第一期全部内建，
但架构预留**第三方插件**入口，将来接入时核心不重写。

---

## 1. 产品目标与设计约束

### 1.1 目标
- **快**：唤起到可输入、输入到出结果，都要快。热路径不碰重逻辑。
- **统一入口**：App 启动、插件命令、上下文动作在同一个搜索框里排序呈现。
- **三种插件形态**：Inline / List / Window，各自有清晰的宿主与生命周期。
- **本地优先**：本地数据、本地执行，少运行时耦合。

### 1.2 关键约束（决定架构的硬条件）
1. **App 不是插件**。App 启动是启动器的核心能力，直接内建，不走插件抽象（见第 4 章）。
2. **第三方插件画不了 GPUI**。第三方运行在子进程 / wasm 沙箱里，无法返回 GPUI 元素。
   这决定了"对第三方开放的视图必须是声明式数据"，是第 7 章入口设计的根因。
3. **核心只依赖 trait 与可序列化数据类型**，不依赖任何具体插件、不依赖 `&'static str`。
   这是"内建插件"与"第三方插件"能插进同一个插座的前提。

---

## 2. 总体架构

### 2.1 分层

```text
app/        GPUI 应用运行时：启动装配、启动器、窗口控制、主题、共享 UI 原语
              └─ 内建 App 源 (AppCatalog) 也在这一层，由启动器直接拥有
core/       插件契约 (Plugin trait + PluginView)、命令模型、注册表、存储、快捷键
features/   内建插件，一目录一个：manifest / service / store / view
platform/   OS 相关封装：clipboard / apps / shell / hotkey / tray
```

依赖方向（严格单向，不允许反向）：

```text
features ──> core ──> (std / gpui)
   │           ▲
   └──> platform┘
app ──> core, features, platform
```

- `core` 不依赖 `features` / `platform` 的实现细节，也不依赖 GPUI 渲染树以外的 UI。
- `platform` 不依赖 `features` 的 UI。
- `app` 负责把它们装配起来。

### 2.2 核心数据流：搜索 → 命中 → 激活

```text
                       唤起启动器
                          │
              ┌───────────┴───────────┐
              │     统一搜索/排序       │
              │  (一个 Vec<Command>)   │
              └───────────┬───────────┘
          ┌───────────────┴────────────────┐
   命令来源 A: 内建 App 源          命令来源 B: PluginManager
   (AppCatalog, app 层)            (内建插件 + 将来的第三方插件)
              │                              │
        Activation::Run                Activation::Open / Run
              │                              │
        ┌─────┴─────┐              ┌─────────┴──────────┐
   启动 App 并关闭   插件 Action    打开插件视图(Inline/List/Window)
                                          │
                              ┌───────────┼───────────┐
                          Inline        List        Window
                       (启动器内嵌)  (启动器内嵌)   (独立 OS 窗口)
```

**关键点**：App 和插件命令在**搜索/排序层统一**（都是 `Command`，共享打分与 usage 排序），
只在**激活层分叉**——App 走 `Activation::Run`（执行即关），插件走 `Activation::Open`（开视图）。

---

## 3. 核心概念与数据模型

所有跨边界的数据类型都 **owned + `#[derive(Serialize, Deserialize)]`**。第一期序列化用不上，
但它就是将来第三方 IPC 的线格式，现在加几乎零成本，事后补很痛。

### 3.1 `Command`（搜索/排序的统一单元）

```rust
// core/command.rs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Command {
    pub id: CommandId,             // 稳定 id，如 "json-parser.open" / "app:/Applications/Safari.app"
    pub source: CommandSource,     // 来源：内建 App / 某插件
    pub title: String,
    pub subtitle: String,
    pub icon: IconRef,
    pub keywords: Vec<String>,
    pub prefixes: Vec<String>,     // 命令前缀，如 "json" / "qr"
    pub usage_key: String,         // usage 排序键
    pub activation: Activation,    // 命中后做什么
    pub recommend: Vec<ContextMatcher>, // 上下文加权（保留现有 command.rs 的打分模型）
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CommandSource {
    App,                  // 内建 App 源
    Plugin(PluginId),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Activation {
    Run(Action),              // 执行后关闭启动器（App 启动、插件快捷动作）
    Open { plugin: PluginId },// 打开插件视图（Inline/List/Window）
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    LaunchApp { path: String },                                   // 内建：启动 App
    Plugin { plugin: PluginId, action: String, payload: Option<String> }, // 插件动作
}
```

> **保留**：现有 `command.rs` 的上下文打分（prefix / input_kinds / clipboard_kinds /
> `ContextMatcher`）设计得很好，整体迁移过来，只把 `CommandItem` 重命名/收敛为 `Command`，
> 把 `CommandTarget` 升级为 `Activation` + `Action`（新增 `LaunchApp`）。

### 3.2 `Manifest`（插件元数据，owned）

```rust
// core/plugin.rs
pub type PluginId = Arc<str>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub id: PluginId,
    pub name: Arc<str>,
    pub description: Arc<str>,
    pub icon: IconRef,
    pub keywords: Vec<Arc<str>>,
    pub prefixes: Vec<Arc<str>>,
    pub mode: ViewMode,          // Inline | List | Window —— 开窗前就要知道往哪放、多大
    pub window: WindowSpec,      // 尺寸 / 置顶（沿用现有 plugin_spec.rs）
    pub category: Category,
    pub background: bool,        // 是否需要后台任务
    pub dynamic_commands: bool,  // commands() 是否依赖 query（见 6.3）
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode { Inline, List, Window }
```

> **关键改动**：现有 `PluginManifest` 是 `Copy + &'static str`，对内建零分配很爽，但
> **文件加载的第三方 manifest 不可能是 `'static`**。所以核心消费的类型必须 owned（`Arc<str>`）。
> 内建插件仍可用 `&'static str` 常量，注册时 `.into()` 转一下。**这是留给第三方最关键的一道门**，
> 且因为 `&'static str` 现在 threaded 得到处都是，越晚改越痛。

### 3.3 `PluginView`（三种形态，类型化枚举）

这是整个设计的**正中心**，直接建模你的产品需求：

```rust
// core/plugin.rs
pub enum PluginView {
    Inline(Box<dyn InlineView>),
    List(Box<dyn ListView>),
    Window(Box<dyn WindowView>),
}

/// 独立窗口：自己管输入和窗口 chrome，host 只负责 render。
pub trait WindowView {
    fn render(&mut self, window: &mut Window, cx: &mut App) -> AnyElement;
    fn on_reopen(&mut self, _window: &mut Window, _cx: &mut App) {}
    fn on_close(&mut self) {}
}

/// 内联：嵌在启动器窗口里，共享启动器的搜索框输入。
pub trait InlineView {
    fn render(&mut self, window: &mut Window, cx: &mut App) -> AnyElement;
    fn on_input(&mut self, text: &str, cx: &mut App);
}

/// 列表：host 提供输入与列表 UI，插件只产数据 + 处理选择。
/// 注意：items() 返回的是**数据**，这天然就是将来第三方最先能开放的形态。
pub trait ListView {
    fn items(&mut self, query: &str, cx: &mut App) -> Vec<ListItem>;
    fn on_select(&mut self, item_id: &str, cx: &mut App) {}
    fn on_enter(&mut self, cx: &mut App) -> bool { false } // 返回 true = 已处理
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: IconRef,
    pub enabled: bool,
}
```

> **对比现状**：现有 `PluginSession` 把三种形态的 8 个方法揉成一个宽 trait，List 插件忘了实现
> `list_items` 会**静默返回空**。拆成枚举 + 三个窄 trait 后，模式和方法被类型绑死——List 视图缺
> `items()` 直接编译不过。`manifest.mode` 与 `open()` 返回的变体应一致，加一个 `debug_assert` 防手滑。

### 3.4 `ViewModel`（声明式视图，第三方契约——第一期只定义，不渲染）

第三方画不了 GPUI，所以它们只能产**声明式数据**，由 host 渲染。第一期只定义类型占位，
不实现渲染器；将来 `RemotePlugin` 的视图适配器把它渲染成 `AnyElement`（见第 7 章）。

```rust
// core/view_model.rs  —— 第一期定义，phase 2 才有渲染器
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ViewModel {
    List   { items: Vec<ListItem>, actions: Vec<Action> },
    Detail { markdown: String, actions: Vec<Action> },
    Form   { fields: Vec<Field>, actions: Vec<Action> },
}
```

> **为什么不在 `PluginView` 里加 `Rendered(ViewModel)` 臂**：因为 `ListView::items()` 本身就返回
> 数据，远程插件直接实现 `ListView`（内部转 IPC）即可；`WindowView`/`InlineView` 的远程适配器
> 持有一个 `ViewModel`，用 host 的渲染器渲成 `AnyElement`。**视图 trait 就是接缝，`ViewModel`
> 只是远程适配器的内部表示**，不污染 `PluginView` 枚举。

---

## 4. 启动器核心：App 内建（不是插件）

App 启动是启动器的**核心能力**，直接做进 `app` 层，**不包装成插件**。

### 4.1 `AppCatalog`

把现有 `features/app_launcher/service.rs` 的 `AppIndexService`（扫描 / 索引 / 搜索 / usage）
**提升**到 `app` 层，删除其插件外壳（`features/app_launcher/plugin.rs` 等）。

```rust
// app/app_catalog.rs（由 features/app_launcher 提升而来）
pub struct AppCatalog {
    index: Arc<AppIndexService>,   // 复用现有索引/扫描逻辑
}

impl AppCatalog {
    /// 返回 App 命令（activation = Run(LaunchApp)）。供启动器搜索直接调用。
    pub fn search(&self, query: &str, limit: usize) -> Vec<Command>;
    /// 启动 App。
    pub fn launch(&self, path: &str) -> anyhow::Result<()>;
    /// 后台扫描；扫描完成后 push 失效（发 CommandsChanged），不轮询。
    pub fn start_background(&self, cx: &mut App);
}
```

### 4.2 启动器如何合并两类命令

启动器同时持有 `AppCatalog` 和 `PluginManager`，把两边结果合并进一个排序列表：

```rust
// app/launcher.rs（示意）
fn collect_commands(&mut self, query: &Query, cx: &mut App) -> Vec<Command> {
    let mut out = Vec::new();
    out.extend(self.app_catalog.search(query.body(), APP_LIMIT)); // 来源 A：内建 App
    out.extend(self.plugins.borrow_mut().commands(query));         // 来源 B：插件
    rank(out, query, &self.usage)                                  // 统一打分 + usage 排序
}
```

> **收益**：现有 `app_launcher` 是唯一一个 override 了 `commands` / `commands_for_query` /
> `commands_revision` 的"插件"——动态命令那套机器本来大半就是为它和 quick-launch 准备的。
> App 内建后，它从插件模型里消失，插件契约因此可以更简单（见 6.3）。
>
> **扩展点**：将来若要别的"常驻、无视图"的内建来源（计算器、系统命令、剪贴板上下文动作），
> 在 `collect_commands` 的合并点加一个来源即可，无需做成插件。

---

## 5. 插件契约

### 5.1 两段生命周期：Plugin（长） / PluginView（窗口级）

| | `Plugin`（runtime） | `PluginView`（session/view） |
|---|---|---|
| 生命周期 | 应用级，常驻，便宜 | 打开时创建，关闭时释放 |
| 拥有 | `Arc<Service>`、后台 handle、轻量缓存、manifest | GPUI `Entity<T>`、输入/选择状态、订阅、渲染缓存 |
| **不可**拥有 | GPUI 渲染树、窗口 handle、大缓冲 | —— |
| 释放 | `shutdown()` | `on_close()` / drop 时释放大列表、预览、editor buffer、订阅 |

### 5.2 `Plugin` trait（精简核心 + 默认方法）

```rust
// core/plugin.rs
pub trait Plugin {
    fn manifest(&self) -> &Manifest;

    /// 贡献命令。静态插件无视 query（默认返回单条"打开"命令）；
    /// 动态插件（manifest.dynamic_commands = true）用 query 产命令。
    fn commands(&self, _query: &Query) -> Vec<Command> {
        vec![Command::open(self.manifest())]
    }

    /// 打开视图。返回与 manifest.mode 一致的变体。
    fn open(&mut self, cx: &mut PluginCx) -> anyhow::Result<PluginView>;

    // —— 以下按需实现，其余插件无视 ——
    fn handle(&mut self, _action: &Action, _cx: &mut PluginCx) -> anyhow::Result<Outcome> {
        Ok(Outcome::default())
    }
    fn shortcuts(&self) -> Vec<ShortcutDescriptor> { Vec::new() }
    fn start_background(&mut self, _cx: &mut PluginCx) {}
    fn shutdown(&mut self) {}
}
```

> **不拆成一堆 capability 对象**：对 ~12 个内建插件，"精简 trait + 默认方法"比能力对象更简单。
> `commands()` 的默认实现（单条 open 命令）就是现在 `core/plugin.rs:82` 那个好设计，保留。

### 5.3 `PluginCx`（运行时上下文）

```rust
pub struct PluginCx<'a> {
    pub app: &'a mut App,            // GPUI
    pub events: &'a AppEventBus,
}
impl PluginCx<'_> {
    /// 命令源变化时调用 → 启动器 push 失效命令缓存（替代轮询 revision）。
    pub fn notify_commands_changed(&self, plugin: &PluginId);
}
```

> 共享服务（db / paths / theme）**在构造期**通过 `BuildCx` 注入并被插件结构体捕获（见第 6 章），
> 不在运行时 `PluginCx` 里传。

---

## 6. PluginManager 与注册

### 6.1 职责边界

`PluginManager` 只做插件 runtime 协调：注册、manifest 收集、命令缓存、命令查询委派、
隔离边界、后台启动、idle 关闭。

**不做**：开窗 / 存窗口 handle / 调平台 API / 知道插件 UI 状态 / 拥有全局刷新策略。
窗口行为属于 `app/window_controller.rs`。

### 6.2 声明式注册 + DI（修掉 DB 时序与 clipboard 特例）

```rust
// core/registry.rs
pub struct PluginDescriptor {
    pub manifest: Manifest,
    pub databases: Vec<DatabaseSpec>,  // 声明 DB schema
    pub source: PluginSource,          // 留给第三方
}

#[derive(Clone, Copy)]
pub enum PluginSource { Builtin, External /* (path) —— phase 2 */ }

/// 构造期上下文：共享服务从这里取。
pub struct BuildCx<'a> {
    pub db: &'a Arc<DatabaseService>,
    pub paths: &'a AppPaths,
    pub theme: &'a Arc<Mutex<ThemeStore>>,
    pub events: &'a AppEventBus,
}

pub struct FeatureRegistry { /* ... */ }
impl FeatureRegistry {
    pub fn register<F>(&mut self, descriptor: PluginDescriptor, build: F)
    where F: FnOnce(&BuildCx) -> anyhow::Result<Box<dyn Plugin>>;

    /// 装配：对每个 entry —— 先注册 databases，再调 build(cx)，最后 manager.register。
    pub fn build_all(self, cx: &BuildCx, manager: &mut PluginManager) -> anyhow::Result<()>;
}
```

注册点（替代现有 120 行手工装配的 `register_builtin_plugins`）：

```rust
registry.register(JsonParser::descriptor(),  |cx| Ok(Box::new(JsonParser::build(cx)?)));
registry.register(Clipboard::descriptor(),   |cx| Ok(Box::new(Clipboard::build(cx)?)));
// ... 一行一个
```

**这一下解决三件现状的麻烦**：
1. **DB 时序**：schema 在 descriptor 里声明，框架保证"先注册 schema，再构造插件"——
   急切 open（http-capture / quick-launch）和懒加载（api-debugger / ftp）走**同一条路**，
   现有 `registry.rs` 里的手工预注册和双重注册全消失。
2. **clipboard 特例**：它要和 `WindowController` 共享 service？让 controller 也从 `BuildCx` /
   共享句柄取，不必再逃逸到 `app/runtime.rs` 单独注册。
3. **装配臃肿**：变成一行一个 `register`，新增插件只动这一处 + 它自己的目录。

### 6.3 命令缓存：push 失效，不轮询

```text
静态命令集： build_all 后，对所有插件调一次 commands(empty)，缓存。
            插件命令源变化 → notify_commands_changed → manager 失效缓存。  ← push
动态命令：   仅对 manifest.dynamic_commands = true 的插件，按当前 query 调 commands(query)，
            与缓存合并。                                                  ← 按需，非全量
```

> **对比现状**：现在每次按键都 `commands_revision()` 遍历**所有** runtime 做 `fold(*31)` 哈希 +
> 对所有 runtime 调 `commands_for_query`。改成 push 失效 + `dynamic_commands` 标记后，绝大多数
> 静态插件每次按键零成本，只有真正动态的（如 quick-launch）才被 query。`commands_revision()`
> 和那个 fold-hash 一并删除。

### 6.4 隔离边界：只留一道

第一期插件全内建（静态链接），**不需要在每个调用点裹 `catch_unwind`**。只在**激活派发**
那一层留一道边界——一个插件在打开/处理命令时 panic，不连累启动器与其它插件窗口。
将来这道边界对远程插件**自然变成进程/wasm 边界**。

> 现状 `PluginManager` 里约 10 处 `catch_unwind` 收敛成 1 处，去掉近 40% 样板。

---

## 7. 第三方插件入口（Phase 2 接缝，第一期只留口）

### 7.1 接缝在哪：`trait Plugin` + 可序列化数据

核心（manager / launcher / window_controller）**只认 `trait Plugin` 和数据类型**。于是第三方
插件不是新类型，而是 host 侧一个 **`RemotePlugin` 适配器**——它**也实现 `trait Plugin`**，
把每次调用翻译成 IPC：

```text
                      ┌──────────────────────────────┐
   PluginManager ────>│   trait Plugin  (唯一接缝)    │
   launcher           └─────┬──────────────────┬──────┘
   window_controller        │                  │
                     内建插件(原生 GPUI)   RemotePlugin (适配器)
                                                │ impl Plugin，转 IPC
                                         ┌──────┴───────┐
                                        子进程 / wasm（第三方，任意语言）
                                        JSON-RPC over stdio
```

对 `PluginManager` 来说，内建和第三方都是 `Box<dyn Plugin>`，**manager 与 launcher 一行不改**。

视图同理：`RemotePlugin::open()` 返回的 `PluginView::List(RemoteListView)`，其 `items()` 走 IPC
取数据；`Window`/`Inline` 的远程视图持有 `ViewModel`，用 host 渲染器渲成 `AnyElement`。

### 7.2 第一期必须做的 5 条纪律（= 留门，几乎零成本）

1. **核心只依赖 `trait Plugin` + owned 数据**，不依赖具体插件、不依赖 `&'static str`。
   → 即第 3.2 节：`Manifest` / `PluginId` 用 `Arc<str>`；`plugin_id()` 之类返回 owned。
2. **`Manifest` / `Command` / `Action` / `ListItem` / `ViewModel` 全部 derive serde**。将来的线格式。
3. **`open()` 返回 `PluginView` 三变体枚举**（窄 view trait 即接缝）。第一期只有内建实现。
4. **registry 带 `PluginSource { Builtin, External }`**，第一期只用 `Builtin`，留一个"将来 loader
   把 `RemotePlugin` 塞进同一个 manager"的位置。
5. **隔离只留一道**（6.4），将来对 remote 自然变成进程边界。

### 7.3 明确推迟（第一期不要碰）

JSON-RPC / wasm transport、`RemotePlugin` 适配器、`ViewModel` 的 host 渲染器（List/Detail/Form）、
插件目录发现 + `manifest.toml` 解析、权限/版本/沙箱策略。**因为接缝留对了，这些都是增量，不是重写。**

### 7.4 Phase 2 真接入时的步骤（仅备忘，核心不动）
- transport 选型：**子进程 + JSON-RPC over stdio 起步**（语言无关、最简单、Raycast 验证过），
  wasm 作为同一 `RemotePlugin` 接缝后面的可替换项。
- 写 `RemotePlugin: impl Plugin`，方法转 RPC。
- 写发现器：扫插件目录 → 读 `manifest.toml` → 产 `RemotePlugin` 注册进 `PluginManager`。
- 实现 `ViewModel` 的 List/Detail/Form 渲染器（**List 优先**，因其数据结构第一期就对了）。

---

## 8. 目标模块布局

```text
src/
  app/
    runtime.rs            仅启动装配（tracing/paths/registry/menus/background 启动）
    launcher.rs           搜索 + Inline/List 插件宿主 + 合并 AppCatalog 与插件命令
    window_controller.rs  独立窗口(Window 模式)生命周期：开/激活/记忆/清理
    app_catalog.rs        【新】内建 App 源（由 features/app_launcher 提升而来）
    background.rs         app 级循环：tray / hotkey / theme
    events.rs             revision 通知总线（保留现有 AppEventBus）
    ui.rs / theme*.rs     共享 GPUI 原语与主题
  core/
    plugin.rs             Plugin trait、PluginView、InlineView/ListView/WindowView、Manifest
    command.rs            Command / Activation / Action / 上下文打分（保留现有打分）
    view_model.rs         【新】ViewModel（声明式，第一期仅定义）
    registry.rs           【新】FeatureRegistry / PluginDescriptor / BuildCx / PluginSource
    plugin_spec.rs        WindowSpec / Category 等视觉规格
    database.rs           DatabaseService / DatabaseSpec
    storage.rs            AppPaths
    shortcut.rs           快捷键
  features/
    <plugin>/             manifest.rs / service.rs / store.rs / view(/) / plugin.rs
                          （app_launcher 目录删除）
  platform/               clipboard / apps / shell / hotkey / tray
```

---

## 9. 关键时序

### 9.1 启动 App
```text
唤起 → 输入 "saf" → launcher.collect_commands
  → app_catalog.search("saf") 返回 Safari (Activation::Run(LaunchApp{path}))
  → 回车 → app_catalog.launch(path) → 关闭启动器
```

### 9.2 打开插件视图
```text
输入 "json" → 命中 json-parser 的 open 命令 (Activation::Open{plugin})
  → 回车
     ├─ manifest.mode = Inline/List → launcher 内嵌：plugin.open() 得到 PluginView，宿主在启动器内渲染
     └─ manifest.mode = Window      → window_controller 开独立窗口，渲染 WindowView
```

### 9.3 插件数据变化（push，不轮询）
```text
插件后台 watcher 检测到 service 变化
  → PluginCx::notify_commands_changed(plugin)   （或发 CommandsChanged 事件）
  → PluginManager 失效该插件的静态命令缓存
  → launcher 下次 collect_commands 重新合并
```

---

## 10. 状态与所有权规则

- **视图状态统一用 GPUI `Entity<T>`**（框架原生响应式）。废弃现有 `Rc<RefCell<Panel>>` 与
  `Entity<Panel>` 两套并存——只留 `Entity<T>`。
- 共享服务默认 `Arc<Service>`；`Mutex/RwLock` 只锁服务内部具体可变状态，不锁整个服务。
- 便宜的 revision / worker flag 用 atomics。
- 后台到 UI 用 channel 或 service snapshot；**不要把 `Rc<RefCell<_>>` 放进后台服务**。
- 任何锁都**不可跨慢 IO / 网络 / 压缩 / 进程等待 / DB 扫描**持有。
- `Rc<RefCell<_>>` 只用于 GPUI 主线程上的窗口级状态。

---

## 11. 性能设计（"快"落到实处）

1. **热路径只碰缓存命令索引**：唤起→输入→回车全程读 `Vec<Command>`，不进插件重逻辑。
2. **视图懒构造**：DB / service / watcher 只在 `open()` 时建，搜索阶段零成本。
   （保留现有 `service()` 懒加载方向，统一收口到 `BuildCx` + 下条的 `RevisionedService`。）
3. **App 扫描后台异步**，扫完 push 失效，首屏不阻塞。
4. **push 替轮询**（6.3）：静态插件每次按键零成本，无 N 个常驻 timer。
5. **抽出 `RevisionedService<T>` 基建**：封装 `state + revision() + subscribe() + 后台同步`，
   把现在 api-debugger / ftp / app / quick-launch / clipboard 里复制 5 遍的
   `Option<Arc<Service>> + watch_started + ensure_watcher` 收口成一处。

---

## 12. 测试策略

可不依赖 GPUI 测的，必须有测试：
- 命令匹配与打分（`command.rs` 的 prefix / context / score）。
- 命令缓存失效（push 后缓存重建；`dynamic_commands` 仅对动态插件 query）。
- `AppCatalog` 搜索 / usage 排序。
- service / store 的快照、迁移、解析、job 状态转换。
- `PluginView` 路由：mode 与返回变体一致（`debug_assert` + 单测）。

GPUI 视图层改动可不强求广测，但不得削弱 service/store 测试。

测试门槛：
```bash
cargo fmt
cargo check
cargo test
```

---

## 13. 从现状迁移（映射 + 分步）

### 13.1 现状 → 目标 映射

| 现状 | 目标 |
|---|---|
| `PluginRuntime`（god trait，含 commands_revision/commands_for_query） | `Plugin`（精简，owned manifest，`commands(query)` 统一，无 revision） |
| `PluginSession`（宽 trait，8 方法） | `PluginView` 枚举 + `InlineView`/`ListView`/`WindowView` 窄 trait |
| `PluginManifest`（`Copy` + `&'static str`） | `Manifest`（owned `Arc<str>` + serde） |
| `ConfiguredPluginRuntime` / `PanelPluginSession`（fn 指针 builder） | trait 精简后基本不需要，删除或留极薄 helper |
| `features/app_launcher/*`（插件） | `app/app_catalog.rs`（内建，提升 `AppIndexService`，删插件外壳） |
| `register_builtin_plugins`（手工 120 行 + 手工预注册 DB） | `FeatureRegistry` + `PluginDescriptor`（声明 DB，先注册后构造） |
| clipboard 在 `app/runtime.rs` 特例注册 | 经 `BuildCx` 正常注册 |
| `commands_revision` 轮询 + fold-hash | push 失效 + `manifest.dynamic_commands` |
| `Rc<RefCell<Panel>>` 与 `Entity<Panel>` 并存 | 统一 `Entity<T>` |
| 约 10 处 `catch_unwind` | 1 处（激活派发边界） |
| —— | 新增：`Activation`/`Action::LaunchApp`、`PluginSource`、`ViewModel`、全量 serde derive |

### 13.2 分步实施（每步独立可编译、可验证）

> 原则：小步、可回退、每步 `cargo check` 通过。先在 1～2 个插件上跑通再铺开。

- **M1 — Manifest owned 化 + serde（机械）**
  `PluginManifest` → owned `Manifest`（`Arc<str>`），全链路去 `&'static str`；给跨界数据加
  `derive(Serialize, Deserialize)`。内建用 `&'static str` 常量 `.into()`。
- **M2 — 视图枚举（核心收益）**
  引入 `PluginView` + `InlineView`/`ListView`/`WindowView`，拆掉宽 `PluginSession`。
  先迁 `json_parser`(Inline) + `http_capture`(Window) + 一个 List 插件，验证三种宿主路由。
- **M3 — 注册表 + DI**
  引入 `FeatureRegistry`/`PluginDescriptor`/`BuildCx`，先注册 DB 再构造，**修掉 DB 时序**，
  **去掉 clipboard 特例**。
- **M4 — App 内建**
  `AppIndexService` 提升到 `app/app_catalog.rs`，启动器合并 App 命令；删除 `features/app_launcher`。
- **M5 — push 失效**
  `notify_commands_changed` + `dynamic_commands` 替换 `commands_revision` 轮询与 fold-hash；
  抽 `RevisionedService<T>` 收口 watcher 样板。
- **M6 — 留口（收尾）**
  定义 `ViewModel`、`PluginSource::External` 占位、隔离边界收敛到一处。**不写**任何远程实现。

- **Phase 2（独立里程碑，非第一期）**
  `RemotePlugin` + JSON-RPC transport + `ViewModel` 渲染器 + 插件发现/`manifest.toml`。

---

## 14. 设计取舍记录（为什么这么定）

- **App 内建而非插件**：App 是唯一"无视图、launch-and-go"的东西，硬塞进"打开视图"的插件模型
  本就别扭（它是现状里唯一 override 全套动态命令机器的插件）。内建后插件契约更简单。
- **视图用枚举而非宽 trait**：把"模式 ↔ 方法"用类型绑死，消除 List 插件静默失效一类的隐患。
- **push 而非轮询**：避免每次按键全量遍历插件 + N 个常驻 timer；静态插件零成本。
- **第一期内建、留口不实现**：第三方是明确目标但非第一期。接缝（`trait Plugin` + serde 数据 +
  `ViewModel` + `PluginSource`）一旦留对，将来接入是增量。**最贵且最该现在做的是 Manifest 去
  `&'static str`**，因为它 threaded 到处都是，事后 retrofit 代价最高。
- **保留现有命令打分模型**：`command.rs` 的上下文打分（prefix/kinds/matcher）质量高，整体沿用。
```
