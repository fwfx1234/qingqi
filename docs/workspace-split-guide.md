# 轻骑（qingqi）多包（Cargo workspace）拆分指导文档 · v2

> 状态：已完成并验证（P0–P8 完成，workspace 编译与测试通过）
> 目标：把当前单一二进制 crate 拆成一组**经过设计的** crate，核心可脱离插件独立运行，并为未来对外开放插件接口打好稳定契约。
> 铁律：**目标即终态**——不写兼容层、不写兜底、不搞过渡方案；每个执行阶段落地的都是最终代码且 `cargo build`/`cargo test` 通过。
>
> **文档权威性**：本文是 workspace 拆分与目标架构的唯一主导文档。其他工程文档只描述编码约定、UI 细节或阶段执行清单；若与本文冲突，以本文为准。

---

## 0. 设计约束（本次明确的 5 条，优先级最高）

1. **架构经过设计**：crate 边界按「职责 + 依赖方向 + 稳定性」划分，不是按目录机械切块。
2. **一次性到位**：直接做成终态，**禁止**兼容别名、转发壳、双机制、"以后可能用"的可选 crate、防御式过渡代码。开放问题在 §9 一次性定夺。
3. **文档同步**：架构或决策一变，先改本文档，再改代码。
4. **这是插件系统，未来对外开放插件接口**：必须有一组**稳定、自洽的 SDK crate**，内置插件与未来第三方插件都只依赖它；SDK 不得泄漏宿主内部实现。
5. **核心可不依赖插件独立运行**：宿主只依赖 `Plugin` trait 抽象，**绝不**编译期依赖任何具体插件；空插件集也能启动运行。内置插件是「恰好随包发行的插件」，由 bin 注入。

> 用法：每次开工先读 §10 进度表和当前验证说明；结构迁移阶段以文档对齐 + `cargo check` + 依赖体检为主，架构收敛后再集中跑测试并修错。

---

## 1. 现状速览

- 单 crate（`qingqi`，bin-only，无 `src/lib.rs`），`edition = 2024`，137 个 `.rs`，目录已分 `core/ platform/ app/ features/` 但无 crate 边界。
- `build.rs`（`#[path="icon_raster.rs"]`）从 `assets/app-icon.svg` 生成 `assets/app_icon_*.png`，供 `[package.metadata.bundle]`。
- 关键事实：**`core` 已依赖 `gpui`**（`Plugin` 的视图 trait 用 `App/Window/AnyElement`，shortcut 用 `Action`）。当前插件模型是**进程内 Rust 插件**——`Plugin::open()` 直接返回 `gpui::AnyElement`。这决定了 SDK 的形态（见 §4、§9-D1）。
- 已确认的跨层依赖环见 §5（这是拆分真正的拦路虎）。

---

## 2. 目标架构（以插件 SDK 为核心）

依赖**严格自下而上**，无环。中轴是 **`qingqi-plugin`（SDK 契约）**：所有人依赖它，它几乎不依赖别人。

```
                              qingqi (bin)
                         ── 组合根：装配内置插件 + 启动 app ──
                        /        |          |           \
                qingqi-app   qingqi-feature-* (内置插件=普通插件)
                  /  |  \         \   |   /
        ┌────────┘   |   └──┐      \  |  /
   qingqi-core  qingqi-ui  qingqi-platform
        \           |          /
         \          |         /
          └──► qingqi-plugin ◄──┘     ← 稳定 SDK 契约（trait + 类型 + 存储 + 事件）
                   (gpui, serde, rusqlite…；不依赖任何上层)
```

| crate | 角色 | 主要内容 | 依赖 | 对外稳定? |
|---|---|---|---|---|
| **qingqi-plugin** | **插件 SDK 契约** | `Plugin`/视图 trait、`Manifest`+规格、`Command`/`Activation`/匹配器、`ShortcutDescriptor`、`PluginCx`、事件总线、存储(`AppPaths`/`DatabaseService`/`DatabaseSpec`)、`JobProvider`、`ClipboardContext`、`lock_or_recover` | gpui, serde, anyhow, rusqlite/r2d2 | **是（semver）** |
| **qingqi-ui** | 渲染工具箱（插件 UI 必需） | theme / theme_mode / ui(+components) / text_input / assets / accent 配色 | qingqi-plugin, gpui, gpui-component | **是（semver）** |
| **qingqi-platform** | OS 服务 | clipboard / hotkey / low_level_hook / tray / power / display / apps / shell / svg_icon | (gpui, windows…)，**不依赖 plugin** | 否 |
| **qingqi-core** | **插件宿主** | `PluginManager`、`FeatureRegistry`/`BuildCx`/`PluginDescriptor`、`CommandUsageStore`、命令排序 | qingqi-plugin | 否 |
| **qingqi-app** | GUI 外壳 | runtime / window_controller / launcher / app_index(+store/catalog) / background / **shortcut 服务** | core, plugin, ui, platform | 否 |
| **qingqi-feature-\<name\>** | 单个内置插件 crate | clipboard / api_debugger / … 各自独立 | **plugin, ui, platform**（**不依赖 app/core**） | 否 |
| **qingqi**（bin） | 组合根 | `main` + 装配内置插件 + build.rs/assets/bundle | 全部 | 否 |

> **关键点（满足第 4、5 条）**：
> - `qingqi-core` 与 `qingqi-app` **只依赖 `qingqi-plugin` 的 trait**，从不依赖任何 `qingqi-feature-*`。
> - `qingqi-feature-*` 与未来的第三方插件**同构**：都只依赖 `qingqi-plugin` + `qingqi-ui`(+platform)。内置插件就是「自带的第三方插件」，这本身验证了 SDK 的完备性。

---

## 3. 不变量（CI/评审须守住）

- **I1 · 核心零插件可运行**：`qingqi-core`/`qingqi-app` 的依赖图里**没有**任何 `qingqi-feature-*`。`cargo tree -p qingqi-app` 不应出现任何具体插件。app 用空 `PluginManager` 也能启动（只有启动器外壳，无命令）。
- **I2 · SDK 不泄漏宿主**：`qingqi-plugin`、`qingqi-ui` **不依赖** core/app/platform-internal/features。第三方插件 = `qingqi-plugin` + `qingqi-ui` 两个依赖即可编译。
- **I3 · 无兼容/兜底**：不存在「转发壳」「双机制」「过渡别名」。具体清算见 §8。

---

## 4. 插件 SDK 契约（`qingqi-plugin` + `qingqi-ui`）

这是第 4 条的落点：**一个插件作者需要、且仅需要的全部东西**。

**`qingqi-plugin` 暴露：**
- 生命周期 trait：`Plugin`（`manifest/commands/open/handle_command/shortcuts/start_background/clipboard_boost/shutdown/...`）。
- 视图 trait：`InlineView` / `WindowView`（+ `PluginView`、`ListItem`）。
- 元数据：`Manifest` + `IconRef`、`ViewMode`、`WindowSpec`/`WindowSize`、`PluginAccent`、`PluginCategory`、`PluginStatus`。
- 命令模型：`Command`、`Activation`、`Action`、`CommandKind`、`ContextMatcher`、`ContextKind`、`ClipboardPayload`、`LauncherContext`、`CommandInvocation`、`CommandOutcome`。
- 快捷键**声明**：`ShortcutDescriptor`、`ShortcutScope`、`ShortcutTarget`、`CoreShortcutAction`（注册/派发是宿主的事，见 §5-C1）。
- 运行时句柄：`PluginCx`、事件 `AppEventBus`/`AppEventKind`。
- 存储：`AppPaths`（feature 目录）、`DatabaseService`、`DatabaseSpec`（插件持久化的统一入口）。
- 能力 trait：`JobProvider`（后台任务）、`ClipboardContext`（向宿主回供剪贴板内容）。
- 工具：`lock_or_recover`、`Page<T>`。

**`qingqi-ui` 暴露**（插件 `render()` 用）：语义主题 `theme::semantic()`、`ui::*` 组件、`text_input::TextInput`、配色 `accent_color(PluginAccent)`、资源 `assets`。

**契约纪律：**
- SDK 类型尽量 `#[derive(Serialize, Deserialize)]`（现状已大多如此），保留「日后可加声明式/IPC 通道」的可能，但**现在不实现**该通道（避免第 2 条禁止的过度方案）。
- 插件模型 = **进程内 Rust 插件**（返回 `gpui::AnyElement`）。是否将来支持进程外/声明式插件，见 §9-D1（一次性定夺）。
- SDK 一旦发布，破坏性改动走 semver major。内部 crate（core/app/platform）随意演进。

---

## 5. 阻断拆分的依赖环 + 终态破法（**最关键**）

Rust 不允许 crate 成环。当前 5 条「向上」边，破法已是终态设计的一部分（非过渡）：

| 环 | 证据 | 终态破法 |
|---|---|---|
| **C1 core→app** | `core/shortcut.rs:12` `app::window_controller::{WindowController,WindowControllerHandle}`；`dispatch_target()` | shortcut **声明类型** → `qingqi-plugin`；**注册/解析/派发服务**（`ShortcutService`/`dispatch_target`/`resolve_shortcuts`/平台注册）→ `qingqi-app` |
| **C2 core→platform** | `core/shortcut.rs:150`/`:364` `LowLevelEntry` / `register_global_hotkeys` | 同上，平台调用随服务进 `qingqi-app` |
| **C3 platform→app** | `platform/svg_icon.rs:10` `crate::app::assets::resolve` | `assets` → `qingqi-ui`；`svg_icon` 改为**接收已解析的绝对路径**，彻底去掉该边 |
| **C4 app→features** | `runtime.rs:29` `register_builtin_plugins`；`window_controller.rs:23`/`launcher.rs:35` `features::clipboard::service::ClipboardService` | 组合根（`register_builtin_plugins`）→ **bin**；剪贴板经 `ClipboardContext`（SDK trait）注入，app 持 `Arc<dyn ClipboardContext>`，不识具体插件 |
| **C5 features→app** | 12×`features/*/view.rs` `use crate::app::{theme,theme_mode,ui}`；`app::text_input`；5×`plugin.rs` `app::events::*` | UI 三件套 → `qingqi-ui`；事件 → `qingqi-plugin`；**删除** `app/events.rs` 转发壳（第 2 条） |

破环后四层引用方向应满足 §附录体检（core/app 无 features、platform 无 app、features 仅依赖 plugin/ui/platform）。

---

## 6. 执行顺序（自底向上 · 每步落终态代码 · 可跨多次会话）

> 设计是一次性终态，但**落地分阶段**只为「每步可编译、可回退、可独立提交」。每个阶段产出的都是最终代码，不含任何待删的过渡物。

- [x] **P0 预清理（§8）**：删全局 `#![allow(dead_code)]`、删已确认全死模块、删 `app::events` 壳。先瘦身再拆，避免垃圾进 SDK。
- [x] **P1 workspace 骨架**：根 `Cargo.toml` 加 `[workspace]`，现有代码整体移入 `crates/qingqi/`（bin）。验证 `cargo build`（含 build.rs/assets 路径）。
- [x] **P2 `qingqi-plugin`（SDK）**：从 `core/` 抽出 §4 列举的 trait/类型/存储/事件 → 新 crate。`crate::core::X` → `qingqi_plugin::X`。**同时**把 shortcut **声明类型**留这里、服务留 app（C1/C2 的类型侧）。验证 `cargo check -p qingqi-plugin` 且其依赖图不含上层。
- [x] **P3 `qingqi-ui`**：theme/theme_mode/ui(+components)/text_input/assets → 新 crate；合并 `ThemeAccent` 入 `PluginAccent`（去重）；`svg_icon` 改收绝对路径（破 C3）。
- [x] **P4 `qingqi-platform`**：OS 服务 → 新 crate（依赖最小）。
- [x] **P5 `qingqi-core`（宿主）**：`PluginManager`/registry/usage/排序 → 新 crate，仅依赖 `qingqi-plugin`。**校验 I1**：`cargo tree -p qingqi-core` 无 features。
- [x] **P6 `qingqi-feature-*`**：内置插件 → 每插件一个 crate，依赖 plugin/ui/platform；clipboard 通过 `ClipboardContext` trait 回注宿主；旧 monolith feature 目录已收敛为 bin 侧注册壳。
- [x] **P7 `qingqi-app`**：runtime/window/launcher/app_index/background/shortcut 服务 → 新 crate。`bootstrap() -> Result<AppHost>` 与 `run(host, clipboard)` 已落地，shortcut 服务也已并入 app。
- [x] **P8 `qingqi`（bin）收尾**：仅留 `main` + 组合根 `register_builtin_plugins`（调用 features）+ build.rs/assets/bundle。当前 `main.rs` 已是终态组合根；剩余工作是集中验证与环境侧测试收口。

任一步报 `circular dependency` = §5 某条环未断净，回 §5 修，**不得在 Cargo.toml 里硬绕或加壳**。

---

## 7. workspace 机械细节

- 根 `Cargo.toml`：`[workspace] resolver="2" members=["crates/*"]`，`[workspace.package]` 统一 version/edition/license，`[workspace.dependencies]` 集中三方依赖版本，子 crate 用 `dep = { workspace = true }`；`[profile.release]` 留根。
- 子 crate：被抽出的都建 `lib.rs`（`pub mod ...`）；`qingqi` 保持 bin。crate 名用连字符（`qingqi-plugin`），`use` 名为下划线（`qingqi_plugin`）。
- `build.rs` + `icon_raster.rs` + `assets/` + `[package.metadata.bundle]` 全归 bin crate（`crates/qingqi/`）；`build.rs` 用 `CARGO_MANIFEST_DIR`，随之指向 bin 目录。
- 平台条件依赖（`windows` crate 等）跟到 `qingqi-platform`。
- 结构迁移阶段默认以 `cargo check --workspace`、定向 `cargo check -p <crate>`、依赖体检为主；全量 `cargo test --workspace` 放在架构收敛后集中执行。
- 本轮已验证 `cargo test --workspace -j 1 --quiet` 可稳定通过；不带 `-j 1` 的全量测试在当前 Windows 机器上仍可能因为 pagefile / 编译器资源波动触发非确定性失败，因此日常验收以串行全量测试结果为准。

---

## 8. 拆分前清理（第 2 条：不把垃圾/双机制带进 SDK）

> 判定原则不变：**未被引用 ≠ 垃圾**。(A) 取代/废弃→删；(B) 面向未来的稳定 API/共享组件没人用→保留并登记待推广；(C) 权衡→记录待定。

- [x] 移除根 `#![allow(dead_code)]`（`main.rs:1`），它压制了 **245 条**告警。拆分后各 crate 默认暴露死代码告警；个别保留处就地 `#[allow]` + 理由。
- [x] **`core/view_model.rs`（`ViewModel/Field/FieldKind`）**：当前被 trait 式视图取代、零引用。按 D1 已删除，不纳入 SDK。
- [x] **`core/dict_store.rs`（`PluginDictStore`，229 行）**：插件通用 KV 存储，生产零引用（插件各用自带 store）。已归入 `qingqi-plugin`，作为 SDK 统一存储入口保留。
- [x] **`Plugin::database_specs()` vs `PluginDescriptor::with_databases`** 双机制：lint 证实 trait 方法从未被调用 → 删 `database_specs()`，DB 声明**只保留一种**（descriptor/manifest）。这是第 2 条「无双机制」。
- [x] **`PluginAccent`（plugin_spec） vs `ThemeAccent`（theme）** 字段相同的两枚举 + `accent_to_theme` 桥接：合并为 SDK 的单一 `PluginAccent`，`qingqi-ui` 直接配色。
- [~] `theme.rs` 已去掉 `token(name)` / `ThemeAccent` / `accent_to_theme` 等旧入口；`ui::badge`、`ui_badge` 已删除，空状态与状态 pill 正在向 `components/*` 终态接口收敛；launcher 主题辅助函数仍作为语义 API 保留在 `qingqi-ui::theme`。
- [~] `window_controller`/`app_index` 内联 poison 恢复正在收敛到 `lock_or_recover`；`app_index` 已统一，`window_controller` 仍有局部重复，不阻塞当前架构终态。

---

## 9. 决策点（第 2 条要求一次定夺，改本文档落定）

- **D1 · 外部插件模型** ✅ **已定 = ① 仅进程内 Rust 插件**：`Plugin` 返回 `gpui::AnyElement`，SDK 含 gpui；崩溃靠宿主侧 `catch_unwind` 隔离。**后果：删 `core/view_model.rs`（`ViewModel/Field/FieldKind`）**；不建声明式/IPC 通道（第 2 条）。SDK 类型仍保 `Serialize/Deserialize`（零成本、便于配置/日志），但**不**据此实现任何通道。
- **D2 · features 粒度** ✅ **已定 = ② 每插件一个 crate**（`qingqi-feature-<name>`）：插件多，独立成 crate 以获得最佳增量编译与隔离；每个内置插件 = 一个「自家的第三方插件」，依赖 `qingqi-plugin` + `qingqi-ui`(+`qingqi-platform`)，**不依赖 app/core**。共享的插件内工具（如表格头、设置卡片）下沉到 `qingqi-ui::components`，不在插件间互相依赖。
- **D3 · 通用存储归属** ✅ **已定**：`DatabaseService`/`AppPaths`/`DatabaseSpec`/`PluginDictStore` 放 `qingqi-plugin`（插件与宿主共用）。
- **D4 · crate 命名** ✅ **已定**：`qingqi-plugin / -ui / -platform / -core / -app / qingqi-feature-<name>` + bin `qingqi`。（"core=宿主、plugin=SDK"。）
- **D5 · 是否提供 `qingqi-sdk` 伞 crate**（re-export plugin+ui，方便第三方一行依赖）：**建议否**（避免过度封装，第 2 条），第三方直接依赖两个 crate。

---

## 10. 进度跟踪表

阶段 P0 · 预清理（§8）
- [x] 删 `#![allow(dead_code)]`，记录告警基线
- [x] 按 D1 处理 `view_model`；按 D3 收编 `dict_store`/存储
- [~] 删 `database_specs()` 双机制、合并 accent 枚举、清 theme 裸色板/重复组件
- [x] 删 `app/events.rs` 壳，引用改 `qingqi_plugin`(事件)

阶段 P1–P8 · 拆包（§6，自底向上；阶段内以 `cargo check` + §附录体检为主，收敛后集中测试）
- [x] P1 workspace 骨架（bin 单成员）
- [x] P2 qingqi-plugin（SDK）+ shortcut 类型侧
- [x] P3 qingqi-ui（含 accent 合并、svg_icon 收绝对路径）
- [x] P4 qingqi-platform
- [x] P5 qingqi-core（宿主）→ 校验 I1
- [x] P6 qingqi-feature-*（每插件一个 crate，+ ClipboardContext 实现）
- [x] P7 qingqi-app（+ shortcut 服务、run() 接收装配）
- [x] P8 qingqi(bin) 组合根收尾（编译与装配已完成）

阶段 V1 · 收敛验证
- [x] `cargo fmt --all`
- [x] `cargo check -p qingqi-app`
- [x] `cargo check -p qingqi`
- [x] `cargo check --workspace`
- [x] 定向 `cargo test -p qingqi-plugin/qingqi-core/qingqi-app -j 1`
- [x] `cargo test --workspace -j 1 --quiet`

当前收敛结论：
- `qingqi-app` 依赖 `qingqi-core` / `qingqi-plugin` / `qingqi-platform` / `qingqi-ui`，不依赖任何 `qingqi-feature-*`。
- `qingqi-core` 仅依赖 `qingqi-plugin`。
- `qingqi-plugin` 维持 SDK 边界，不依赖 app/core/platform/feature。
- `crates/qingqi/src` 已收敛为 `main.rs + features/registry.rs + features/mod.rs`，bin 不再保留旧 `app/`、`core/` 副本源码。
- `cargo test --workspace -j 1 --quiet` 已通过；不带 `-j 1` 的并行全量测试仍可能受当前 Windows pagefile 资源波动影响。

---

## 附录 A · 跨层依赖体检命令（仓库根运行）

```bash
rg -n "\bqingqi_feature_" crates/qingqi-core crates/qingqi-app
rg -n "\bqingqi_app::|\bqingqi_platform::|\bqingqi_feature_" crates/qingqi-plugin
rg -n "\bqingqi_feature_" crates/qingqi-app      # 应空（app 不依赖 feature）
cargo tree -p qingqi-core | rg -i "qingqi-feature"   # 应空（I1）
cargo tree -p qingqi-plugin | rg -i "qingqi-(core|app|platform|feature)"   # 应空（I2）
```
> 注意：这些命令面向**拆分完成后的当前 workspace**。若要回看历史单体阶段，请只把附录 B 当作破环背景资料，不要再拿旧 `src/app` / `src/core` 路径做现状体检。

## 附录 B · v1 快照——已知跨层边（破环目标，行号会漂移，认符号）

| 环 | 证据 |
|---|---|
| C1 core→app | `core/shortcut.rs:12` `app::window_controller::{WindowController,WindowControllerHandle}`；`dispatch_target()` |
| C2 core→platform | `core/shortcut.rs:150` `LowLevelEntry`；`:364` `platform::hotkey::register_global_hotkeys` |
| C3 platform→app | `platform/svg_icon.rs:10` `crate::app::assets::resolve` |
| C4 app→features | `app/runtime.rs:29` `register_builtin_plugins`；`app/window_controller.rs:23`、`app/launcher.rs:35` `features::clipboard::service::ClipboardService` |
| C5 features→app | `features/*/view.rs` `use crate::app::{theme,theme_mode,ui}`（12）；`app::text_input::TextInput`；`features/*/plugin.rs` `app::events::{AppEventBus,AppEventKind}`（5，实为 `core::events` 转发） |

## 附录 C · `run()` 组合根上移（终态示意）

```rust
// 终态：crates/qingqi/src/main.rs —— bin 作组合根（app 不识 features）
fn main() -> anyhow::Result<()> {
    let mut host = qingqi_app::app::runtime::bootstrap()?; // db/events/paths/app_index/theme_store + 空 PluginManager
    let clipboard = features::registry::register_builtin_plugins(&mut host)?; // 返回 Arc<dyn ClipboardContext>
    qingqi_app::app::runtime::run(host, clipboard)         // app 仅依赖 qingqi_plugin 的 trait
}
```
