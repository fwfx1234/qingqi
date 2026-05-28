# Qingqi Plugin Migration Guide

本文档是 Qingqi 长期迁移 `suishou` 插件的执行手册。目标读者是低级模型或接力工程师：拿到一个插件任务后，按本文档读取参考文件、建立 Rust 类型、迁移 service/store、复刻 GPUI 界面、补测试、更新矩阵。

## Mission

Qingqi 是 `/Users/fwfx1234/develop/suishou` 的 Rust + GPUI 复刻版。主界面可以更现代、更简洁；每个插件窗口必须以 suishou 的 QML 页面为视觉基准，逐步做到功能真实、结构 Rust 化、最终像素级接近。

硬性要求：

- `qml-demo` 不迁移。Qingqi 使用 `gpui-demo` 作为 GPUI 学习演示插件。
- 不需要兼容 suishou 的 PySide6、QML runtime、QObject ViewModel、Python 插件 API、旧数据库 schema 或动态加载方式。只参考功能和视觉，不保留兼容层。
- 业务逻辑必须能不启动 GPUI 单独测试。
- render/on_click 中不得执行长任务、网络、扫描、压缩、下载、数据库大查询。
- 新增插件不能只注册 stub。至少要有 manifest、真实输入、真实按钮行为、错误状态、空状态。
- 每次改动必须跑对应测试和 `cargo check`，并更新本文档矩阵。

## No Compatibility Policy

Qingqi 是 Rust/GPUI 原生复刻，不是 suishou 的兼容运行时。低级模型执行迁移时必须遵守：

- 不实现 QML loader，不嵌入 Qt/PySide，不保留 Python runtime bridge。
- 不让 Rust 类型模拟 QObject、Signal、Slot、Property。把它们翻译成 Rust enum、struct、trait、channel 和 DTO。
- 不迁移 suishou 用户数据 schema。Qingqi 每个插件自建 schema、版本号和 migration。
- 不为了兼容 suishou 的文件路径、配置字段或插件 entrypoint 牺牲 Rust 设计。
- `plugin.json` 只作为参考资料，最终 manifest 由 Rust `manifest.rs` 定义。
- QML 控件树不逐行翻译。只抽取功能、状态、布局尺寸、颜色和交互结果。
- 能用 Rust crate 或系统 API 做好的能力，不复制 Python 的实现细节。

判断标准：如果某个方案需要“让旧 Python/QML 也能跑”，就是错误方案；如果某个方案让 Rust service/store/view 边界更清晰，就是优先方案。

## Local Paths

```text
Qingqi repo      /Users/fwfx1234/develop/qingqi
Suishou repo     /Users/fwfx1234/develop/suishou
Reference QML    /Users/fwfx1234/develop/suishou/src/features/<plugin>/*Page.qml
Reference logic  /Users/fwfx1234/develop/suishou/src/features/<plugin>/*.py
Qingqi features  /Users/fwfx1234/develop/qingqi/src/features/<plugin>
Migration guide  /Users/fwfx1234/develop/qingqi/docs/migration-guide.md
Architecture     /Users/fwfx1234/develop/qingqi/docs/architecture.md
```

Rust 工具链在本机 PATH 中不一定可见。使用以下命令：

```bash
cd /Users/fwfx1234/develop/qingqi
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo fmt
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo test
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo check
```

## Current Architecture

当前代码已经有核心骨架：

- `src/core/plugin.rs`: `PluginRuntime`、`PluginSession`、`PluginManager`。
- `src/core/command.rs`: typed command，包含 `PluginOpen` 和 `PluginAction`。
- `src/app/runtime.rs`: GPUI application、窗口打开、插件注册、启动器命令分发。
- `src/app/launcher.rs`: 启动器搜索和插件打开。
- `src/app/text_input.rs`: 目前是增强过的单行输入，支持 `multiline` 标志，但还不是完整多行编辑器。
- `gpui-component`: 已在启动时初始化，可用于重复控件、表格、虚拟列表和真正编辑器场景；使用前必须阅读 `docs/gpui-component-guide.md`。
- `src/features/*`: 各插件 runtime 和页面。

目标分层：

```text
core/
  command.rs       typed command, score, prefix, action payload
  plugin.rs        PluginRuntime and PluginSession traits
  plugin_spec.rs   manifest visual/window/status/category spec
  storage.rs       AppPaths and data path derivation

app/
  runtime.rs       GPUI app bootstrap, window lifecycle, service registry
  launcher.rs      modern launcher UI, command search, Enter open
  text_input.rs    SingleLineInput today, TextEditor target
  theme.rs         tokens from suishou Theme.qml
  ui.rs            stable shared components only

platform/
  macos.rs         app open, permissions, clipboard, file dialog, proxy
  mod.rs           platform traits and feature gates

features/<plugin>/
  manifest.rs      static manifest and command metadata
  service.rs       pure business logic and async jobs
  store.rs         SQLite/file persistence, migrations, paging
  view.rs          GPUI view/session state and render helpers
  mod.rs           module exports
```

当前部分插件仍集中在 `plugin.rs`。后续迁移时不要继续堆大文件；改动插件时优先拆成 `manifest.rs`、`service.rs`、`store.rs`、`view.rs`。

## Rust-Native Architecture Rules

迁移目标不是“把 Python 改写成 Rust 语法”，而是重建适合 Rust 的边界：

- `Runtime` 是长生命周期对象，持有 `Arc<Service>`、cache、background handle 和动态命令注册状态。
- `Session` 是窗口生命周期对象，只持有 GPUI entity、当前选中项、轻量 UI state。窗口关闭时释放大列表、编辑器 buffer、预览图。
- `Service` 只做业务逻辑和后台任务编排，不引用 GPUI 类型。返回不可变 DTO。
- `Store` 只做持久化、migration、分页查询。不要把 UI 状态写进数据库。
- `View` 只渲染 `Vm`，只发出 `Action`。不要在 render 中读文件、扫目录、访问网络、执行命令。
- 后台任务统一通过 `cx.spawn`、background executor、channel 或 service state 回主线程。不要在锁里执行长任务。
- 跨线程共享用 `Arc`、`Mutex/RwLock`、channel；窗口内部临时 UI 状态才允许 `Rc<RefCell<_>>`。
- `gpui-component` 只能用于 UI 层。`service.rs`、`store.rs` 和平台层不得依赖 `gpui-component`。Sheet/Dialog/Notification 等 Root 相关 API 只能在已明确迁成 `gpui_component::Root` 的窗口中使用。
- 错误用 `anyhow::Result` 或插件自定义 error enum；UI 层把错误转为 `StatusVm`。
- 大列表必须分页或虚拟化。render 中不得 clone 全量数据库记录。
- 每个插件先做 domain/service/store 单测，再接 GPUI。

推荐数据流：

```text
User input
  -> ViewAction
  -> Session updates local UI state or calls Runtime service
  -> Service validates and runs sync/async work
  -> Store persists or pages data
  -> Service returns DTO/Page<RowVm>/StatusVm
  -> Session stores VM
  -> GPUI render consumes VM only
```

低级模型禁止跨层捷径：

- 不要让 `view.rs` 直接打开 SQLite。
- 不要让 `store.rs` 引用 GPUI。
- 不要让 `service.rs` 持有 `Entity<TextInput>`。
- 不要把平台命令散落在插件 UI 中；放到 `platform` trait 或插件 service。

## Status Definitions

矩阵中的状态只允许使用这些值：

- `Stub`: 只有入口或占位说明，没有真实功能。
- `UI only`: 有页面结构，但输入、按钮或数据不完整。
- `Functional v1`: 核心业务路径真实可用，可单测，但未完整复刻 suishou。
- `Feature parity`: suishou 的主要功能完整迁移。
- `Pixel parity`: 功能完整，视觉尺寸、间距、颜色、状态、交互和 suishou QML 基本一致。

不要因为页面能打开就标成 `Functional v1`。必须有真实输入、真实执行路径和错误处理。

## Plugin Status Matrix

| Plugin | Reference | Qingqi path | Current status | Next milestone |
| --- | --- | --- | --- | --- |
| `about` | `about/AboutPage.qml`, `about/runtime.py` | `src/features/about` | `Functional v1` | 对齐 suishou 关于页尺寸、应用名、技术栈展示，补 snapshot/manual QA |
| `app-launcher` | `app_launcher/plugin.json`, `app_launcher/runtime.py` | `src/features/app_launcher` | `Functional v1` | 更多像素对齐细节与虚拟列表 |
| `clipboard` | `clipboard/ClipboardWindowPage.qml`, `runtime.py`, `view_model.py` | `src/features/clipboard` | `Functional v1` | 文件记录详情区像素对齐、设置页更细的视觉对齐、history_store.rs 收口为 store.rs |
| `json-parser` | `json_parser/JsonParserPage.qml`, `service.py`, `view_model.py` | `src/features/json_parser` | `Functional v1` | 真正 `TextEditor`、拆分 manifest/view、窄窗口响应式、状态栏像素对齐 |
| `qr-code` | `qr_code/QrCodePage.qml`, `service.py`, `view_model.py` | `src/features/qr_code` | `Functional v1` | 真正多行编辑器、header/status 细节像素对齐 |
| `quick-launch` | `quick_launch/*.py`, `QuickLaunchWindowPage.qml` | `src/features/quick_launch` | `Functional v1` | 继续做列表 / toolbar / editor sheet / 历史与结果弹层的像素对齐，补更多 QML 细节收口 |
| `system-settings` | `system_settings/SystemSettingsPage.qml`, `view_model.py` | `src/features/system_settings` | `Functional v1` | 系统跟随（macOS 外观变化监听）、插件导入目录/ZIP、图标缓存清理 |
| `gpui-demo` | replaces `qml_demo` | `src/features/gpui_demo` | `UI only` | 增加真实 GPUI 控件实验：输入、列表、弹层、tabs、编辑器 |
| `image-compress` | `image_compress/ImageCompressPage.qml`, `service.py`, `view_model.py` | `src/features/image_compress` | `Functional v1` | 后台压缩（JobProvider）、真正的 SaveAs 对话框、像素对齐 |
| `download-manager` | `download_manager/*.py`, `DownloadManagerPage.qml` | `src/features/download_manager` | `Functional v1` | 设置 UI 控件、分类筛选、像素对齐 |
| `api-debugger` | `api_debugger/*.py`, `ApiDebuggerPage.qml` | `src/features/api_debugger` | `Functional v1` | 集合/环境 CRUD UI、请求 tabs 切换、WebSocket、OpenAPI/Mock |
| `http-capture` | `http_capture/*.py`, `HttpCapturePage.qml` | `src/features/http_capture` | `Functional v1` | 真实代理引擎接入、HTTPS 证书生成/信任、系统代理接管、重放/导出 |
| `ftp-sftp-ssh-client` | `ftp_sftp_ssh_client/*.py`, `FtpSftpSshClientPage.qml` | `src/features/ftp_sftp_ssh_client` | `Functional v1` | FTPS 支持、SSH terminal 桥接、传输队列 UI 细化、像素对齐 |

## Global Work Protocol

每次只迁移一个插件或一个共享基础设施能力。不要在同一任务中同时重写多个复杂插件。

标准步骤：

1. 读取 suishou manifest。
2. 读取 suishou QML 主页面，记录尺寸、布局、控件、状态、弹窗。
3. 读取 suishou Python service/view_model/repository，记录业务动作和数据模型。
4. 在本文档该插件区域更新迁移笔记。
5. 建 Rust 类型：`State`、`Action`、`RowVm`、`DetailVm`、`Service`、`Store`。
6. 先迁业务 service/store，并写单元测试。
7. 再接 GPUI view，保持 UI state 和 service DTO 边界清晰。
8. 更新 manifest、commands、dynamic actions。
9. 跑 `cargo fmt`、对应单测、`cargo check`。
10. 更新矩阵状态和剩余项。

禁止事项：

- 禁止把 Python/QML 的大段状态直接翻译到 `render`。
- 禁止在 `render` 里扫描文件、打开网络、读写 SQLite。
- 禁止把 `Rc<RefCell<_>>` 放进后台 service/store。
- 禁止让动态命令依赖“用户先打开过插件”才注册，除非矩阵里明确标注为临时问题。
- 禁止新增 warning。已有 warning 可以分批清理，但新代码应保持干净。

## Low-Level Model Execution Protocol

低级模型每次只接一个“批次任务”。批次任务必须小到可以在一次改动中完成、编译、测试、记录。

每次开始前必须读取：

1. 本文档对应插件任务卡。
2. suishou `plugin.json`。
3. suishou 主 QML。
4. suishou 对应 Python service/view_model/repository。
5. Qingqi 当前插件目录。

每次输出必须包含：

- 改了哪些 Rust 文件。
- 新增了哪些类型。
- 迁移了哪些 suishou 功能。
- 哪些 UI 区块完成。
- 哪些测试跑过。
- warning 数量是否增加。
- 文档矩阵是否更新。

低级模型必须按这个顺序做：

1. 建类型，不写 UI。
2. 写 service/store 测试。
3. 实现 service/store。
4. 跑对应测试。
5. 接 GPUI view。
6. 跑 `cargo fmt`。
7. 跑 `cargo test` 或插件相关测试。
8. 跑 `cargo check`。
9. 更新文档任务状态。

低级模型不允许：

- 一次迁移多个插件。
- 先画大 UI 再补 service。
- 把“暂不支持”做成静默失败。必须显示错误或状态。
- 删除现有可用功能。
- 为了消除 warning 删除未来明确会用的架构接口，除非文档同步说明。

任务粒度示例：

- 好任务：`json-parser: 添加复制输出和清空按钮`。
- 好任务：`quick-launch: 实现参数提取 service 和单测`。
- 好任务：`download-manager: 建 store schema 和 repository 单测`。
- 坏任务：`完整迁移 api-debugger`。
- 坏任务：`重写所有 UI`。
- 坏任务：`顺手清理全局架构`。

## Pixel Recreation Workflow

像素级复刻以 suishou QML 为视觉基准，但不要照搬 QML 控件树。流程如下：

1. 打开参考 QML。
2. 抽取页面根布局：
   - `anchors.margins`
   - `spacing`
   - header 高度
   - 左右栏宽度
   - row height
   - card radius
   - border width
   - font size
3. 抽取状态：
   - empty
   - loading
   - selected
   - hover
   - error
   - disabled
   - running
4. 抽取动作：
   - button onClicked
   - list selection
   - popup open/close
   - context menu
   - keyboard shortcut
5. 在 Rust 中建立 `Vm`：
   - 列表页使用 `Vec<RowVm>`。
   - 详情页使用 `DetailVm`。
   - status bar 使用 `StatusVm`。
   - 表单使用显式 `FormState`。
6. GPUI 只消费 VM，不直接碰数据库或后台任务。
7. 对每个状态做 manual QA。

常用提取命令：

```bash
cd /Users/fwfx1234/develop/qingqi
rg -n "anchors\\.margins|spacing:|radius:|height:|width:|font\\.pixelSize|text:" \
  /Users/fwfx1234/develop/suishou/src/features/<plugin>/*.qml

rg -n "class |def |@Slot|Signal\\(|Property\\(" \
  /Users/fwfx1234/develop/suishou/src/features/<plugin> --glob '*.py'
```

视觉默认值：

- 页面 margin 优先使用 suishou 的 `Theme.space["3"]`，当前等价约 `12px`。
- 卡片 radius 优先 `8px` 或 suishou `Theme.radii.md`。
- 插件标题 `20px`，局部标题 `13px` 到 `15px`。
- 状态栏高度 `28px` 到 `30px`。
- 列表 row height 必须固定，不允许内容导致高度跳动。
- 重插件优先三栏或上下 split，不要卡片套卡片。

## Shared Infrastructure Tasks

这些任务应优先于重插件迁移。

### TextEditor

当前 `TextInput` 只有部分多行能力。目标新增 `TextEditor`：

- 多行输入、换行、paste 保留换行。
- 光标移动、选择、复制、剪切、全选。
- 垂直滚动，长行横向滚动或软换行二选一。
- monospace 模式。
- read-only 模式。
- 错误行高亮接口，供 JSON/API 响应使用。
- 单元测试覆盖输入、选择、paste、clear、focus。

迁移顺序：

1. 抽出 `SingleLineInput`，保留启动器和短输入使用。
2. 新建 `TextEditor`，先支持纯文本多行。
3. 替换 `json-parser` 输入/输出。
4. 替换 `api-debugger` 请求体/响应体。
5. 替换 `quick-launch` 脚本编辑框。

Current Notes:

- 共享 `TextInput` 的 multiline 模式现在已经能按显式换行真正分多行渲染，不再把包含 `\n` 的内容挤成单行；`json-parser` 输入/输出区与 `quick-launch` 的 `env` 多行框会直接受益。
- multiline shell 现在已改成顶部对齐，并提供纵向滚动；放进固定高度 pane 后不会再把多行文本垂直居中挤在中间，更接近真正编辑器表面。
- 已补 `monospace` 模式，JSON 输入/输出和 quick launch 的多行 `env`、内联脚本输入已经切到更接近 suishou 的等宽编辑观感；后续 API debugger、下载/响应体等插件可以直接复用这一模式。
- 当前实现仍是“显式换行多行渲染”，还没有做完整 `TextEditor` 级别的软换行、横向滚动策略、行号、错误行高亮和更细的命中/选择优化。
- 已补纯 helper 测试，覆盖多行 range 拆分、空行保留、offset 到 line 映射和按行 run 切片；这让后续低级模型继续演进 shared editor 时有稳定护栏。

### Service Registry

目标新增 `AppContextServices`：

```rust
pub struct AppContextServices {
    pub paths: AppPaths,
    pub theme: Arc<ThemeStore>,
    pub clipboard: Arc<Mutex<ClipboardService>>,
    pub app_index: Arc<AppIndexService>,
    pub quick_launch: Arc<QuickLaunchRegistry>,
}
```

规则：

- runtime 持有 service 的 `Arc`。
- session 只持有轻量 clone 或 UI entity。
- 后台任务通过 channel 或 service state 通知主线程刷新。
- 所有锁只包围短操作，不包围网络/文件扫描/压缩。

### ThemeStore (已实现)

```text
src/app/theme_store.rs  ThemeMode, ThemeStore with JSON persistence
src/core/storage.rs     AppPaths::config() added
```

当前状态：
- `ThemeMode::Light | Dark | System` enum，支持 serde 序列化。
- `ThemeStore` 初始化时从 `AppPaths::config("theme.json")` 加载，自动持久化。
- 设置页已接入三档切换：浅色、深色、跟随系统。
- 全局 `DARK_MODE` AtomicBool 作为即时缓存，ThemeStore 更新后立即生效。

待办：
- 系统跟随：监听 macOS 外观变化，只在 `ThemeMode::System` 时切换。
- 所有窗口订阅刷新：后续应按窗口/entity 定向通知，避免 app-wide repaint。

### Virtual List

大列表必须虚拟化或分页：

- Clipboard history。
- App index。
- Download tasks。
- HTTP capture sessions。
- FTP transfer queue。
- API collection tree。

先实现简单分页也可以：service 返回 `Page<RowVm> { rows, total, offset, limit }`。

## Rust Plugin Template

目标结构：

```text
src/features/<plugin>/
  mod.rs
  manifest.rs
  service.rs
  store.rs
  view.rs
```

`manifest.rs`：

```rust
use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec, PluginWindowMode, WindowSpec},
};

pub const PLUGIN_ID: &str = "<plugin-id>";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "...",
        description: "...",
        keywords: &["..."],
        background: false,
        visual: PluginVisualSpec {
            icon: "...",
            accent: PluginAccent::Blue,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.8, 0.8),
        },
        stats: PluginStats {
            primary: "...",
            secondary: "...",
            tertiary: "...",
        },
        command_hint: "...",
        command_prefixes: &["..."],
    }
}
```

`service.rs`：

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub input: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub output: String,
    pub message: String,
}

pub struct Service;

impl Service {
    pub fn run(&self, request: Request) -> anyhow::Result<Response> {
        // pure business logic here
        Ok(Response {
            output: request.input,
            message: "ok".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_basic_case() {
        let service = Service;
        let response = service.run(Request { input: "x".into() }).unwrap();
        assert_eq!(response.output, "x");
    }
}
```

`store.rs`：

```rust
use std::path::PathBuf;

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn ensure_schema(&self) -> anyhow::Result<()> {
        // explicit schema version and migrations
        Ok(())
    }
}
```

`view.rs`：

```rust
#[derive(Clone, Debug)]
pub struct RowVm {
    pub id: String,
    pub title: String,
    pub subtitle: String,
}

#[derive(Clone, Debug)]
pub struct DetailVm {
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub enum ViewAction {
    Refresh,
    Select(String),
    Execute,
}
```

## Command System Rules

每个插件至少提供 open command：

- target `CommandTarget::PluginOpen { plugin_id }`
- prefixes 来自 suishou `plugin.json`
- icon 使用 Qingqi assets 路径

动态动作使用：

- target `CommandTarget::PluginAction { plugin_id, action_id, payload }`
- `payload` 只放可序列化轻量数据，如 id、path、url。
- 大对象放 store，通过 id 查询。

示例：

```rust
CommandItem::plugin_action(
    "quick-launch",
    format!("run-{id}"),
    action.name.clone(),
    action.subtitle(),
    action.keywords(),
    ["ql", "quick"],
    "qta/fa5s.bolt.png",
    Some(id.to_string()),
)
```

`handle_command()` 必须：

- 校验 plugin id 和 action id。
- 返回 `CommandOutcome { message }`。
- 对长任务只启动后台任务，不阻塞 UI。
- 执行失败时返回清晰 message。

## Per-Plugin Execution Cards

低级模型执行任意插件迁移时，只能按本节任务卡推进。每个插件都分为功能规划、UI 复刻规划、Rust 模块规划、执行批次、验收测试。未完成的批次不要跳过，不要把后续批次的 UI 先堆出来。

### about

Reference:

- `/Users/fwfx1234/develop/suishou/src/features/about/plugin.json`
- `/Users/fwfx1234/develop/suishou/src/features/about/AboutPage.qml`
- `/Users/fwfx1234/develop/suishou/src/features/about/runtime.py`

Current Qingqi:

- `src/features/about/plugin.rs`
- `src/features/about/manifest.rs` (batch 1 done)
- `src/features/about/mod.rs`

Functional Plan:

- 展示 Qingqi 应用名、版本、说明、技术栈、数据目录入口。
- 保留 suishou 关于页的“产品身份 + 版本信息 + 简短说明”功能，不保留 PySide6 文案。
- 版本必须来自 `env!("CARGO_PKG_VERSION")`。
- 不需要 store，不需要后台任务。

UI Plan:

- 固定居中信息布局：icon、标题、版本、副标题。
- 对齐 suishou AboutPage 的 72px icon、标题居中、说明条结构。
- Qingqi 可以使用更现代的卡片，但不要卡片套卡片。
- 窗口比例保持轻量，首屏不可滚动。

Rust Plan:

- `manifest.rs`: manifest、window spec、icon、keywords。
- `view.rs`: `AboutPage`、section row、tech row。
- `plugin.rs`: thin runtime/session，只组合 manifest 和 view。

Execution Batches:

1. 拆 `manifest.rs`，保持 command 行为不变。 ✅ 已完成
2. 拆 `view.rs`，不改变视觉。 ✅ 已完成
3. 对齐 QML 尺寸和间距：72px icon、居中布局、描述卡片。 ✅ 已完成
4. 补 manifest 单测。 ✅ 已完成 — 7 tests，覆盖 id、name、accent、category、prefixes、background

Acceptance:

- `about.open` 能从启动器打开。
- 页面显示 Qingqi、版本号、Rust + GPUI。
- `cargo check` 通过，无新增 warning。

### json-parser

Reference:

- `json_parser/plugin.json`
- `json_parser/JsonParserPage.qml`
- `json_parser/service.py`
- `json_parser/view_model.py`

Current Qingqi:

- `src/features/json_parser/plugin.rs`
- `src/features/json_parser/manifest.rs`
- `src/features/json_parser/service.rs`
- `src/features/json_parser/view.rs`

Functional Plan:

- JSON 格式化、压缩、验证、查询。
- 查询支持 `.a.b`、`$.a.b`、`/a/b`。
- 输出统计：字符数、行数、对象/数组摘要、错误位置。
- 支持复制输出、从剪贴板填充、清空。
- 输入错误时保留原文，输出错误摘要，状态栏显示错误位置。

UI Plan:

- 顶部 toolbar：格式化、压缩、执行查询、复制输出、从剪贴板填充、清空。
- JSONPath 行在输入/输出上方或 toolbar 下方，短输入使用 `SingleLineInput`。
- 主区域左右双栏：左输入、右输出。窄窗口可上下堆叠。
- 输入/输出都用 monospace `TextEditor`；输出 read-only。
- 底部 status bar 显示 status、errorLoc、stats。
- QML 对齐点：root margin `Theme.space["3"]`，toolbar gap `Theme.space["2"]`，label font `Theme.fontSize.caption`。

Rust Plan:

- `manifest.rs`: id `json-parser`、prefix `json/jq`、window ratio `0.82/0.84`。
- `service.rs`: `JsonMode`、`JsonRequest`、`JsonResult`、`JsonStats`、query parser。
- `view.rs`: `JsonPanelState`、`JsonAction`、`JsonVm`、toolbar、editor panes。
- `plugin.rs`: runtime/session glue。

Execution Batches:

1. 完成 `TextEditor` 基础能力并替换当前伪多行输入。
2. 扩展 service 输出 `JsonStats` 和 error location。 ✅ 已完成
3. 增加复制输出、从剪贴板填充、清空按钮。 ✅ 已完成
4. 拆分 manifest/service/view。 ✅ 已完成（runtime/session 只保留胶水层，页面状态与 render helper 已移到 `view.rs`）
5. 对齐 QML 双栏 UI 和状态栏。 ✅ 已完成（当前仍使用增强 `TextInput`，尚未替换为真正 `TextEditor`）
6. 输出区切成只读多行 pane，并收掉 editor pane 的内层双重边框。 ✅ 已完成
7. toolbar 改为可换行，窄窗口下输入/输出区自动上下堆叠；状态栏空字段不占位。 ✅ 已完成

Acceptance:

- 输入合法 JSON 后格式化/压缩结果正确。
- 输入非法 JSON 不崩溃，有错误状态。
- 查询 `.foo.bar`、`$.foo.bar`、`/foo/bar` 均可用。
- service 单测覆盖格式化、压缩、错误、查询。

Current Notes:

- 当前 GPUI 页面已经对齐 suishou `Flow + SplitView` 的主要响应式行为：toolbar 可换行，窗口宽度小于 `860px` 时输入/输出区会自动从双栏切换为上下堆叠，并给 pane 保留最小高度。
- `json-parser` 的输出标题已按 `格式化 / 压缩 / 验证 / 查询结果` 跟随模式变化；底部状态栏现在只在 `errorLoc` 或 `stats` 非空时渲染对应字段，避免空占位把间距撑乱。
- `json-parser` 现在已经按 Rust-native 结构拆成 `manifest.rs + service.rs + view.rs + plugin.rs`；`plugin.rs` 只负责 runtime/session glue，后续低级模型可以更安全地单独修改 manifest 或页面状态，而不用继续堆大文件。
- 当前仍是增强版 `TextInput`，但 multiline 模式已经具备真实分行渲染、顶部对齐、纵向滚动和等宽字体；下一批优先项改为继续补软换行/横向滚动策略、更完整的 shared `TextEditor` 能力，并让 `JSONPath` 行和输入 pane 的交互进一步贴近 suishou。

### qr-code

Reference:

- `qr_code/plugin.json`
- `qr_code/QrCodePage.qml`
- `qr_code/service.py`
- `qr_code/view_model.py`

Current Qingqi:

- `src/features/qr_code/plugin.rs`
- `src/features/qr_code/manifest.rs`
- `src/features/qr_code/service.rs`
- `src/features/qr_code/store.rs`
- `src/features/qr_code/view.rs`

Functional Plan:

- 文本/URL 生成二维码。
- 从剪贴板填充。
- 复制原始内容。
- 保存二维码图片。
- 导出二维码历史，打开保存目录。
- 扫描二维码图片，第一阶段允许只完成文件选择和错误状态，第二阶段接识别 crate。
- 历史记录：保存 text、created_at、matrix size、saved path。

UI Plan:

- Header 左侧标题，右侧 scan/history icon buttons。
- 内容输入区 label “内容”，输入框固定高度。
- 预览卡固定正方形，空状态显示“二维码预览”。
- 底部按钮：保存图片、复制内容、从剪贴板、清空。
- 历史弹层使用遮罩 + 居中 popup sheet，保留过滤、导出、清空、删除、回填、复制。
- 扫描弹层使用遮罩 + 居中 popup sheet，保留选择图片、路径输入、结果区、复制结果、用作生成内容。
- QML 对齐点：预览 sourceSize 约 `360x360`，预览卡 radius `Theme.radii.lg`，status bar 约 28px。

Rust Plan:

- `manifest.rs`: id `qr-code`、prefix `qr/qrcode`。
- `service.rs`: `QrCodeService`、`generate(text) -> QrMatrix`、`save_png(matrix, path)`、`scan_image(path)`、history export/save root。
- `store.rs`: `QrHistoryStore`、insert/list/search/delete/clear/export。
- `view.rs`: `QrPanel`、preview state、history popup state、scan popup state、status bar action。

Execution Batches:

1. 拆 manifest/service/view，不改变现有生成功能。 ✅ 已完成
2. 增加 `QrHistoryStore` 和单测。 ✅ 已完成
3. 增加保存 PNG service 和按钮。 ✅ 已完成
4. 增加复制内容、从剪贴板、清空。 ✅ 已完成
5. 增加历史弹窗。 ✅ 已完成（已切到遮罩 + 居中 overlay sheet，剩余 header/detail 像素细调）
6. 增加扫描弹窗和扫描 service。 ✅ 已完成（已切到 popup 式扫描面板 + 文件选择 + `quircs` 解码；本轮补齐 `file://`/引号路径规范化和 GPUI `ExternalPaths` 拖拽导入）

Current Notes:

- QR 生成、保存、复制、历史、导出、扫码识别均已是 Rust service/store 真实路径。
- 扫描路径现在会先做 suishou 等价的本地路径规范化：支持 `file://`、`file://localhost/...`、URL percent decode、引号包裹路径和 `~/`。
- 扫描弹层的路径输入区已接 GPUI 外部文件拖放，拖入图片后会直接扫描并同步规范化后的路径。
- 剩余主要是视觉细节与更完整的编辑器体验，不再是功能缺口。

Acceptance:

- 空文本拒绝，错误显示在状态栏。
- 生成矩阵尺寸大于 0。
- 保存图片在临时目录单测通过。
- 历史记录可插入、过滤、删除、清空、导出。
- 扫描 PNG/JPG/BMP/WebP 可解码并写入历史。

### app-launcher

Reference:

- `app_launcher/plugin.json`
- `app_launcher/runtime.py`

Current Qingqi:

- `src/platform/apps.rs`
- `src/features/app_launcher/manifest.rs`
- `src/features/app_launcher/plugin.rs`
- `src/features/app_launcher/service.rs`
- `src/features/app_launcher/store.rs`

Functional Plan:

- 扫描 macOS app，生成应用索引。
- 搜索名称、bundle id、路径。
- 打开选中应用。
- 动态 command 注册每个常用应用。
- 后台刷新索引，缓存结果，启动后不依赖用户先打开插件。
- 不兼容 suishou runtime 的 list session，只保留“搜索和启动应用”功能。

UI Plan:

- Header：标题、总应用数、扫描状态。
- Search row：真实输入、实时过滤、重置。
- List：固定 row height 56px，icon letter/app icon、名称、路径或 bundle id。
- Empty state：无应用或无匹配。
- Loading state：扫描中。
- Status bar：索引数量、上次扫描时间、错误。
- 交互：Enter 打开选中项，上下键移动，点击只选择，双击或按钮打开。

Rust Plan:

- `manifest.rs`: id `app-launcher`、mode `List`、prefix `app/open`。
- `service.rs`: `AppIndexService`、scan、filter、open。
- `store.rs`: JSON cache，路径来自 `AppPaths::feature_state("app-launcher", "index.json")`。
- `platform/apps.rs`: open app、read bundle id、scan app dirs。
- `view.rs`: `AppRowVm`、`AppIndexVm`、selection state。

Execution Batches:

1. 把同步扫描搬进 `AppIndexService`，保留当前 UI。 ✅ 已完成
2. 增加 cache，启动时先读 cache，再后台刷新。 ✅ 已完成
3. dynamic commands 从 service/cache 读取。 ✅ 已完成
4. 插件窗口改为输入实时过滤和键盘选择。 ✅ 已完成（entity view + 输入订阅 + 上下键选择 + Enter 打开）
5. 移除 UI 层 `PlistBuddy` 调用，改 platform/service。 ✅ 已完成（平台命令已收口到 `src/platform/apps.rs`）
6. 补 suishou list 交互与状态栏细节。 ✅ 已完成 v1（点击只选择、双击打开、底部 status bar 指标）
7. 增加分页或虚拟列表。 ✅ 已完成 v1（service `Page<T>` + 40 条分页窗口 + 上/下页控制 + 页码状态）
8. 图标缓存容错与别名覆盖。 ✅ 已完成（`validate_cached_icon` 零字节/损坏文件清理 + `clear_broken_icon_paths` 加载后/刷新后清理 + `camel_case_split` + `normalize_search_text` 别名扩展）

Acceptance:

- 首次打开能看到应用列表。
- 搜索 app 名称和 bundle id 有结果。
- 启动器可搜索动态 app command。
- 扫描不阻塞 UI。
- cache 序列化单测通过。

Current Notes:

- 插件窗口现在已经对齐 suishou 里“点击只选择，双击或按钮打开”的核心交互；列表行仍保持固定高度，不会因为 subtitle 长短抖动。
- 标题区已收成更简洁的说明文案，运行状态、索引数量、匹配数、最近扫描时间已下沉到独立底部 status bar，页面结构更接近 suishou 的 list 工具页。
- `app-launcher` 现已支持真实 app icon：平台扫描阶段会解析 `Info.plist`、查找 bundle icon，并将可解码图标转成缓存 PNG；列表行优先显示真实图标，失败时回退字母 tile。
- 搜索链路已经补上别名和规范化匹配：`VS Code` / `vscode` / bundle id 末段都能命中，动态 app command 也会复用这些 aliases，和 suishou 启动器行为更接近。
- 列表现在先走 Rust-native 分页而不是一次性渲染全量命中项：service 返回 `Page<AppEntry>`，窗口固定每页 40 条，页内保留键盘选择、双击打开、按钮打开和底部页码状态；这给后续 `clipboard`、下载类插件提供了可复用模板。
- 扫描流程已切成 metadata-first 的两阶段刷新：后台线程会先发布无图标的应用元数据结果，让列表和搜索尽快可用，再异步补 icon 并落缓存；status bar 会区分 `refreshing` 和 `icons` 两个阶段。
- 当前剩余重点收敛为像素对齐和虚拟列表；扫描 / cache / 动态命令 / 后台刷新 / metadata-first 增量刷新 / 双击打开 / 真实图标 / 别名搜索 / 分页展示 / 图标缓存容错这一条真实功能链已经完整。
- 2026-05-28 当前验证结果：`cargo test` 255/255 通过，`cargo check` 通过，warning 维持未新增。
- 图标缓存容错已实现：`validate_cached_icon` 会检测零字节和损坏的缓存文件并自动清理；`clear_broken_icon_paths` 在加载缓存和刷新后清理无效路径，确保 UI 不会收到损坏的图标路径；失败时诚实回退到字母 tile。
- 别名覆盖已扩展：`camel_case_split` 拆分 PascalCase（"VSCode" → "vs code"）；`normalize_search_text` 归一化特殊字符（"com.apple.Safari" → "comapplesafari"）；bundle-id 归一化变体也已加入搜索文本。

### quick-launch

Reference:

- `quick_launch/plugin.json`
- `quick_launch/QuickLaunchWindowPage.qml`
- `quick_launch/repository.py`
- `quick_launch/executor.py`
- `quick_launch/parameters.py`
- `quick_launch/registrar.py`
- `quick_launch/view_model.py`

Current Qingqi:

- `src/features/quick_launch/manifest.rs`
- `src/features/quick_launch/model.rs`
- `src/features/quick_launch/parameters.rs`
- `src/features/quick_launch/plugin.rs`
- `src/features/quick_launch/service.rs`
- `src/features/quick_launch/store.rs`
- `src/features/quick_launch/view.rs`

Functional Plan:

- 管理动作：新建、编辑、复制、删除、启用、停用。
- 动作类型：script、inline script、open path、open url。
- 参数：从 `${name}` 提取，运行前弹出参数 sheet，替换到 path/args/cwd/env/url。
- 执行：后台运行、timeout、stop、capture stdout/stderr、通知反馈。
- 运行历史：记录开始/结束、状态、输出、错误、耗时。
- 动态命令：enabled actions 注册到启动器。
- 当前硬编码系统命令只能作为 seed data，不是最终架构。

UI Plan:

- Header：标题、搜索框、新建动作按钮。
- Action list：row height 64px，kind icon、名称、kind chip、feedback chip、enabled/running 状态。
- Row actions：运行、停止、编辑、更多菜单。
- Empty state：无动作或无匹配。
- Action editor sheet：名称、类型、脚本/URL/路径、cwd、env、timeout、feedback、enabled。
- Parameter sheet：参数名、输入框、运行/取消。
- Result sheet：成功/失败图标、stdout/stderr、复制、关闭。
- Context menu：停止、启用/停用、复制、运行历史、删除。

Rust Plan:

- `manifest.rs`: id `quick-launch`、background true when registry exists、prefix `ql/quick`。
- `model.rs`: `QuickAction`、`ActionKind`、`FeedbackMode`、`ExecutionStatus`、`QuickRun`、`ParameterSpec`。
- `parameters.rs`: extract/substitute/substitute_mapping。
- `store.rs`: actions/runs schema and migrations。
- `executor.rs`: background process execution、stop、timeout。
- `registry.rs`: dynamic command list from enabled actions。
- `view.rs`: list/editor/parameter/result sheet states。

Execution Batches:

1. 实现 `parameters.rs` 和单测。 ✅ 已完成
2. 实现 `store.rs` actions CRUD 和 runs 记录。 ✅ 已完成
3. 把硬编码 command 改成首次空库 seed data。 ✅ 已完成
4. 实现 executor，同步测试用 mock runner。 ✅ 已完成基础执行路径；当前已升级为服务层后台线程执行，并已补 stop/timeout
5. 实现 dynamic commands。 ✅ 已完成
6. 复刻 action list 和搜索。 ✅ 已完成（当前列表已改为读取 store-backed action 仓库）
7. 复刻 editor sheet。 ✅ 已完成 v1（create / edit / duplicate / enable / disable / delete）
8. 复刻 parameter/result/history UI。 ✅ 已完成参数 sheet、结果详情 sheet、历史 sheet；顶部工具条和历史项都可打开结果详情
9. 把执行移出 UI 线程。 ✅ 已完成 v1（后台线程 + runtime revision watcher + running 状态）

Current Notes:

- 已从 runtime 大文件拆出 `view.rs`，`quick-launch` 主窗口现在是 GPUI entity view，不再继续把搜索/选择/弹层状态混在 runtime 内。
- action list 已支持键盘上下选择、Enter 执行、Esc 关闭当前 sheet。
- 参数化动作会先解析 `${name}`，动态命令和插件窗口执行在缺参时都会提示用户打开窗口填写。
- 插件窗口已补参数 sheet、结果详情 sheet 和最近运行历史 sheet；运行历史读取 `quick_launch_runs`，每次执行后会刷新当前动作的历史列表。
- 结果详情 sheet 会在 `popup` feedback 或失败/超时/停止等非成功状态下自动弹出，也支持从顶部“最新结果”和历史项“详情”手动打开，并支持复制 `stdout/stderr`。
- 动作仓库已补 create / edit / duplicate / enable / disable / delete 的完整 service 接口，view 层新增 editor sheet 和选中项管理条，不再需要直接操作 store。
- 启动器窗口现在会在 render 时同步动态命令；配合 `QuickLaunchService.revision` 和 runtime watcher，enabled action、复制、删除、新建后的命令列表会自动刷新。
- `QuickLaunchService` 现在持有 Rust-native runtime state：`running_action_ids`、`revision`、`last_event`、`stopping_action_ids`、`active_pids`。runtime 只负责监听 revision 并刷新窗口，view 只消费快照。
- 动作执行已经搬到后台线程；列表会显示“运行中”，支持从行内和顶部工具条请求停止，动态命令也不再同步阻塞启动器。
- 脚本动作现在在独立进程组中执行，stop/timeout 会向整个进程组发信号，运行历史状态已扩展到 `success / failed / timeout / stopped / error`。
- `QuickAction` / `QuickActionDraft` 已补 `script_type`、`script_source`、`interpreter`、`env` typed 字段；SQLite schema 会在旧库上自动补列，旧脚本动作默认迁成 `inline + shell`，不会把已有 `script_body` 误当文件路径。
- editor sheet 已接通 `脚本类型 / 脚本来源 / 解释器覆盖 / 环境变量`，参数输入改为 shell 风格拆分并支持 quoted args round-trip；`env` 采用每行 `KEY=VALUE` 的 Rust-native map 编辑方式。
- script 执行分发已对齐 suishou 语义到 Rust 侧：inline shell/node/python 与 path script 分别走 typed dispatch，支持解释器覆盖、环境变量替换、cwd、timeout、stop 和结果记录。
- `TextInput` 的 multiline 模式已保留真实换行文本显示，先支撑 quick launch env 输入与后续 shared `TextEditor` 迭代。
- 共享 `TextInput` 的多行模式已补一批真正编辑器行为：`Up/Down`、`Shift+Up/Down`、多行 `Home/End`、粘贴时 `CRLF/CR -> LF` 归一化；这让 JSON、脚本和响应体类输入在真正 `TextEditor` 落地前先具备更合理的键盘语义。
- 共享 `TextInput` 已补 `read_only` 语义，读写能力与选择/复制能力解耦；`json-parser` 输出区已经切到只读多行 pane，可滚动、可选择、可复制，不再只是静态文本展示块。
- quick launch editor 的“内联脚本”目标输入现已切成真正多行、等宽、可滚动输入；脚本来源在“文件 / 内联”之间切换时，输入控件会同步切换 placeholder、尺寸和编辑模式，不再把脚本正文塞在单行框里。
- quick launch editor 现已接通脚本路径 / 目标路径 / 工作目录选择器，复用 `platform::shell` 的 macOS 原生 `osascript` 面板，而不是把平台细节散进 view。
- 行级 `⋯` 上下文菜单现已对齐 suishou 基础交互：运行中动作可停止，其他动作可启用/停用、复制、查看运行历史，并通过二次确认弹层删除；这些操作都直接走 typed service 接口，而不是依赖当前选中行的隐式状态。
- 2026-05-26 当前验证结果：`cargo test` 67/67 通过，`cargo check` 通过，warning 维持 73 个未新增。
- 下一批应该继续做 quick launch 的像素级细节：列表行、toolbar、editor 布局、结果/历史面板与 suishou QML 逐块对齐，并收口更多 hover / spacing / 状态色细节。

Acceptance:

- 可以新建一个 open url 动作并执行。
- 可以新建一个 script 动作，记录 stdout/stderr。
- `${name}` 参数缺失会弹参数 sheet。
- 非成功执行或 popup feedback 会自动打开结果详情 sheet，且可复制 `stdout/stderr`。
- 顶部工具条“最新结果”和历史项“详情”都能打开最近执行记录。
- 可以在插件窗口内新建、编辑、复制、启停和删除动作，并保持动作仓库与动态命令列表同步。
- disabled action 不出现在启动器动态命令里。
- 删除动作后列表和命令同步更新。

### clipboard

Reference:

- `clipboard/plugin.json`
- `clipboard/ClipboardWindowPage.qml`
- `clipboard/runtime.py`
- `clipboard/view_model.py`

Current Qingqi:

- `src/features/clipboard/manifest.rs`
- `src/features/clipboard/plugin.rs`
- `src/features/clipboard/service.rs`
- `src/features/clipboard/history_store.rs`
- `src/features/clipboard/view.rs`

Functional Plan:

- 后台采集剪贴板。
- 去重、置顶、删除、清空未置顶、清空全部。
- 搜索和过滤：全部、置顶、文本、链接、代码、图片、文件。
- 选择记录写回系统剪贴板。
- 设置：记录类型、忽略规则、热键、存储上限、保留时间。
- 第一阶段完整支持文本/链接/代码；第二阶段支持图片/文件。

UI Plan:

- Window fixed `980x640`，always on top。
- Outer margin 18px，spacing 14px。
- Header：标题、状态、清理按钮。
- Tabs：全部、置顶、文本、链接、代码、图片、文件、设置。
- 左侧历史列表：固定 row height，type icon、title、meta、pin 状态。
- 右侧 detail：标题、创建时间、内容预览、复制/置顶/删除。
- Settings page：记录类型 toggle、忽略规则列表、热键、存储限制。
- Empty state：暂无历史、当前筛选无结果。
- Loading/error/status 都在底部 status bar。

Rust Plan:

- `manifest.rs`: id `clipboard`、background true、fixed topmost window。
- `model.rs`: `ClipboardKind`、`ClipboardRecord`、`ClipboardFilter`、`ClipboardSettings`。
- `service.rs`: capture、dedupe、classify、write_back、settings。
- `store.rs`: schema version、records、settings、ignore_rules。
- `view.rs`: `ClipboardPanelState`、`HistoryRowVm`、`DetailVm`、`SettingsVm`。
- `platform/macos.rs`: read/write clipboard、hotkey later。

Execution Batches:

1. 给现有 store 增加 schema version 和 migration 单测。
2. 背景 capture 防重复启动。
3. 搜索升级为输入实时过滤。 ✅ 已完成（`observe(query_input)` 实时刷新）
4. 增加 filter tabs 和 `ClipboardFilter`。 ✅ 已完成；当前已支持全部、置顶、文本、链接、代码、图片、文件。
5. ~完整文本/链接/代码分类。~ ✅ `classify_text()`、`text_badges()`、`text_stats()` 分类函数；`badge` 列 schema + migration；ClipboardFilter 增加 Link/Code 筛选；子标题显示 badge；详情面板显示文本统计。
6. 复刻右侧详情区。
7. 补状态栏、空状态和更合理的分页反馈。 ✅ 已完成 v1（filter label + 结果数/关键词状态 + empty state + “已加载全部记录”反馈）
8. 增加 settings page。 ✅ 已完成 v1（历史/设置主 tab + 文本/图片/文件采集开关 + 文本长度上限）
9. 增加 ignore rules。 ✅ 已完成 v1（SQLite 持久化 + 采集时过滤 + 设置页真实输入/保存/清空）
10. 分阶段增加图片/文件。✅ 图片采集已完成；✅ 文件列表采集、存储、详情渲染、写回均已真实接入。

Current Notes:

- `clipboard` 当前仍是较早期的 `Rc<RefCell>` + `RenderOnce` 面板实现，但文本历史、分类筛选、写回剪贴板、置顶、删除、清理和分页读取已经是真实 service/store 路径。
- 这轮补上了更像工具页的反馈层：左侧列表在无数据和无匹配时会显示明确 empty state，底部状态栏会显示当前 filter、加载条数、关键词命中情况，而不是只有零散操作提示。
- “加载更多” 已根据分页状态区分为继续加载、已加载全部记录或暂无更多内容，避免空列表仍然显示可点击按钮。
- `clipboard_config` 现已落到 SQLite：文本采集开关和文本长度上限会真实持久化，打开设置 tab 后即可修改，不再只是内存里的临时 `ClipboardConfig`。
- 这轮把 suishou 的 ignore rules 也接成真实功能：`clipboard_config` 新增 `capture_image`、`capture_files`、`ignore_patterns_json`、`hotkey` 字段，文本采集会先跑 regex（失败回退为大小写不敏感子串）再决定是否落库；设置页已提供规则多行输入、长度上限数字输入和快捷键格式化保存。
- 这轮把主面板从早期 `Rc<RefCell<_>> + RenderOnce` 收成了 GPUI `Entity<ClipboardPanel>`：搜索框通过 `observe(query_input)` 直接驱动刷新，筛选、详情和设置按钮统一走 `cx.update_entity(...)`，后续拆分 `view.rs` 会轻很多。
- 这轮已把 `clipboard` 物理拆成更接近 Rust 目标结构的 `manifest.rs + plugin.rs + service.rs + history_store.rs + view.rs`：runtime/session glue 不再和主视图、manifest 混在一个文件里，后续继续把 `history_store.rs` 收口为 `store.rs` 即可。
- 这轮还把 suishou 的 `pinned` 过滤补回 Rust 版：顶部筛选标签现在包含”置顶”，SQLite 查询会走 `pinned = 1` 条件，store 单测已覆盖 pinned-only 搜索。
- 这轮把设置页里的图片/文件采集开关也接成了真实路径：按钮会通过 `ClipboardService` 的 typed config mutator 落库并刷新内存缓存，不再由 view 手工改整份 config 后回写；状态栏设置摘要也会显示文本/图片/文件三类采集状态。
- 这轮把 GPUI clipboard image entry 接成真实采集：图片按内容 id 去重后落到 `features/clipboard/images`，SQLite 记录保存路径/格式/大小摘要，详情区渲染真实图片，写回时恢复为图片剪贴板内容。
- **文件列表采集现已真实接入**：`platform/clipboard.rs` 新增 `read_file_list()` / `write_file_list()` / `text_looks_like_file_paths()`，macOS 下通过 osascript 检测 `file URL` pasteboard 类型并提取 POSIX 路径；`history_store.rs` 新增 `add_files()` 以 JSON 数组存储路径、`file_list_preview()` 生成预览、`parse_file_paths()` 解析；`service.rs` 的 `capture_current()` 按 文件优先 → 文本 → 图片 顺序采集，文件去重用 `files_signature`；`view/history.rs` 新增文件详情渲染（badge + 可滚动路径列表）；`copy_record_to_clipboard()` 对文件记录优先走原生 `write_file_list`，失败时诚实降级为文本写回。
- 文件存储格式：SQLite `content` 列存 JSON 数组 `[“/path/a”,”/path/b”]`，`preview` 列存 `”文件名1, 文件名2 · N 个”`，`badge` 列存 `”文件”`。
- 平台层诚实性：macOS 下 osascript 读写文件 URL 是真实能力；非 macOS 平台 `read_file_list()` 返回 `None`，`write_file_list()` 返回 error，service 会降级为文本路径检测。
- 本轮新增 7 个测试：`history_store` 6 个（file_list_preview_shows_names_and_count、parse_file_paths_roundtrips_json、parse_file_paths_handles_invalid_input、add_files_stores_and_searches、add_files_respects_capture_settings、add_files_deduplicates_same_paths）+ `service` 1 个（files_signature_deduplicates_same_paths）。
- 下一批应继续做文件详情区像素对齐和把 `history_store.rs` 正式重命名/收口到 `store.rs`。

Acceptance:

- 复制新文本后能被后台采集。
- 复制图片后能落盘并在历史详情区预览，选择图片记录可写回系统剪贴板。
- 从 Finder 复制文件后能被 osascript 采集为 `Files` 记录，详情区显示文件路径列表。
- 选择文件记录可写回系统剪贴板（原生文件 URL 或诚实文本降级）。
- 搜索和 tab filter 正确。
- 选择记录可写回系统剪贴板。
- 删除/置顶/清空持久化。
- store 分页单测通过。
- 文件采集单测通过（17 clipboard tests）。

### system-settings

Reference:

- `system_settings/plugin.json`
- `system_settings/SystemSettingsPage.qml`
- `system_settings/view_model.py`

Current Qingqi:

- `src/features/system_settings/mod.rs`
- `src/features/system_settings/plugin.rs`
- `src/features/system_settings/view.rs`
- `src/features/system_settings/settings_store.rs`

Current Status:

- `system-settings` 保持 `Functional v1`。
- 主题模式切换继续通过 `ThemeStore` 工作，并保留 Light / Dark / System 三态。
- 新增 `SettingsStore`，把内联插件窗口保留时间持久化到 `AppPaths::config(“system_settings.json”)`。
- 页面现在展示真实的应用索引状态，并能调用共享 `AppIndexService` 触发重扫描。
- 开发诊断区显示从 `AppPaths::data_dir()` 直接读取的 data/config/logs 真实路径，不再使用占位字符串或间接推导。
- **辅助功能权限**现在通过 `platform::macos::check_accessibility()` 读取真实 macOS `AXIsProcessTrusted` 状态，显示”已授权”/”未授权”/”未知”。
- **打开系统设置**按钮会打开 macOS 隐私 > 辅助功能面板，并在操作后重新读取权限状态。
- **诊断目录**（数据/配置/日志）现在有”打开”按钮，调用 `platform::shell::open_directory` 并显示成功/失败通知。
- 剪贴板访问、文件访问、屏幕录制权限仍诚实标记为”尚未实现”（macOS 无廉价 API 可查询）。
- 插件导入、图标缓存清理、日志诊断仍明确标记为”尚未实现”。

Implemented:

1. 三态主题 UI 继续复用现有 `ThemeStore`，切换后刷新窗口。
2. 插件窗口保留时间可调、可恢复默认，并持久化到独立 settings JSON。
3. `system-settings` 与 `app-launcher` 共享 `AppIndexService`，页面展示真实扫描状态并支持重扫描。
4. 诊断区改为展示 `AppPaths::data_dir()` 直接返回的真实路径（data / config / logs），不再间接推导或使用占位字符串。
5. 未完成能力统一以 disabled/status 形式明确呈现。
6. 辅助功能权限通过 `AXIsProcessTrusted` FFI 检测真实状态（`platform::macos` 模块）。
7. “打开设置”按钮调用 `open_accessibility_settings` 打开 macOS 系统设置面板，操作后刷新状态。
8. 诊断目录（数据/配置/日志）增加”打开”按钮，调用 `platform::shell::open_directory` 并显示成功/失败消息。

Still Incomplete:

- 插件导入目录和 ZIP 导入流程（需要 plugin loading 架构支持，不适合当前批次）。
- 已安装插件管理。
- 剪贴板访问、文件访问、屏幕录制权限检测（macOS 无廉价公开 API）。
- 图标缓存清理和日志诊断动作。
- 系统跟随（监听 macOS 外观变化实时切换）。

Acceptance:

- 切换主题后所有窗口刷新。
- 重启后主题模式保持。
- System 模式在 macOS 能读有效主题。
- diagnostics 路径来自 `AppPaths`。

### gpui-demo

Reference:

- 不迁移 `qml_demo`。
- Qingqi 自有：`src/features/gpui_demo/plugin.rs`。

Functional Plan:

- 作为 Qingqi 的 GPUI 控件实验场。
- 展示真实可交互控件，不展示 QML 教程。
- 每新增共享控件，必须在此插件添加最小 demo。
- 作为低级模型学习 Qingqi UI 写法的本地样例。

UI Plan:

- 左侧 demo nav：Buttons、Inputs、TextEditor、List、Tabs、Dialog/Menu、SplitPane、Background。
- 右侧 demo content：每个 demo 都有可点击/可输入行为。
- 底部 status 显示最近交互结果。
- 不使用大段说明文字；以真实控件展示为主。

Rust Plan:

- `manifest.rs`: id `gpui-demo`、prefix `gpui/demo`。
- `demo_model.rs`: `DemoKind`、`DemoState`。
- `view.rs`: nav、demo panels、status。
- 可选 `service.rs`: background task demo。

Execution Batches:

1. 拆 manifest/view。
2. 增加 demo nav 和 selection。
3. 增加 Buttons/Inputs demo。
4. 增加 TextEditor demo。
5. 增加 List/Tabs demo。
6. 增加 Dialog/Menu/SplitPane demo。
7. 增加 background task demo。

Acceptance:

- 启动器搜索 `gpui` 能打开。
- 每个 demo 至少一个真实交互。
- 不出现 `QML 学习演示` 入口。

### image-compress

Reference:

- `image_compress/plugin.json`
- `image_compress/ImageCompressPage.qml`
- `image_compress/service.py`
- `image_compress/view_model.py`

Current Qingqi:

- `manifest.rs`/`plugin.rs` 已注册真实 runtime。
- `model.rs` 已迁移 profile 基本模型：protocol、host、port、username、auth method、remote/local dir、encoding、passive、pinned、notes。
- `store.rs` 使用 SQLite 保存 `remote_file_profiles`，支持 list/get/create/delete/toggle pinned/update last used，并会 seed 示例 profile。
- `service.rs` 管 selected/connected profile、状态消息和远程条目 VM；当前连接是配置就绪态，真实网络 backend 尚未接入。
- `view.rs` 已从静态演示改成读取真实 profile store，支持选择 profile、添加示例、置顶、删除、连接/断开状态。
- 核心插件 manager/window 已增加 panic 隔离；插件打开、命令、后台启动、render、关闭 panic 会记录并回退错误页，避免带崩核心应用。

Functional Plan:

- 导入 PNG/JPEG/WebP。
- 批量压缩。
- 设置输出格式、质量、尺寸策略、覆盖/另存为。
- 结果展示原大小、新大小、压缩率、状态、错误。
- 支持复制路径、覆盖原图、另存为、重试、移除。
- 所有压缩后台执行。

UI Plan:

- Header：标题、导入按钮、输出目录/策略。
- Drop/import area：空状态、拖拽 hover、选择文件。
- Parameter panel：格式、质量、尺寸、覆盖策略。
- Result list：文件名、尺寸、原大小、新大小、压缩率、状态、操作按钮。
- Status bar：总数、成功数、失败数、节省空间。
- QML 对齐：列表行固定高度，操作按钮为小按钮，错误状态显红色。

Rust Plan:

- `manifest.rs`: id `image-compress`、prefix `img/image`。
- `model.rs`: `ImageEntry`、`CompressOptions`、`CompressStatus`、`CompressResult`。
- `service.rs`: validate、compress_one、compress_batch、save_as。
- `store.rs`: optional recent settings/history。
- `view.rs`: import area、options form、result list。

Execution Batches:

1. 建 feature 目录和 manifest，替换 stub。 ✅ 已完成
2. 实现 service validation 和 tiny fixture 单测。 ✅ 已完成
3. 接 `image`/`oxipng`/JPEG/WebP crate，完成单文件压缩。 ✅ 已完成
4. 实现 batch background task。（尚未 — 当前同步执行，后续切 JobProvider）
5. 复刻导入区和参数区。 ✅ 已完成（含剪贴板图片真实导入 + 文件路径 fallback）
6. 复刻结果列表和操作按钮。 ✅ 已完成 v1（定位/覆盖/另存为/重试/移除）
7. 增加历史/最近设置。（尚未）

Current Notes:

- 剪贴板粘贴已分为两路：优先读取 GPUI clipboard image payload，materialize 到本地临时文件后以 `from_clipboard=true` 入队；fallback 为文本路径解析，`from_clipboard=false`。
- `from_clipboard=true` 的条目禁用「覆盖原图」，按钮不会出现；`from_clipboard=false` 且有真实源文件时才会显示覆盖按钮。
- 每条结果行的 per-entry 操作：「定位」(macOS open Finder)、「覆盖」(overwrite-original copy)、「另存为」(rfd native save dialog)、「重试」(re-compress failed entry)、「移除」。
- 压缩仍是同步执行（run_compression 在主线程循环），未接 background task。后续批次应切到 JobProvider + channel 回调。
- SaveAs 使用 `rfd::FileDialog::save_file()` 原生对话框，需要主线程。
- 15 个测试通过：2 个 service 压缩测试 + 13 个 view 路径解析/格式化测试。

Acceptance:

- tiny PNG/JPEG/WebP fixture 压缩成功。
- invalid file 显示错误，不崩溃。
- 批量任务进度更新。
- UI 线程不卡顿。

### download-manager

Reference:

- `download_manager/plugin.json`
- `download_manager/DownloadManagerPage.qml`
- `download_manager/repository.py`
- `download_manager/service.py`
- `download_manager/view_model.py`

Current Qingqi:

- `src/features/download_manager/manifest.rs` — id `download-manager`、prefix `down/download`
- `src/features/download_manager/model.rs` — `DownloadTask`、`TaskStatus`、`FileCategory`、`DownloadSettings`、URL 提取和文件名解析
- `src/features/download_manager/store.rs` — SQLite 持久化，schema v2，tasks + settings 表，migration
- `src/features/download_manager/service.rs` — 下载队列管理、HTTP client、并发控制、限速、重试、设置管理
- `src/features/download_manager/plugin.rs` — runtime/session glue
- `src/features/download_manager/view.rs` — GPUI entity view，任务列表、进度条、操作按钮、状态栏

Functional Plan:

- 新建下载任务（单 URL / 多 URL 粘贴提取）。
- 队列、开始、暂停、恢复、取消、重试、删除。
- 并发限制和限速（真实执行）。
- 保存目录（来自 settings，可通过代码设置）。
- 进度、速度、剩余时间、状态。
- 任务持久化，重启后恢复未完成/失败状态。
- 设置持久化：保存根目录、超时、重试次数、代理 URL、User-Agent、Referer、Cookie、自定义请求头。
- 清空已完成/清空失败。
- 暂停全部/恢复全部。
- 打开下载目录/定位文件。

UI Plan:

- 顶部输入区：URL 输入框 + 添加按钮（支持粘贴多 URL 文本自动提取）。
- 底部操作条：暂停全部、恢复全部、清空已完成、清空失败。
- 任务列表：文件名、大小、进度条（relative 宽度）、速度、剩余时间、状态标签、操作按钮。
- 操作按钮：重试（失败/已取消任务）、打开目录（已完成任务）。
- Status bar：目录路径、任务统计。
- Empty state：暂无下载任务。

Rust Plan:

- `manifest.rs`: id `download-manager`、prefix `down/download`。✅ 已完成
- `model.rs`: `DownloadTask`、`TaskStatus`、`FileCategory`、`DownloadSettings`、URL 提取、文件名解析、自定义请求头解析。✅ 已完成
- `store.rs`: tasks schema v2、settings 表、migration、CRUD、统计。✅ 已完成
- `service.rs`: 队列管理、HTTP client（reqwest blocking）、并发控制、限速节流、重试、设置管理。✅ 已完成
- `view.rs`: 输入区、任务列表、进度条、操作按钮、状态栏。✅ 已完成

Execution Batches:

1. 建 feature 目录和 manifest，替换 stub。✅ 已完成
2. 实现 store schema 和 CRUD 单测。✅ 已完成（19 tests）
3. 实现 service 层：队列、并发、限速、HTTP client、设置。✅ 已完成
4. 复刻主 UI 表格和操作按钮。✅ 已完成
5. 多 URL 粘贴提取、清空失败、重试、打开目录。✅ 已完成
6. 设置持久化和真实执行。✅ 已完成（save_root、timeout、retry、proxy、headers 均已接入下载行为）
7. 复刻设置弹窗和分类筛选。（尚未实现）
8. 像素对齐 suishou QML 布局。（尚未实现）

Current Notes:

- 核心下载链路完整：添加 URL → 提取文件名 → HTTP 下载（支持代理、自定义请求头、超时、重试）→ 进度回调 → 持久化状态。
- 并发控制真实执行：`start_download` 检查 `active.len() >= settings.max_concurrent`，排队任务在已有任务完成后自动启动。
- 限速真实执行：下载循环中按 `speed_limit_kbps` 对每次 read 进行节流。
- 自动重试：HTTP 408/425/429/5xx 响应自动重试，次数由 `retry_limit` 控制。
- 设置通过 `Arc<Mutex<DownloadSettings>>` 在 service 和 downloader 线程间共享，`save_settings` 持久化到 SQLite。
- View 层通过 `service.tasks_snapshot()` 和 `service.stats()` 获取只读快照，不直接读 store。
- 进度条使用 `relative(pct/100.0)` 而非 `px()`，符合 GPUI 相对布局语义。
- 暂停全部会暂停活跃和排队任务；恢复全部会恢复暂停/失败/已取消任务。
- 多 URL 粘贴：输入框中粘贴包含多个 URL 的文本时，自动提取并批量创建任务。
- 打开目录：已完成任务可通过按钮打开所在目录（macOS `open` 命令）。
- 仍未实现：设置 UI 编辑弹窗、分类筛选标签（视频/音频/文档等）、文件选择器设置保存目录、并发/限速 UI 控件、待下载队列的独立 UI 区。

Acceptance:

- 添加 URL 后生成任务，文件名为 URL 路径末段。
- HTTP 下载真实推进进度到完成，文件落盘。
- 暂停/恢复/取消后状态持久化到 SQLite。
- 重启后未完成任务恢复为 Pending 状态。
- store 单测覆盖 insert/get/update/list/delete/clear/stats/settings。
- model 单测覆盖 URL 提取、文件名解析、自定义请求头解析、进度/ETA 计算。

### api-debugger

Reference:

- `api_debugger/plugin.json`
- `api_debugger/ApiDebuggerPage.qml`
- `api_debugger/EnvManagerDialog.qml`
- `api_debugger/db.py`
- `api_debugger/request_editor_state.py`
- `api_debugger/request_sender.py`
- `api_debugger/response_state.py`
- `api_debugger/script_service.py`
- `api_debugger/variable_service.py`
- `api_debugger/ws_service.py`
- `api_debugger/tabs_controller.py`

Current Qingqi:

- `src/features/api_debugger/manifest.rs`
- `src/features/api_debugger/model.rs`
- `src/features/api_debugger/store.rs` — SQLite schema v1, collection_nodes / environments / env variables / env headers / http_tabs / http_history / api_variables
- `src/features/api_debugger/service.rs` — workspace loading from SQLite, collection tree builder, endpoint snapshot persistence, environment listing, 9 unit tests
- `src/features/api_debugger/view.rs` — GPUI entity view, honest empty state, endpoint persist-on-change
- `src/features/api_debugger/plugin.rs` — runtime/session glue

Functional Plan:

- HTTP request editor：method、url、params、headers、cookies、body、auth、pre/post scripts。
- Collection/case 管理。
- Environment 和变量解析。
- 发送 HTTP、上传文件、显示响应。
- Response 格式化：status、headers、body、timing、size、assertion result。
- WebSocket connect/send/receive/disconnect/timeline。
- Request tabs：打开、关闭、保存、切换。

UI Plan:

- 三栏工作台：左 collection tree，中 request editor，右 response panel。
- 顶部 tab bar 和 method/url/send toolbar。
- Request editor tabs：Params、Headers、Cookies、Body、Auth、Pre、Post。
- Body editor 使用 `TextEditor`，支持 JSON/raw/form/multipart。
- Response panel：title/status、body viewer、headers/details/assertions。
- Env manager dialog：环境列表、变量表、保存/删除。
- WebSocket 区：连接按钮、编码选择、发送内容、timeline。

Rust Plan:

- `manifest.rs`: id `api-debugger`、prefix `api/http`。
- `model.rs`: request/response/environment/case/tab/body/auth/ws models。
- `store.rs`: schema for collections、requests、environments、history、ws timeline。
- `request_editor.rs`: parse/build kv/header/cookie/form rows。
- `variable_service.rs`: variable resolution and magic values。
- `script_service.rs`: assertions and extraction。
- `transport.rs`: trait `HttpClient`/`WsClient` and real implementation。
- `service.rs`: send request, upload file, websocket session manager。
- `view.rs`: workbench panes and dialogs。

Execution Batches:

1. 只做 model + request_editor 单测。✅ 已完成（model 类型已就绪，基础类型通过编译）
2. 做 variable_service 和 script_service 单测。✅ 已完成（variable_service 13 tests + script_service 14 tests，覆盖 resolve/pre-ops/assertions/extraction）
3. 做 store schema/migration。✅ 已完成（schema v1，collection_nodes / environments / env_variables / env_headers / http_tabs / http_history / api_variables，store 单测 12 tests）
4. 做 mocked HTTP transport。✅ 已完成（curl subprocess transport + pre-ops + variable resolution + assertion + history persistence + tab state）
5. 做 real HTTP transport。✅ 已完成（与 batch 4 合并：真实 curl 执行，非 mock）
6. 做 response formatting。✅ 已完成 v1（status_line / duration_ms / size_bytes / assertion_results）
7. 做 tabs controller。✅ 已完成 v1（tab state 通过 save_tab 持久化到 SQLite，send 时自动保存）
8. 做基础三栏 UI。✅ 已完成 v1（collection tree + request editor + response area，honest empty state）
9. 做 Env manager。（尚未开始 — environments schema 已就绪，UI 尚未接入）
10. 做 WebSocket。（尚未开始）

Current Notes:

- `load_workspace()` 不再依赖 `sample_groups()`。真实数据来源为 SQLite `collection_nodes` 表，通过 `build_collection_tree()` 转换为 UI `Vec<ApiGroup>` 树结构。
- 空集合状态诚实显示：无数据时创建一个占位 group + 一条空请求，状态栏提示"集合为空，点击 + 创建第一个请求"。
- `persist_endpoint_snapshot()` 已实现：选中端点编辑后自动将 method/url/params/headers/body/auth 序列化为 `RequestSnapshot` 写回 SQLite。
- 环境列表从 SQLite 加载（`list_environments_ui()`），无数据时回退到诚实默认值（`default_environments()`），不再是假 demo 数据。
- `service.rs` 共 16 个单测：原有 9 个（build groups/persist/env/kv/method/auth）+ 新增 7 个（resolve_with_temp_overrides_env_vars、resolve_with_temp_falls_back_to_env、pre_ops_draft_modifies_url_headers_body、history_persisted_after_send、tab_state_persisted、script_assertion_results_in_response、format_kv_rows_filters_empty_keys）。
- `variable_service.rs` 13 个单测 + `script_service.rs` 14 个单测已就绪，覆盖 resolve/pre-ops/assertions/extraction。
- HTTP transport 已真实接入：`send_request` 走 curl subprocess，执行前应用 pre-ops（`set`/`header`/`query`/`body.append`），变量通过 `resolve_with_temp` 解析（pre-op 临时变量 > 环境变量），响应后跑 assertions 并持久化 history 和 tab state。
- `ApiResponse` 新增 `assertion_results: Vec<(String, bool)>` 字段，view 层在 notice 和 Logs tab 中展示断言结果。
- 空响应状态改为诚实占位（`status_code: 0`，body 提示"发送请求后显示"），不再使用 `sample_response()`。
- 2026-05-28 验证结果：`cargo test --bin qingqi -- features::api_debugger` 59/59 通过，`cargo check` 通过，无新增 warning。
- 剩余主要缺口：环境 CRUD UI、request tabs 切换/多 tab 并发、WebSocket、认证类型（Bearer/Basic/OAuth）、文件上传、response body 格式化（JSON 高亮）。

Acceptance:

- 不启动 GPUI 可测试 request parser、变量、断言。
- mock HTTP 请求可返回 response DTO。
- UI 发送请求不阻塞。
- 大集合树分页/懒加载。

### http-capture

Reference:

- `http_capture/plugin.json`
- `http_capture/HttpCapturePage.qml`
- `http_capture/service.py`
- `http_capture/view_model.py`

Current Qingqi:

- `src/features/http_capture/manifest.rs` — id `http-capture`、prefix `cap/capture/httpcap`、background true
- `src/features/http_capture/model.rs` — `CapturedExchange`、`FilterState`、`CaptureStats`、`CaptureMethod`、`HeaderEntry`
- `src/features/http_capture/store.rs` — SQLite persistence with `captured_exchanges` table, insert/query/count/get_by_id/clear, paged query, filter support
- `src/features/http_capture/plugin.rs` — `HttpCaptureRuntime` + `HttpCaptureSession`, registered as builtin plugin
- `src/features/http_capture/view.rs` — GPUI entity `CapturePanel` with real store-backed data access, filter panel, exchange list, detail panel, pagination

Functional Plan:

- 代理状态：启动、停止、端口、系统代理状态。（尚未接入真实代理引擎）
- HTTPS 证书：生成、安装、信任、状态提示。（尚未实现）
- 捕获请求/响应：method、url、host、status、duration、size、protocol、headers、body。（store schema 已就绪，ingestion 路径待补）
- 过滤：method、host、status、search、error only、hide static。（view 层 filter 已真实工作，经 SQLite 查询）
- 统计：visible/total/error/https/replayed/avg duration/bytes/top host。（基础统计已展示，完整 stats 计算待补）
- 详情 inspector：headers、body、timing、重放、复制。（详情面板展示基础字段，tab 式 inspector 待补）
- 手机抓包引导。（尚未实现）

UI Plan:

- Header：HTTP 抓包工作台、proxy url、start/stop、状态。（header 完成，引擎状态明确标注"未接入"）
- 左侧 filter/stat/cert panel。（filter panel 完成：关键词搜索、Host 过滤、方法 chips、错误/HTTPS toggle、统计）
- 中间 capture table，固定 row height，按状态着色。（完成：固定 32px row height，状态着色，时间/方法/Host/URL/状态/大小/耗时列）
- 右侧 detail inspector，tabs for overview/headers/body/timing。（基础详情完成，tab 式 inspector 待补）
- HTTPS 解密和手机抓包使用小 card，不弹大说明页。（尚未实现）
- Empty state：未启动代理/暂无流量。（完成：空状态明确提示"暂无抓包记录 — 请先接入代理捕获引擎"；过滤无匹配也有独立提示）

Rust Plan:

- `manifest.rs`: id `http-capture`、prefix `cap/capture/httpcap/mitm`。✅ 已完成
- `model.rs`: `CaptureSession`、`CapturedExchange`、`ProxyState`、`CertificateState`、`FilterState`、`CaptureStats`。✅ 已完成（核心类型已就绪，`CaptureSession`/`ProxyState`/`CertificateState` 枚举定义完成但尚未接入运行时）
- `store.rs`: captured exchanges schema and paged query。✅ 已完成（含 schema v1、insert/query/count/get_by_id/clear、分页、多条件过滤）
- `platform/macos.rs`: system proxy、certificate trust。（尚未实现）
- `proxy.rs`: proxy trait and mock/real implementation。（尚未实现）
- `service.rs`: event ingestion、filter、stats、replay。（尚未实现独立 service，当前逻辑在 view 层）
- `view.rs`: panels/table/inspector。✅ 已完成（完整三栏布局 + 状态栏）

Execution Batches:

1. 建 model/filter/stats 单测。✅ 已完成（15 tests：method/display、filter_by_method/host/search/https_only/error_only/hide_static/all_passes、status_color、formatted_size/duration、detail_tab、header_entry）
2. 建 store insert/page/search 单测。✅ 已完成（7 tests：schema、insert_and_get_by_id、query_with_filter、pagination、count_and_total_count、clear、query_ordered_by_id_desc）
3. 建 platform/proxy trait 和 mock。（尚未开始）
4. 实现 UI skeleton with mock data。✅ 已完成（三栏 GPUI entity view + 真实 store 数据查询 + 空状态 + 分页）
5. 接真实代理事件流。（尚未开始）
6. 接证书和系统代理。（尚未开始）
7. 接重放/导出。（尚未开始）

Current Notes:

- manifest、model、store 均已完整实现并通过单测（manifest 6 tests、model 15 tests、store 7 tests）。
- `CapturePanel` 是 GPUI `Entity` view，与 store 通过 `Rc<CaptureStore>` 共享数据，符合 architecture spec 的 service/store/view 边界。
- 搜索和 Host 过滤通过 `cx.observe(entity)` 实时响应输入变更，每次输入变化自动 reset offset 并重新查询 SQLite。
- 方法过滤（GET/POST/PUT/DELETE）使用 chip toggle 模式，错误和 HTTPS toggle 也支持独立切换。
- 列表每页 50 条，支持上/下翻页，选中行显示高亮并通过 `get_by_id` 加载右侧详情。
- "清空记录" 按钮调用 `store.clear()` 真实删除 SQLite 数据后刷新视图。
- 引擎状态通过 `engine_running: bool` 字段明确区分：false 时 header 和 status bar 显示"捕获引擎未接入 — 仅可浏览已存储的抓包记录"，不做虚假状态。
- 当前无 live capture 路径，所有数据来自 SQLite 持久化查询。后续需要接入真实代理引擎（mitmproxy/mitm 或自定义 proxy），实现 service 层的 event ingestion → store insert → AppEventBus → view refresh 的完整数据流。
- 缺少独立 `service.rs`：当前过滤/查询/清空逻辑直接在 view 层调用 store，不符合 architecture spec 推荐的 service snapshot/DTO 模式。后续应抽取 `HttpCaptureService`。
- 详情区目前是 key-value 列表，缺少 tab 式 inspector（概览/请求头/请求体/响应头/响应体/计时）和 headers/body 的格式化展示。

Acceptance:

- mock proxy start/stop 状态正确。（尚未 — 无 proxy mock）
- filter/stats 单测通过。✅ model + store 单测全部通过
- 1000 条记录分页渲染不卡。（待验证 — store 分页 query 已实现，但未做 1000 条 benchmark）
- 未安装证书时 UI 明确提示。（尚未 — 证书相关 UI 未实现）

### ftp-sftp-ssh-client

Reference:

- `ftp_sftp_ssh_client/plugin.json`
- `ftp_sftp_ssh_client/FtpSftpSshClientPage.qml`
- `ftp_sftp_ssh_client/models.py`
- `repository.py`
- `connection_pool.py`
- `service.py`
- `transfer_service.py`
- `terminal_session.py`

Current Qingqi:

- `src/features/ftp_sftp_ssh_client/manifest.rs` — 完成
- `src/features/ftp_sftp_ssh_client/model.rs` — `RemoteProfile`、`RemoteProfileDraft`、`RemoteProtocol`、`AuthMethod`、`ConnectionStatus`、`RemoteFileItem`、`TransferItem` 等
- `src/features/ftp_sftp_ssh_client/store.rs` — profiles SQLite，create/update/list/get/delete/toggle pinned/last used
- `src/features/ftp_sftp_ssh_client/service.rs` — `FtpSftpSshService`，profile CRUD + connect/disconnect/navigate/mkdir/rename/delete + transfer
- `src/features/ftp_sftp_ssh_client/backend.rs` — `RemoteBackend` trait + `SftpBackend`（ssh2）+ `FtpBackend`（suppaftp）
- `src/features/ftp_sftp_ssh_client/pool.rs` — `RemoteConnectionPool`
- `src/features/ftp_sftp_ssh_client/transfer.rs` — `TransferService`，upload/download/cancel/clear
- `src/features/ftp_sftp_ssh_client/view.rs` — GPUI entity view，sidebar + editor + remote file list + transfer strip
- `src/features/ftp_sftp_ssh_client/plugin.rs` — runtime + session + revision watcher
- 已注册为 builtin plugin（`src/features/registry.rs`）
- SFTP/FTP 真实连接后端已接通，FTPS 和 SSH terminal 仍为后续批次

Functional Plan:

- 连接 profile CRUD：protocol、host、port、username、auth method、remote root。
- 连接池：connect/disconnect/status。
- 远程文件浏览：list、mkdir、rename、delete、refresh。
- 传输：upload、download、queue、progress、cancel、retry。
- SSH terminal：第一阶段 mock/log，第二阶段接真实 terminal backend。
- 支持 FTP/SFTP/FTPS/SSH 分阶段，不一次完成。

UI Plan:

- 左侧连接管理列表，row height 50px，protocol chip。
- 顶部连接状态：当前 profile、host、status、connect/disconnect。
- 主文件区：路径面包屑、toolbar、文件表格。
- 底部 transfer queue：文件、本地路径、远程路径、进度、状态、操作。
- 终端/log 区：可折叠，显示 session 输出。
- Context menu：连接、编辑、复制、删除、上传、下载、重命名。

Rust Plan:

- `manifest.rs`: id `ftp-sftp-ssh-client`、prefix `ftp/sftp/ssh`。已完成。
- `model.rs`: `RemoteProfile`、`RemoteProfileDraft`、`RemoteProtocol`、`AuthMethod`、`ConnectionStatus`、`RemoteFileItem`。已完成 suishou profile 主字段；`TransferTask`、`TerminalEvent` 待补。
- `store.rs`: profiles SQLite store。已完成 create/update/list/get/delete/toggle pinned/last used 和旧 DB `ALTER` migration；recent paths 待补。
- `service.rs`: selected/connected profile state、profile create/update/delete 和真实 UI VM。已完成第一批；连接池待补。
- `backend.rs`: trait `RemoteBackend` and local mock backend。
- `connection_pool.rs`: active sessions。
- `transfer_service.rs`: queue/progress/cancel。
- `terminal.rs`: terminal trait/mock/real later。
- `view.rs`: profiles/file table/editor/transfer queue/terminal。profiles 已接真实 store，右侧 profile 编辑器已可创建/保存 suishou 主字段；远程文件列表已接 SFTP/FTP 真实 list_dir；传输队列已接 upload/download 真实路径；终端仍未接。

Execution Batches:

1. 建 model 和 profile store。已完成。
2. 建 `RemoteBackend` trait 和 mock backend。已完成，已升级为 SFTP（ssh2）和 FTP（suppaftp）真实后端。
3. 建 transfer queue state machine。已完成 `TransferService`，upload/download/cancel/clear。
4. 复刻连接列表和顶部状态。已完成，profile 编辑表单、保存/新建/重置/删除、连接/断开均已接通。
5. 复刻文件表格，用真实 SFTP/FTP backend。已完成 list_dir/navigate/mkdir/rename/delete。
6. 复刻传输队列。已完成 upload/download + 进度 + cancel。
7. 接 SFTP crate。已完成（ssh2 SftpBackend）。
8. 接 FTP/FTPS。已完成 FTP（suppaftp），FTPS 仍为后续批次。
9. 接 SSH terminal。待做。

Acceptance:

- profile CRUD 可通过 UI 创建、更新、删除、置顶并持久化到 SQLite。
- SFTP 连接后能真实 list_dir 并在 UI 中浏览远程文件。
- FTP 连接后能真实 list_dir。
- upload/download 在后台线程执行，transfer strip 显示进度。
- transfer cancel 生效。
- 已注册为 builtin plugin，启动器可搜索打开。

## Store and Migration Rules

SQLite store 规则：

- 每个插件使用独立数据库，路径由 `AppPaths::database("<plugin>.db")` 派生。
- schema 必须有版本表或 `PRAGMA user_version`。
- migration 必须显式，不能隐式吞错。
- 查询接口返回 DTO，不返回 rusqlite row。
- 大列表接口必须分页。
- delete/update 返回 bool 或 affected rows。
- 单测使用临时目录或内存数据库。

推荐接口：

```rust
pub struct Page<T> {
    pub rows: Vec<T>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}
```

## Background Task Rules

用 GPUI background executor 或 async task：

- 文件扫描。
- app index。
- clipboard polling。
- downloads。
- image compression。
- network request。
- proxy capture。
- FTP/SFTP operations。
- quick launch process execution。

结果回主线程：

- 更新 service state。
- 发送 channel message。
- `cx.notify()` 或当前窗口的 `window.refresh()`。

不要：

- 在锁内 await。
- 在锁内执行进程。
- 在 render clone 万条记录。
- 在 on_click 里 `.output()` 等待长命令。

## Warning Policy

当前工程仍有一批 warning，主要来自尚未收敛的 theme/ui helper 和未接入方法。后续规则：

- 新代码不得新增 warning。
- 每个迁移任务至少清理自己触碰文件中的无用 import、无用方法。
- 共享 UI helper 如果是未来设计系统，集中保留并在 `gpui-demo` 使用；否则删除。
- 不要用大范围 `#[allow(dead_code)]` 掩盖真实未接入功能。

## Acceptance Checklist

每个插件从 `Stub` 升级到 `Functional v1` 前必须满足：

- manifest 与 suishou `plugin.json` 对齐，Qingqi 特有差异写进本文档。
- command 可从启动器打开。
- 有真实输入或真实数据。
- 主要按钮有真实行为。
- 有 empty/loading/error/status 至少三类状态。
- 业务 service 或 store 有单元测试。
- `cargo check` 无 error。
- 新增代码无 warning。
- 矩阵已更新。

升到 `Feature parity` 前必须满足：

- suishou 主要功能都存在。
- 数据持久化、后台任务、动态命令完整。
- 复杂插件支持关闭/重开窗口不丢必要状态。
- 大列表分页或虚拟化。
- 手动打开/关闭 10 次无崩溃。

升到 `Pixel parity` 前必须满足：

- QML 中主要尺寸、间距、字体、颜色、row height、card radius 已对齐。
- hover/selected/disabled/error/loading 状态对齐。
- 弹窗、菜单、tabs、toolbar、status bar 对齐。
- 在浅色和深色主题下检查。
- 更新本文档 Pixel notes。

## Test Plan

Core:

- command scoring。
- prefix 匹配。
- dynamic command 注册。
- manifest window spec。
- PluginRuntime open/close/shutdown。

UI state:

- SingleLineInput 输入、paste、clear、focus。
- TextEditor 多行、选择、复制、只读、滚动。
- Launcher query 实时搜索和 Enter 打开。

Store:

- SQLite schema 初始化。
- schema migration。
- page/search/delete。
- bad path/error handling。

Plugin:

- JSON format/compact/query/error。
- QR matrix/save/history。
- Quick Launch parameter/executor/store。
- App Launcher index/filter/open command payload。
- Clipboard capture/search/delete/ignore。
- Download mocked progress。
- Image compression fixture。
- API debugger parser/variable/assertion。
- HTTP capture filter/stats。
- FTP mock backend。

Manual:

1. `cargo run` 启动。
2. 打开启动器，直接输入搜索。
3. Enter 打开当前项。
4. 打开每个已迁插件。
5. 切换主题。
6. 重复关闭/打开窗口。
7. 观察无卡顿、无崩溃、无 UI 重叠。

## Handoff Format

每次完成一个迁移任务，在最终回复和本文档中留下：

```text
Plugin: <id>
Reference files read:
- ...
Rust files changed:
- ...
Business status:
- ...
UI status:
- ...
Tests run:
- ...
Warnings:
- before N, after M
Matrix update:
- Stub -> Functional v1
Remaining:
- ...
```

## Current Known Gaps

- `TextEditor` 尚未完成，JSON/API/脚本类插件仍受限。
- `ServiceRegistry` 尚未落地，runtime 仍直接持有部分服务。
- theme 仍是全局 bool，未持久化，未跟随系统。
- `app-launcher` 已补真实 app icon、双击打开、别名搜索（含 CamelCase 拆分和 bundle-id 归一化）、分页展示、metadata-first 两阶段刷新与图标缓存容错（零字节/损坏文件自动清理并回退字母 tile）；剩余是虚拟列表和更多像素细节。
- `quick-launch` 已切到 store-backed 动作仓库，但参数提取、后台 executor、编辑器和历史面板仍未完成。
- `http-capture` 已完成 model + store + view 的 Functional v1 基础：SQLite 持久化、分页查询、多条件过滤、详情面板、空状态均真实可用；缺少独立 service 层、真实代理引擎、证书管理和系统代理接口。
- `download-manager` 已升级到 `Functional v1`：核心下载链路、并发控制、限速、重试、设置持久化、多 URL 提取、清空失败/已完成、暂停全部/恢复全部均已真实可用；剩余是设置 UI 弹窗、分类筛选标签和像素对齐。
- `ui.rs/theme.rs` helper 仍需收敛，最好通过 `gpui-demo` 反向验证哪些组件真实通用。
