# GPT-5.4 执行手册：Qingqi workspace 拆分

> 本手册给执行模型使用，必须服从 [workspace-split-guide.md](workspace-split-guide.md)。
> `workspace-split-guide.md` 决定边界与取舍；本文只把它拆成可执行步骤。
>
> 执行总原则：**一次只做一个阶段，阶段内每个子步骤都落终态代码**。不得加入兼容别名、转发壳、双机制、临时兜底、"以后再删"的过渡物。

---

## 0. 执行模型守则

1. 每次开工先读 `docs/workspace-split-guide.md` 的 §0、§2、§4、§5、§6、§8、§9、§10。
2. 本手册是执行顺序，不是新的设计来源。若本文与主导文档冲突，停下并修本文，不要按冲突内容改代码。
3. 每次只推进一个 P 阶段。阶段完成后必须跑本阶段验收命令，并在提交说明里记录结果。
4. 不保留旧路径 re-export、兼容 trait、双注册机制、双事件总线、双 accent 枚举。
5. 不让 `qingqi-app`、`qingqi-core` 依赖任何 `qingqi-feature-*`。
6. 不让 `qingqi-feature-*` 依赖 `qingqi-app` 或 `qingqi-core`。
7. 不建立 `qingqi-sdk` 伞 crate。第三方插件未来直接依赖 `qingqi-plugin` + `qingqi-ui`。
8. 遇到 circular dependency，回到主导文档 §5 破环；禁止用 Cargo feature、转发壳或抽象假 facade 硬绕。
9. 工作区有未提交改动时，先 `git status --short`，只改当前阶段需要的文件，不回滚别人的改动。

---

## 1. 决策锁定

执行时不要重新讨论这些点：

| 决策 | 结论 |
|---|---|
| 外部插件模型 | 仅进程内 Rust 插件；SDK 含 GPUI；不做声明式/IPC/wasm/JSON-RPC 通道 |
| `ViewModel/Field/FieldKind` | 删除，不纳入 SDK |
| 内置插件粒度 | 每插件一个 crate：`qingqi-feature-<name>` |
| 存储归属 | `DatabaseService` / `AppPaths` / `DatabaseSpec` / `PluginDictStore` 放 `qingqi-plugin` |
| UI 归属 | theme / theme_mode / ui / components / text_input / assets 放 `qingqi-ui` |
| shortcut | 声明类型在 `qingqi-plugin`；注册、解析、派发服务在 `qingqi-app` |
| 组合根 | bin crate `qingqi` 装配内置插件；app/core 不识具体插件 |

---

## 2. 标准执行循环

每个阶段按这个循环走：

```powershell
git status --short
cargo fmt
cargo check
```

如果阶段开始前 `cargo check` 已失败，记录基线错误，不要把它当成本阶段引入的错误。阶段完成后至少要保证：

```powershell
cargo fmt
cargo check --workspace
cargo test --workspace
```

拆分后每个阶段增加依赖体检：

```powershell
cargo tree -p qingqi-app | rg "qingqi-feature"      # 期望无输出
cargo tree -p qingqi-core | rg "qingqi-feature"     # 期望无输出
cargo tree -p qingqi-plugin | rg "qingqi-(core|app|platform|feature)" # 期望无输出
```

阶段交付说明必须包含：

- 完成的 P 阶段和子步骤。
- 关键文件移动/删除。
- 验证命令结果。
- 未解决问题，若有。

---

## 3. 目标 crate 清单

最终 workspace：

```text
crates/
  qingqi/                         bin：main + 组合根 + build.rs + assets/bundle
  qingqi-plugin/                  SDK 契约
  qingqi-ui/                      主题、组件、资源、TextInput
  qingqi-platform/                OS 服务
  qingqi-core/                    插件宿主与命令排序
  qingqi-app/                     GPUI 外壳
  qingqi-feature-about/
  qingqi-feature-anti-peeping/
  qingqi-feature-api-debugger/
  qingqi-feature-clipboard/
  qingqi-feature-download-manager/
  qingqi-feature-ftp-sftp-ssh-client/
  qingqi-feature-gpui-demo/
  qingqi-feature-http-capture/
  qingqi-feature-image-compress/
  qingqi-feature-json-parser/
  qingqi-feature-qr-code/
  qingqi-feature-quick-launch/
  qingqi-feature-system-settings/
```

`src/features/stub_plugin.rs` 不默认成为 crate。先确认是否仍被引用；若零引用且只为旧预览占位，按 P0 删除。

---

## 4. P0 预清理

目标：先删除不进入 SDK 的旧机制，避免把噪音搬进新 crate。

### P0.1 死代码与废弃模块

1. 删除根级 `#![allow(dead_code)]`。
2. 删除 `src/core/view_model.rs`。
3. 从 `src/core/mod.rs` 移除 `pub mod view_model;`。
4. 搜索确认没有引用：

```powershell
rg -n "view_model|ViewModel|FieldKind|Field" src
```

验收：无生产引用；若有引用，先判断是否应删除引用，而不是保留声明式通道。

### P0.2 收编通用存储

1. 保留 `PluginDictStore`，目标归属是 `qingqi-plugin`。
2. 若当前生产零引用，不删除；在后续 P2 与 `DatabaseService` / `AppPaths` 一起进入 SDK。
3. 不新增各插件自己的通用 KV facade。

### P0.3 删除 DB 双机制

1. 搜索：

```powershell
rg -n "database_specs|with_databases|DatabaseSpec" src
```

2. 删除 `Plugin::database_specs()` 这一未被调用的 trait 方法。
3. DB 声明只保留 `PluginDescriptor::with_databases` / descriptor 路线。
4. 调整测试和实现，确保没有第二套 schema 声明入口。

验收：

```powershell
rg -n "fn database_specs|database_specs\(" src # 期望无输出
cargo check
```

### P0.4 合并 accent 枚举

1. 搜索 `PluginAccent`、`ThemeAccent`、`accent_to_theme`。
2. 保留 SDK 语义的 `PluginAccent`。
3. 删除 `ThemeAccent` 与桥接函数。
4. `qingqi-ui` 未来直接根据 `PluginAccent` 取色。

验收：不存在 `ThemeAccent` / `accent_to_theme`。

### P0.5 清理事件转发壳

1. 当前 `src/app/events.rs` 若只是 `pub use crate::core::events::*;`，删除该文件。
2. 引用方改为直接使用事件的最终归属。拆分前可先指向 `crate::core::events`；P2 后改 `qingqi_plugin::events`。
3. 禁止新建 `app::events` 兼容壳。

验收：

```powershell
rg -n "app::events|crate::app::events" src # 期望无输出
```

### P0.6 共享 UI 老旧重复只做必要清算

只处理会阻碍 `qingqi-ui` 抽取的双机制：

1. 标记并删除已无引用的旧 `ui::badge` / `ui_badge` 类重复原语。
2. 保留当前仍被大量使用的一套，先保证可编译。
3. 不在 P0 大面积改插件 UI。

---

## 5. P1 workspace 骨架

目标：建立 workspace，但先只有 bin 单成员，行为不变。

### P1.1 移动现有包

1. 新建 `crates/qingqi/`。
2. 移动：

```text
src/ -> crates/qingqi/src/
assets/ -> crates/qingqi/assets/
build.rs -> crates/qingqi/build.rs
icon_raster.rs -> crates/qingqi/icon_raster.rs
```

3. 根 `Cargo.lock` 留在根目录。
4. 根 `Cargo.toml` 改为 workspace：

```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
edition = "2024"
version = "0.1.0"

[workspace.dependencies]
# 把原依赖版本集中到这里
```

5. `crates/qingqi/Cargo.toml` 保留 package/bin/bundle 配置，依赖改 `{ workspace = true }`。

### P1.2 修资源与 build 路径

1. `build.rs` 所有资源路径基于 `CARGO_MANIFEST_DIR`。
2. bundle metadata 指向 `crates/qingqi/assets/...` 内的源或生成物。
3. README 的当前结构说明如果需要，标注“拆分前路径”。

验收：

```powershell
cargo check -p qingqi
cargo run -p qingqi # 可选手测
```

---

## 6. P2 抽 `qingqi-plugin`

目标：插件作者和宿主都依赖的稳定 SDK 契约先独立。

### P2.1 建 crate

新建：

```text
crates/qingqi-plugin/Cargo.toml
crates/qingqi-plugin/src/lib.rs
```

依赖只允许 SDK 需要的底层依赖，例如 `gpui`、`serde`、`anyhow`、`rusqlite`、`r2d2`、`tracing`。不得依赖 `qingqi-core`、`qingqi-app`、`qingqi-ui`、`qingqi-platform`、`qingqi-feature-*`。

### P2.2 移入 SDK 类型

从当前 `core/` 移入或拆出：

```text
command.rs              Command / Activation / Action / matcher / context 类型
plugin.rs               Plugin trait、PluginView、InlineView、ListView、WindowView、Manifest
plugin_spec.rs          IconRef、ViewMode、WindowSpec、PluginAccent、分类和状态
icon.rs                 IconRef
storage.rs              AppPaths
database.rs             DatabaseService / DatabaseSpec
dict_store.rs           PluginDictStore
events.rs               AppEventBus / AppEventKind
job.rs                  JobProvider / JobSnapshot
page.rs                 Page<T>
shortcut.rs             只保留 ShortcutDescriptor/Scope/Target/CoreShortcutAction 等声明类型
```

如果某个文件同时包含 SDK 类型和宿主实现，必须拆文件：

- SDK 契约进 `qingqi-plugin`。
- 宿主逻辑留待 P5 进入 `qingqi-core`。
- 平台注册/派发留待 P7 进入 `qingqi-app`。

### P2.3 调整引用

1. bin 当前代码临时依赖 `qingqi-plugin`。
2. `crate::core::X` 中属于 SDK 的引用改为 `qingqi_plugin::X`。
3. 不保留 `crate::core` re-export 兼容层。

验收：

```powershell
cargo check -p qingqi-plugin
cargo tree -p qingqi-plugin | rg "qingqi-(core|app|ui|platform|feature)" # 期望无输出
cargo check -p qingqi
```

---

## 7. P3 抽 `qingqi-ui`

目标：插件 UI 所需工具从 app 层下沉，破除 features -> app。

### P3.1 建 crate 并移动 UI 层

移动：

```text
app/theme.rs -> qingqi-ui::theme
app/theme_mode.rs -> qingqi-ui::theme_mode
app/ui/ -> qingqi-ui::ui
app/text_input.rs -> qingqi-ui::text_input
app/assets.rs -> qingqi-ui::assets
```

依赖：`qingqi-plugin`、`gpui`、`gpui-component`，以及当前 UI 资源加载需要的底层库。

### P3.2 破 C3：platform 不依赖 app/assets

1. `platform/svg_icon.rs` 不再调用 `app::assets::resolve`。
2. 改为接收已解析绝对路径，或接收 bytes/reader。
3. 资源解析只在 `qingqi-ui::assets`。

### P3.3 features 引用 UI

1. `crate::app::{theme, theme_mode, ui}` 改 `qingqi_ui::{theme, theme_mode, ui}`。
2. `crate::app::text_input::TextInput` 改 `qingqi_ui::text_input::TextInput`。
3. 插件不得再 import app。

验收：

```powershell
rg -n "crate::app|app::theme|app::ui|app::text_input" crates/qingqi/src/features
cargo check -p qingqi-ui
cargo check -p qingqi
```

期望第一条无输出，或只剩注释/迁移说明。

---

## 8. P4 抽 `qingqi-platform`

目标：OS 服务独立，且不依赖 app/core/features。

### P4.1 建 crate 并移动平台层

移动：

```text
platform/clipboard.rs
platform/hotkey.rs
platform/low_level_hook.rs
platform/tray.rs
platform/power.rs
platform/display.rs
platform/apps.rs + apps/*
platform/shell.rs
platform/svg_icon.rs
platform/macos.rs
```

平台条件依赖跟到 `qingqi-platform/Cargo.toml`。

### P4.2 清理引用方向

1. `qingqi-platform` 不依赖 `qingqi-plugin`，除非某个类型确实是 OS 服务公共契约；默认不依赖。
2. `qingqi-platform` 不依赖 `qingqi-ui`。图标栅格化只处理路径/bytes。
3. app 和 feature crate 通过 `qingqi_platform::*` 使用 OS 能力。

验收：

```powershell
cargo check -p qingqi-platform
cargo tree -p qingqi-platform | rg "qingqi-(app|core|feature)" # 期望无输出
```

---

## 9. P5 抽 `qingqi-core`

目标：插件宿主独立，只依赖 `qingqi-plugin`。

### P5.1 建 crate 并移动宿主逻辑

移动或拆出：

```text
PluginManager
FeatureRegistry
PluginDescriptor
BuildCx
CommandUsageStore
命令缓存、排序、打分宿主逻辑
```

注意：如果当前 `plugin.rs` 同时有 trait 和 manager，trait 已在 P2 进 `qingqi-plugin`，manager 在这里。

### P5.2 BuildCx 收口

`BuildCx` 至少包含主导文档要求的共享服务：

```text
db / paths / events
theme 或可注册共享服务表
app_index 或 AppCatalog 所需句柄（如果仍由插件构造期需要）
```

不要让 system settings、clipboard 等插件通过闭包捕获宿主内部对象绕过 DI。

### P5.3 核心零插件验收

```powershell
cargo check -p qingqi-core
cargo tree -p qingqi-core | rg "qingqi-feature" # 期望无输出
cargo tree -p qingqi-core | rg "qingqi-app|qingqi-platform|qingqi-ui" # 默认期望无输出
```

若 `qingqi-core` 需要 UI 或 platform，说明边界错了；回主导文档 §5。

---

## 10. P6 抽 `qingqi-feature-*`

目标：每个内置插件成为一个“自带的第三方插件”。

### P6.1 每插件建 crate

对每个 `src/features/<name>/`：

1. 新建 `crates/qingqi-feature-<name>/Cargo.toml`。
2. 移动该插件目录内容到 `crates/qingqi-feature-<name>/src/`。
3. `mod.rs` 改为 `lib.rs` 或由 `lib.rs` 导出模块。
4. 依赖只允许：
   - `qingqi-plugin`
   - `qingqi-ui`
   - `qingqi-platform`
   - 插件自己的第三方库
5. 不依赖 `qingqi-app` / `qingqi-core` / 其它 `qingqi-feature-*`。

### P6.2 插件导出统一形态

每个 feature crate 导出：

```rust
pub fn manifest() -> Manifest;
pub fn databases() -> Vec<DatabaseSpec>;
pub fn build(deps: FeatureDeps) -> anyhow::Result<Box<dyn Plugin>>;
```

其中 `FeatureDeps` 是该 feature crate 自己定义的显式依赖结构，只装它真正需要的东西，例如：

- `Arc<DatabaseService>`
- `AppPaths`
- `Arc<dyn ClipboardContext>`
- `Arc<ThemeStore>` 或 `ThemeHandle`
- `platform` 服务句柄

实际类型以 P2/P5 最终归属为准：

- `Plugin` trait 来自 `qingqi-plugin`。
- `PluginDescriptor` / `BuildCx` 若在 `qingqi-core`，feature crate 不应依赖 core；bin/core 负责用 `manifest()` + `databases()` 组装 descriptor，并从 `BuildCx` 拆出 `FeatureDeps` 再调用 feature crate 的 `build(...)`。
- 若保留 `FeatureRegistry` 在 core，则注册闭包应由 bin crate 写，feature crate 只暴露 `manifest()` / `databases()` / `build(...)`。

### P6.3 ClipboardContext

1. `ClipboardContext` trait 在 `qingqi-plugin`。
2. `qingqi-feature-clipboard` 实现它。
3. bin 注册 clipboard 后拿到 `Arc<dyn ClipboardContext>`，注入 `qingqi-app`。
4. `qingqi-app` 不知道 `ClipboardService` 具体类型。

### P6.4 每插件验收

每迁一个插件就跑：

```powershell
cargo check -p qingqi-feature-<name>
cargo tree -p qingqi-feature-<name> | rg "qingqi-(app|core|feature)" # 期望无输出
```

所有插件迁完：

```powershell
cargo check --workspace
```

---

## 11. P7 抽 `qingqi-app`

目标：GUI 外壳独立，不编译期依赖任何具体插件。

### P7.1 建 crate 并移动 app 层

移动：

```text
runtime.rs
window_controller.rs
launcher.rs
app_index.rs
app_index_store.rs
app_catalog.rs
background.rs
shortcut 服务相关实现
```

不移动已归入 `qingqi-ui` 的 theme/ui/text_input/assets。

### P7.2 run/bootstrap API

最终 API 形态建议：

```rust
pub struct AppHost {
    pub plugins: PluginManager,
    pub build_cx: BuildCx,
    // db / paths / app catalog / events / theme store 等 app 运行所需对象
}

pub fn bootstrap() -> anyhow::Result<AppHost>;
pub fn run(host: AppHost, clipboard: Arc<dyn ClipboardContext>) -> anyhow::Result<()>;
```

要点：

1. `bootstrap()` 创建空 PluginManager 和宿主共享服务。
2. bin 在 `bootstrap()` 后注册内置插件。
3. `run()` 接收注册完的 host 和 `ClipboardContext`。
4. `qingqi-app` 不调用任何 `qingqi_feature_*`。

### P7.3 shortcut 服务落 app

1. `ShortcutDescriptor` 等声明类型已在 SDK。
2. `ShortcutService`、`dispatch_target`、`resolve_shortcuts`、平台热键注册落在 `qingqi-app`。
3. OS 热键调用通过 `qingqi-platform`。

验收：

```powershell
cargo check -p qingqi-app
cargo tree -p qingqi-app | rg "qingqi-feature" # 期望无输出
```

并做空插件集启动验证：构造空 `PluginManager` 时 app 能启动到启动器外壳。

---

## 12. P8 bin 收尾

目标：`qingqi` 只做组合根、资源和打包。

### P8.1 bin 内容

`crates/qingqi/src/main.rs` 只保留：

1. tracing / panic hook 的最外层入口，如果必须在最外层。
2. 调 `qingqi_app::bootstrap()`。
3. 注册所有 `qingqi-feature-*`。
4. 获取 clipboard context。
5. 调 `qingqi_app::run(host, clipboard)`。

内置插件注册可以放在 `crates/qingqi/src/builtin.rs`，但只属于 bin。

### P8.2 注册顺序

1. 创建 registry。
2. 对每个 feature 添加 descriptor + builder。
3. registry build_all：先注册 database schema，再构造插件，再注册到 manager。
4. clipboard 插件额外回传 `Arc<dyn ClipboardContext>`。

禁止 `qingqi-app` 或 `qingqi-core` 暴露 `register_builtin_plugins`。

### P8.3 最终验收

```powershell
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo tree -p qingqi-app | rg "qingqi-feature"      # 期望无输出
cargo tree -p qingqi-core | rg "qingqi-feature"     # 期望无输出
cargo tree -p qingqi-plugin | rg "qingqi-(core|app|platform|feature)" # 期望无输出
```

可选手测：

1. 启动 app。
2. 唤起 launcher。
3. 搜索 app。
4. 打开 JSON / Clipboard / Quick Launch / API Debugger 中至少三个不同形态插件。
5. 关闭窗口并再次打开，确认生命周期正常。

---

## 13. 推荐提交切分

每个阶段至少一个提交；P2/P3/P6 过大时继续拆：

1. `docs: align workspace split authority`
2. `chore: create workspace skeleton`
3. `refactor(plugin): extract qingqi-plugin sdk`
4. `refactor(ui): extract qingqi-ui`
5. `refactor(platform): extract qingqi-platform`
6. `refactor(core): extract qingqi-core host`
7. `refactor(features): extract feature crates batch 1`
8. `refactor(features): extract feature crates batch 2`
9. `refactor(app): extract qingqi-app shell`
10. `refactor(bin): move builtin composition to qingqi`

提交前不要 stage unrelated dirty files。

---

## 14. 常见卡点与处理

| 卡点 | 正确处理 |
|---|---|
| `qingqi-app` 需要 clipboard 具体 service | 抽 `ClipboardContext` 到 SDK，由 clipboard 插件实现，bin 注入 trait object |
| feature 需要 theme/ui/text_input | 依赖 `qingqi-ui`，不要依赖 app |
| platform 需要 assets resolve | 调用方先用 `qingqi-ui::assets` resolve，platform 接收绝对路径 |
| core shortcut 调 platform hotkey | shortcut 声明在 SDK，注册/派发服务移到 app |
| BuildCx 不够用 | 扩展 BuildCx 或共享服务表，不用闭包捕获宿主内部对象 |
| circular dependency | 回主导文档 §5 拆职责；不要加 re-export 壳 |
| 插件之间想共享 helper | 下沉 `qingqi-ui::components` 或 `qingqi-plugin`，不得 feature 互依 |
| 某旧 API 大量报错 | 先判断是否是双机制；若是，删除旧 API 并一次性迁移调用点 |

---

## 15. 给 GPT-5.4 的启动提示模板

```text
你在 F:\develop\qingqi 工作。请严格以 docs/workspace-split-guide.md 为主导文档，
并按 docs/gpt-5.4-workspace-split-execution-plan.md 执行。

本轮只做 P<编号>：<阶段名>。
要求：
1. 开工先 git status --short，并读主导文档相关章节。
2. 不做兼容层、不做过渡壳、不新增双机制。
3. 不触碰与本阶段无关的代码和用户已有改动。
4. 完成后运行本阶段验收命令。
5. 输出变更摘要、验证结果、剩余风险。
```
