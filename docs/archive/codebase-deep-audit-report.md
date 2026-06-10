# Qingqi 代码深度审计与整改方案

> 日期：2026-06-06  
> 审计基准：当前工作树 `F:\develop\qingqi`，包含未提交改动。  
> 覆盖范围：Rust 源码、Cargo 配置、AGENT/README/docs、插件 UI、插件 SDK、后台服务、存储、平台层。  
> 说明：仓库当前约 171 个 Rust 文件、71,201 行。本文采用“全仓逐文件指标扫描 + 高风险文件行号级审阅 + 分 crate 整改方案”的方式。对 7 万行逐行贴注释不可读，也不利于执行；本报告把每类风险定位到具体文件、行段、函数和可执行整改任务。

## 1. 当前工作树状态

审计时工作区不是干净状态，已有这些未提交改动：

```text
M Cargo.lock
M crates/qingqi-app/src/app/runtime.rs
M crates/qingqi-feature-api-debugger/src/code_gen.rs
M crates/qingqi-feature-api-debugger/src/curl_parser.rs
M crates/qingqi-feature-api-debugger/src/mac_ui.rs
M crates/qingqi-feature-api-debugger/src/service.rs
M crates/qingqi-feature-api-debugger/src/view.rs
M crates/qingqi-feature-ftp-sftp-ssh-client/src/view/mod.rs
M crates/qingqi-feature-http-capture/Cargo.toml
M crates/qingqi-feature-http-capture/src/certificate.rs
M crates/qingqi-feature-http-capture/src/engine.rs
M crates/qingqi-feature-http-capture/src/model.rs
M crates/qingqi-feature-http-capture/src/plugin.rs
M crates/qingqi-feature-http-capture/src/proxy_handler.rs
M crates/qingqi-feature-http-capture/src/view.rs
M crates/qingqi-feature-qr-code/src/view.rs
?? docs/codebase-optimization-report.md
?? docs/codebase-deep-audit-report.md
```

审计结论以当前工作树为准。后续整改前建议先确认这些改动是否属于同一批工作，避免重构时误覆盖他人修改。

## 2. 编译与边界验证

### 2.1 当前验证结果

已运行：

```bash
cargo check --workspace
```

结果：通过。

当前 warning：

- `qingqi-platform`
  - `crates/qingqi-platform/src/tray.rs:43`：`let mut builder` 不需要 `mut`。
  - `crates/qingqi-platform/src/apps.rs:136`：`convert_icon_with_image` 未使用。
  - `crates/qingqi-platform/src/apps/windows.rs:47`：`ShortcutMetadata.icon_index` 未读。
  - `crates/qingqi-platform/src/macos.rs:91`：`prompt_accessibility` 未使用。
- `qingqi-feature-http-capture`
  - `crates/qingqi-feature-http-capture/src/plugin.rs:22`：`mock_engine` 未读。
  - `crates/qingqi-feature-http-capture/src/view.rs:39`、`61-69`：mock 面板相关字段未读。
- future-incompat
  - `russh v0.54.5` 包含未来 Rust 版本会拒绝的代码，建议运行 `cargo report future-incompatibilities --id 1`。

### 2.2 依赖边界

已验证：

```bash
cargo tree -p qingqi-app | rg "qingqi-feature"
cargo tree -p qingqi-core | rg "qingqi-feature"
cargo tree -p qingqi-plugin | rg "qingqi-(core|app|platform|feature)"
```

结果均无不合理输出。当前 workspace 拆分边界整体健康。

## 3. 全仓指标

### 3.1 按 crate 统计

| Crate | 文件数 | 行数 | unwrap | expect | spawn | unsafe |
|---|---:|---:|---:|---:|---:|---:|
| `qingqi-feature-api-debugger` | 16 | 13,558 | 150 | 16 | 17 | 0 |
| `qingqi-feature-ftp-sftp-ssh-client` | 12 | 8,844 | 0 | 12 | 9 | 0 |
| `qingqi-feature-quick-launch` | 9 | 6,694 | 13 | 108 | 22 | 1 |
| `qingqi-app` | 14 | 5,637 | 4 | 32 | 17 | 0 |
| `qingqi-feature-clipboard` | 10 | 5,334 | 31 | 43 | 22 | 0 |
| `qingqi-platform` | 16 | 5,116 | 4 | 23 | 5 | 204 |
| `qingqi-feature-http-capture` | 13 | 5,097 | 69 | 0 | 9 | 0 |
| `qingqi-feature-download-manager` | 7 | 4,161 | 68 | 2 | 1 | 0 |
| `qingqi-ui` | 16 | 3,910 | 0 | 0 | 0 | 0 |
| `qingqi-feature-image-compress` | 5 | 3,234 | 16 | 8 | 8 | 0 |
| `qingqi-plugin` | 16 | 2,396 | 7 | 26 | 0 | 0 |
| `qingqi-feature-system-settings` | 5 | 2,074 | 0 | 8 | 0 | 0 |
| `qingqi-feature-qr-code` | 7 | 1,468 | 0 | 28 | 10 | 0 |
| `qingqi-feature-json-parser` | 5 | 1,319 | 3 | 0 | 2 | 0 |
| `qingqi-core` | 4 | 1,044 | 9 | 0 | 1 | 0 |
| 其它小 crate | 16 | 1,417 | 1 | 1 | 0 | 0 |

### 3.2 最大文件

| 文件 | 行数 | 审计判断 |
|---|---:|---|
| `crates/qingqi-feature-api-debugger/src/view.rs` | 4,955 | 职责过多，必须拆模块 |
| `crates/qingqi-feature-ftp-sftp-ssh-client/src/view/mod.rs` | 3,861 | UI、终端、远程文件、弹层混杂 |
| `crates/qingqi-feature-quick-launch/src/view.rs` | 3,435 | overlay、编辑器、列表、历史混杂 |
| `crates/qingqi-feature-api-debugger/src/service.rs` | 3,430 | 服务、请求、导入、环境、tab 持久化、测试都在一处 |
| `crates/qingqi-feature-image-compress/src/view.rs` | 2,595 | view 与批处理 worker 状态混杂 |
| `crates/qingqi-app/src/app/launcher.rs` | 1,863 | 启动器交互复杂，应继续拆查询/渲染/输入处理 |
| `crates/qingqi-feature-http-capture/src/view.rs` | 1,837 | 抓包列表、详情、mock 面板混杂 |
| `crates/qingqi-feature-download-manager/src/view.rs` | 1,787 | 表格、设置 overlay、操作按钮重复 |
| `crates/qingqi-feature-system-settings/src/view.rs` | 1,734 | 多设置 section 可拆 |

### 3.3 高风险模式分布

锁 unwrap：

- `crates/qingqi-feature-image-compress/src/view.rs`：15 处。
- `crates/qingqi-feature-download-manager/src/service.rs`：4 处。
- `crates/qingqi-feature-http-capture/src/mock_engine.rs`：1 处。
- `crates/qingqi-feature-http-capture/src/engine.rs`：1 处。

后台任务 / spawn：

- `crates/qingqi-feature-quick-launch/src/view.rs`：20。
- `crates/qingqi-feature-clipboard/src/view/mod.rs`：20。
- `crates/qingqi-feature-api-debugger/src/service.rs`：17。
- `crates/qingqi-app/src/app/background.rs`：12。
- `crates/qingqi-feature-qr-code/src/view.rs`：10。
- `crates/qingqi-feature-http-capture/src/view.rs`：8。
- `crates/qingqi-feature-image-compress/src/view.rs`：8。

unsafe：

- `crates/qingqi-platform/src/clipboard.rs`：125。
- `crates/qingqi-platform/src/apps/windows.rs`：31。
- `crates/qingqi-platform/src/low_level_hook.rs`：16。
- `crates/qingqi-platform/src/power.rs`：15。
- `crates/qingqi-platform/src/theme.rs`：10。

## 4. P0 文档必须先修

### 4.1 问题

`AGENT.md` 和 `README.md` 仍停留在拆分前心智模型。

证据：

- `AGENT.md:17` 仍写 `current pre-split codebase`。
- `AGENT.md:28-48` 仍引用 `src/app`、`src/core`、`src/platform`、`src/features`。
- `README.md:25` 仍写平台能力在 `crates/qingqi/src/platform`。
- `README.md:30-34` 仍写 `crates/qingqi` 承载 app/core/features/platform 主体。
- `docs/workspace-split-guide.md:3` 已说明 P0-P8 完成。
- `docs/workspace-split-guide.md:52-57` 已列出现行 crate 职责。

### 4.2 风险

后续 agent 或开发者会按照旧路径规划修改，容易把逻辑塞回 `qingqi` bin，或者误以为 `qingqi-app` 可以直接依赖 feature。

### 4.3 整改方案

任务：`docs: refresh AGENT and README for current workspace`

改法：

1. `AGENT.md` 删除 `pre-split` 叙述。
2. 用现行 crate 边界替换旧 `src/*` 规则：
   - `crates/qingqi-plugin`：SDK trait、Manifest、Command、events、storage、host handles。
   - `crates/qingqi-core`：PluginManager、FeatureRegistry、CommandUsageStore、命令排序。
   - `crates/qingqi-app`：runtime、window_controller、launcher、background、shortcut 服务。
   - `crates/qingqi-ui`：theme/token/components/text_input/assets。
   - `crates/qingqi-platform`：OS APIs。
   - `crates/qingqi-feature-*`：插件实现，不依赖 app/core/其它 feature。
   - `crates/qingqi`：bin 组合根。
3. 加入依赖体检命令：

```bash
cargo tree -p qingqi-app | rg "qingqi-feature"
cargo tree -p qingqi-core | rg "qingqi-feature"
cargo tree -p qingqi-plugin | rg "qingqi-(core|app|platform|feature)"
rg -n "\bqingqi_feature_" crates/qingqi-core crates/qingqi-app
rg -n "\bqingqi_app::|\bqingqi_platform::|\bqingqi_feature_" crates/qingqi-plugin
```

4. 更新验收命令：

```bash
cargo fmt --all
cargo check --workspace
cargo test -p <affected-crate>
cargo test --workspace -j 1 --quiet
cargo clippy --workspace --all-targets
```

5. `README.md` 更新项目结构，不再引用旧 `crates/qingqi/src/*` 主体目录。

验收：

- `rg -n "pre-split|src/app|src/core|src/platform|src/features" AGENT.md README.md` 不应再出现误导性现状描述。
- README 与 `docs/workspace-split-guide.md` 不冲突。

## 5. P1 插件 SDK 与宿主整改

### 5.1 `PluginManager::open` 只做 debug 校验

证据：

- `crates/qingqi-core/src/plugin.rs:386`：`pub fn open(...)`。
- `crates/qingqi-core/src/plugin.rs:396`：`debug_assert_eq!(expected_mode, view.mode(), ...)`。
- `crates/qingqi-plugin/src/plugin.rs:53-87`：`PluginView` 通过 enum 表达 Inline/List/Window。
- `crates/qingqi-plugin/src/plugin.rs:140-155`：`Plugin::open` 返回通用 `PluginView`。

风险：

- release 下 manifest mode 与实际 view mode 不一致不会立即失败。
- 后续 `into_window` / `into_inline` / `into_list` 才报错，错误位置更远。

整改：

```rust
if expected_mode != view.mode() {
    anyhow::bail!(
        "plugin {plugin_id} returned {:?}, expected {:?}",
        view.mode(),
        expected_mode
    );
}
```

测试：

- 在 `qingqi-core` 增加 fake plugin：manifest 为 `Window`，open 返回 `Inline`，断言 `open()` 返回 error。
- 验证 `open_window_view()` 错误信息包含 plugin id 和 mode。

任务粒度：

- `plugin: enforce view mode mismatch at runtime`
- 预计小改动，1 个 PR。

### 5.2 能力/权限边界隐式

证据：

- `crates/qingqi-core/src/registry.rs:42-58`：`BuildCx` 持有 database、paths、events。
- `crates/qingqi-plugin/src/host.rs`：theme/app-index/shortcut handle trait。
- `crates/qingqi/src/features/registry.rs:22-121`：不同插件通过闭包拿到不同依赖。
- `crates/qingqi-plugin/src/plugin.rs:124-137`：`PluginCx` 暴露 events 和 app。

风险：

- 插件能力靠注册闭包约定，不在 manifest 中声明，审计困难。
- 未来外部插件无法清楚知道自己需要哪些能力。
- host handle 注入与权限策略没有统一检查点。

整改：

新增 `PluginCapability`：

```rust
pub enum PluginCapability {
    Database,
    StoragePath,
    Clipboard,
    Shortcut,
    Theme,
    AppIndex,
    Network,
    Shell,
    Background,
    GlobalHotkey,
}
```

Manifest 或 PluginDescriptor 增加：

```rust
pub capabilities: Vec<PluginCapability>
```

注册时：

1. descriptor 声明 capability。
2. registry 校验 capability 与注入依赖是否一致。
3. 启动日志输出插件能力。

测试：

- 缺少 capability 却申请某 host handle 时返回错误。
- capability 声明与 databases 不一致时 build_all 报错。

### 5.3 `FeatureRegistry::build_all` 非事务化

证据：

- `crates/qingqi-core/src/registry.rs:85-94`：循环里先注册数据库，再 build 插件，再 register。
- `crates/qingqi-core/src/registry.rs:15-25`：`PluginSource` 定义了 Builtin/External，但 build_all 未使用 source 驱动策略。

风险：

- 中途失败时已经注册的 database 和 plugin 不会回滚。
- `PluginSource` 目前是半成品字段，容易误导。

整改：

两阶段：

1. validate phase：检查 plugin id 唯一、database key 唯一、capability 完整。
2. prepare phase：注册数据库，build runtime，但暂不插入 PluginManager。
3. commit phase：全部成功后统一 register。

短期最小改：

- build_all 出错时记录已完成 entry。
- 错误信息包含 plugin id、source、database key。
- 若不支持 External，暂时删除或注释 `PluginSource::External` 的用途。

### 5.4 Manifest 与 VisualSpec 重复

证据：

- `crates/qingqi-plugin/src/plugin.rs:194-214`：`Manifest` 含 id/name/icon/mode/window/category/status/background/dynamic。
- `crates/qingqi-plugin/src/plugin_spec.rs:103-111`：`PluginVisualSpec` 重复 icon/category/status/mode/window。
- `crates/qingqi-plugin/src/plugin.rs:300-324`：`command_prefixes` 另行存在。

风险：

- icon/category/status/mode/window 双写后可能漂移。
- background 与 status=Background 双写。
- prefixes 与 command_prefixes 语义不清。

整改：

引入 `ManifestBuilder`：

```rust
Manifest::builder("json-parser")
    .name("JSON 解析")
    .description("格式化、校验和压缩 JSON")
    .icon(IconRef::named("json"))
    .window(WindowSpec::fixed(900.0, 680.0))
    .category(PluginCategory::Tool)
    .accent(PluginAccent::Blue)
    .prefixes(["json"])
    .context_matchers([...])
    .build()
```

并让 visual 从 manifest 派生：

```rust
impl Manifest {
    pub fn visual(&self) -> PluginVisualSpec { ... }
}
```

前缀建议：

- 如果两个集合确实必要，改名：
  - `open_prefixes`
  - `action_prefixes`
- 如果没有必要，保留单一 `prefixes`。

### 5.5 动态命令刷新粗粒度

证据：

- `crates/qingqi-plugin/src/events.rs:8-13`：事件只有 `FeatureChanged / CommandsChanged / JobsChanged`。
- `crates/qingqi-core/src/plugin.rs:147-166`：command cache 只能整体失效。
- `crates/qingqi-core/src/plugin.rs:169-205`：dynamic commands 每次 query 时对 dynamic plugin 调 `commands(plugin_query)`。

风险：

- 事件没有 plugin-level revision 和差分。
- 动态命令 provider 如果 commands() 变重，会影响 launcher。

整改：

1. `Plugin` 增加可选 `commands_revision() -> u64`，默认 0。
2. `CommandsChanged` 带 source 和 revision。
3. `PluginManager` 按 plugin id 缓存 dynamic command snapshot。
4. launcher 收到 CommandsChanged 时只刷新对应 plugin。

### 5.6 动态命令的 panic 隔离还不完整

补充证据：

- `crates/qingqi-core/src/plugin.rs:131` 左右的普通命令路径有 `catch_unwind` 包裹。
- `crates/qingqi-core/src/plugin.rs:169-205` 的 dynamic command 路径直接调用 `plugin.commands(plugin_query)`。
- `crates/qingqi-app/src/app/launcher.rs:187` 依赖 command cache 和增量刷新，命令收集异常会直接影响搜索体验。

风险：

- 只要某个动态命令 provider 在搜索时 panic，launcher 就会把这次查询打断。
- 这种错误不会像普通命令那样被宿主统一隔离，定位会更绕。

整改：

1. 动态命令收集也必须进入 `catch_unwind`。
2. panic 后保留上一次可用快照，不要把整个 cache 清空。
3. 错误日志里记录 `plugin_id`、`query`、`revision`。
4. 若后续允许接口演进，可把 `commands()` 细化成可失败的收集接口或 snapshot 接口。

验收：

- 构造一个 dynamic plugin 在 `commands()` 内 panic，launcher 仍然可继续查询其他插件。
- panic 只影响单个 plugin 的 command snapshot，不影响全局搜索框。

### 5.7 启动 / 关闭生命周期需要更强约束

补充证据：

- `crates/qingqi-plugin/src/plugin.rs:183` 左右的 `start_background()` 目前不返回 `Result`，也没有 started guard。
- `crates/qingqi-plugin/src/plugin.rs:183-185` 只有事件注入，没有 stop token、join handle 或可观测失败路径。
- `crates/qingqi-app/src/app/window_controller.rs:763` 附近的 `view.on_close()` 不是强隔离边界。

风险：

- 插件后台循环可能重复启动，或者窗口反复打开/关闭时留下悬挂任务。
- `on_close()` 和 `shutdown()` 的异常路径不够清晰，容易让插件把宿主关闭流程拖慢。

整改：

1. 把 `start_background` 收敛成可返回 `Result<()>` 的生命周期入口，或者引入 `LifecycleCx`。
2. 每个 runtime/session 持有 `started` 标记、`JoinHandle`、`stop token`。
3. `shutdown()` 明确负责停任务，不把资源回收散落在 view drop 里。
4. `WindowController` / `PluginWindow` 对 `on_close()` 做 panic 隔离和超时日志。

验收：

- 插件窗口连续打开/关闭不会重复启动后台任务。
- 关闭窗口后，后台任务能明确退出，日志里能看到 join/stop 结果。

## 6. P1 UI 组件体系整改

### 6.1 共享组件已有但能力不足

已读文件：

- `crates/qingqi-ui/src/ui/components/button.rs`
- `crates/qingqi-ui/src/ui/components/chip.rs`
- `crates/qingqi-ui/src/ui/components/overlay_host.rs`
- `crates/qingqi-ui/src/ui/components/settings.rs`
- `crates/qingqi-ui/src/ui/components/table_header.rs`
- `crates/qingqi-ui/src/ui/components/status_pill.rs`

当前问题：

- `button()` 固定 32px，没有 size、icon、disabled、loading、active。
- `icon_button()` 只有 size 和 icon，没有 variant、tooltip、active、disabled。
- `chip()` 仍要求 `dark: bool`，与 conventions 中“新代码不要穿 dark bool”冲突。
- `overlay_host()` 只有 centered overlay，没有 menu/sheet/dialog/drawer 类型，也无 Esc 语义。
- `table_header_flex()` 的 grow 逻辑是 `grow >= 2.0` 才 flex，否则固定 96px，表达力不足。
- `status_pill()` 没有可扩展 metadata，比如 icon、compact、semantic status enum 映射。

### 6.2 feature 层重复证据

按钮重复：

- `crates/qingqi-feature-image-compress/src/view.rs:1598-1625`：`primary_button / secondary_button / ghost_button / action_button`。
- `crates/qingqi-feature-quick-launch/src/view.rs:2711-2972`：`primary_action_button / action_button / icon_action_button / destructive_action_button`。
- `crates/qingqi-feature-download-manager/src/view.rs:1651-1700`：`primary_btn / secondary_btn / action_button / action_icon`。
- `crates/qingqi-feature-system-settings/src/view.rs:1600-1676`：`action_button / seg_button`。
- `crates/qingqi-feature-json-parser/src/view.rs:574-654`：`secondary_button / mode_pill / query_execute_button`。

chip/status 重复：

- `crates/qingqi-feature-image-compress/src/view.rs:960`：`mode_chip`。
- `crates/qingqi-feature-download-manager/src/view.rs:1054`：`filter_chip`。
- `crates/qingqi-feature-quick-launch/src/view.rs:3001-3072`：`kind_chip / subtle_chip / status_chip / latest_run_status_chip / segment_button`。
- `crates/qingqi-feature-api-debugger/src/view.rs:4431-4525`：scenario/method/status badge。
- `crates/qingqi-feature-http-capture/src/view.rs:587`：`status_badge`。

overlay 重复：

- `crates/qingqi-feature-api-debugger/src/view.rs:4125`：`overlay_shell`。
- `crates/qingqi-feature-api-debugger/src/view.rs:4159`：`context_menu_overlay`。
- `crates/qingqi-feature-quick-launch/src/view.rs:2666`：`menu_overlay_shell`。
- `crates/qingqi-feature-download-manager/src/view.rs:1523`：`settings_overlay`。
- `crates/qingqi-feature-ftp-sftp-ssh-client/src/view/mod.rs:2366`、`2547`、`2606`、`2693`：多种 overlay。

### 6.3 共享 UI 整改方案

任务 1：`ui: introduce full button family`

接口建议：

```rust
pub enum ButtonVariant { Primary, Secondary, Ghost, Danger }
pub enum ButtonSize { XSmall, Small, Medium }
pub struct ButtonProps {
    pub variant: ButtonVariant,
    pub size: ButtonSize,
    pub icon: Option<IconName>,
    pub disabled: bool,
    pub loading: bool,
    pub active: bool,
}
```

验收：

- disabled 不绑定 on_click，或 on_click 内统一阻断。
- hover/active/loading/disabled 样式完整。
- 替换 image-compress 和 download-manager 的按钮作为样板。

任务 2：`ui: introduce chip segmented status components`

改法：

- `ChipSize`、`ChipTone`、`selected`、`disabled`。
- `SegmentedControl` 接收 items 和 selected key。
- `StatusPill` 支持 domain enum 映射，例如 `TaskStatus -> StatusTone`。

任务 3：`ui: extend overlay host`

类型：

- `Dialog`
- `Sheet`
- `Drawer`
- `ContextMenu`
- `Popover`

需要支持：

- Esc 关闭。
- 点击外部关闭。
- 内容阻止冒泡。
- 固定最大高度和滚动区域。
- 底部 action bar。

任务 4：`ui: add DataTableShell`

能力：

- column spec：fixed/flex/min/max。
- empty/loading/error 状态。
- row action slot。
- 小窗口横向滚动或列收缩策略。

## 7. P1 大型 UI 文件拆分方案

### 7.1 API Debugger view

文件：`crates/qingqi-feature-api-debugger/src/view.rs`，4,955 行。

当前结构证据：

- `30-120`：`OpenTab`，tab identity 与匹配逻辑。
- `126-209`：`KvRow / KvEditor`。
- `209-259`：Auth 表单模型。
- `279-329`：`ApiDebuggerView` 状态字段。
- `332-1606`：view 状态操作、请求发送、环境管理、导入导出、collection 操作。
- `1607-1864`：主 render 和 overlay 分支。
- `1883-2400`：collection tree / tabs / action bar。
- `2415-2810`：editor/auth 面板。
- `2819-3280`：response panel。
- `3294-3910`：环境 popup/dialog。
- `3933-4395`：curl/rename/context menu overlay。
- `4431-4545`：badge/status/style helper。
- `4553-4696`：TextInput 构造、request_at、parse/format。
- `4696-4836`：KV editor table。
- `4858-4949`：测试。

主要问题：

- 状态、IO action、UI helper、overlay、纯函数和测试混在一个文件。
- `selected_request()` / `selected_environment()` 使用 `expect("... should exist")`，如果 collection 变更或异步刷新后 index 失效，会 panic。
- `export_environments()` 在 UI handler 中同步 `std::fs::write`。
- 多个 `show_*` bool 叠加，overlay 状态不是单一来源。
- 当前新增 tab 持久化逻辑更复杂，后续回归风险高。

拆分建议：

```text
view/
  mod.rs                    ApiDebuggerView + Render shell
  tabs.rs                   OpenTab、tab persistence glue
  collection_tree.rs         collection_tree/group_section/request_tree_block
  request_editor.rs          editor_panel、auth_form_panel、kv editor table
  response_panel.rs          response_panel、history、body/code render
  environment.rs             env popup、env manager、import/export env UI
  overlays.rs                overlay_shell、context menu、rename/curl dialogs
  style.rs                   badges、status、method colors
  input.rs                   single_input/multiline_input/kv_input
```

整改顺序：

1. 先移动纯 helper，不改行为。
2. 抽 `ActiveOverlay`：

```rust
enum ActiveOverlay {
    EnvPopup,
    EnvManager,
    CollectionMenu(CollectionMenuState),
    CurlImport,
    Rename,
}
```

3. `selected_*` 改成返回 `Option` 或 `Result`，render 使用 empty/error state。
4. 文件导入导出迁到 background task，完成后通过 notice 更新。
5. 为 open tab 增加状态机测试：新增、切换、关闭、恢复、collection 删除后 fallback。

### 7.2 FTP/SFTP/SSH view

文件：`crates/qingqi-feature-ftp-sftp-ssh-client/src/view/mod.rs`，3,861 行。

问题：

- `FtpSftpSshView` 同时负责连接管理、远程文件列表、profile editor、terminal、transfer、overlay。
- `remote_menu_overlay`、`profile_menu_overlay`、`remote_action_overlay`、`profile_editor_overlay` 分散。
- `glass_panel` 和本地 UI 风格没有进入共享组件。
- 终端 render 与文件浏览混在 view 文件中。

拆分建议：

```text
view/
  mod.rs
  sidebar.rs
  remote_browser.rs
  profile_editor.rs
  terminal_panel.rs
  transfer_panel.rs
  overlays.rs
  chrome.rs
  style.rs
```

整改重点：

- 把 profile editor 独立成状态结构。
- 把 transfer queue 默认收纳为 bottom compact bar。
- terminal 渲染只接收 `TerminalFrame`，不要直接耦合远程 runtime 状态。
- overlay 统一为 `ActiveOverlay`。

### 7.3 Quick Launch view

文件：`crates/qingqi-feature-quick-launch/src/view.rs`，3,435 行。

问题：

- 主 view 同时包含搜索、管理、编辑器、参数输入、history、result、多个 overlay。
- 本地按钮和 chip 最多。
- overlay 通过多个状态字段组合，新增弹层容易互相遮挡。

拆分建议：

```text
view/
  mod.rs
  action_list.rs
  editor.rs
  parameters.rs
  history.rs
  result.rs
  overlays.rs
  components.rs
```

关键整改：

- `ActiveOverlay` 替代互斥 bool。
- `PendingRun`、`EditorState`、`HistoryState` 独立。
- 迁移按钮/chip 到 `qingqi-ui`。

### 7.4 HTTP Capture view

文件：`crates/qingqi-feature-http-capture/src/view.rs`，1,837 行。

当前 check warning 显示 mock 面板字段未读：

- `mock_store`
- `mock_rules`
- `show_mock_panel`
- `mock_edit_name`
- `mock_edit_url_pattern`
- `mock_edit_method`
- `mock_edit_status`
- `mock_edit_body`

判断：

- 当前 dirty diff 似乎新增了 mock 能力，但 UI 字段尚未接上。
- 要么完成 mock 面板 UI，要么先移除未接线字段，避免半成品状态。

拆分建议：

```text
view/
  mod.rs
  capture_table.rs
  detail_tabs.rs
  filter_bar.rs
  mock_panel.rs
  certificate_panel.rs
```

### 7.5 Image Compress view

文件：`crates/qingqi-feature-image-compress/src/view.rs`，2,595 行。

问题：

- view 内有 `SharedBatchResults`、worker 通知、queue state、UI render。
- `shared.inner.lock().unwrap()` 15 处。
- 本地按钮和 chip 重复。

整改：

- 把批处理状态迁到 `batch.rs`。
- 锁获取改为 `lock_or_recover` 或 `if let Ok` + notice。
- UI drain task 抽成 `BatchResultDrain`。
- 迁移按钮/chip/table。

### 7.6 Download Manager view

文件：`crates/qingqi-feature-download-manager/src/view.rs`，1,787 行。

问题：

- 表格、任务行、设置 overlay、格式化函数都在一处。
- `url_input_entity.clone().expect("url input missing")` 在 render 期间依赖 init 顺序。
- 本地 buttons/chips/status。

整改：

- `task_table.rs`
- `settings_overlay.rs`
- `format.rs`
- `toolbar.rs`
- 初始化阶段确保 input entity 是结构不变量，或改 Option 渲染 fallback。

## 8. P2 后台任务与服务整改

### 8.1 API Debugger service

文件：`crates/qingqi-feature-api-debugger/src/service.rs`，3,430 行。

证据：

- `355`、`398`、`425`、`448`、`464`、`489`、`529`、`581`、`591`、`689`、`899`、`969`、`992`、`1031`、`1045`、`1067`、`1082`：大量 `thread::spawn`。
- `1340`、`1604`：同步 `std::fs::read`。
- `1348`：每次请求构建 `reqwest::blocking::Client`。
- `174-180`：`revision`、`generation`、`state: Mutex<ApiServiceState>`。

风险：

- 连续点击导入/创建/发送会创建无界线程。
- 请求取消只能通过 generation 标记，不能取消已经发出的 blocking request。
- 每次请求重建 client，连接池无法复用。
- UI 中新增 `export_environments()` 同步写文件，会阻塞 UI。

整改：

1. 新增 feature executor：

```rust
struct ApiJobExecutor {
    tx: crossbeam_channel::Sender<ApiJob>,
    workers: Vec<JoinHandle<()>>,
}
```

或使用 `tokio::task::spawn_blocking` + bounded semaphore。

2. `ApiService` 持有 reusable client：

```rust
client: reqwest::blocking::Client
```

当 settings/proxy 改变时重建。

3. 所有 async API 统一：

```rust
fn submit_job(&self, kind: ApiJobKind, on_done: ApiJobResultSink)
```

4. `send_request` 返回 request id，取消时标记并丢弃结果。

5. 导入导出环境迁出 view，同样走 service job。

测试：

- 连续 50 次 create/import/send 不创建 50 个 OS thread。
- cancel 后 pending_response 不覆盖新请求。
- client reuse 可通过 mock server 或计数器验证。
- environment import/export roundtrip 已有测试，需要补旧格式导入和非法 version。

### 8.2 Download Manager service

文件：`crates/qingqi-feature-download-manager/src/service.rs`。

证据：

- `40-46`：store、active、settings、client 多个 mutex。
- `78`：`self.client.lock().unwrap()`。
- `356`：每个下载 `thread::spawn`。
- `380`、`406`、`894`：后台线程里 `store.lock().unwrap()`。
- `692`：已实现 `JobProvider`，可作为 long-running job 样板。

风险：

- 多任务下载直接开线程，不受 max_concurrent 真实控制。
- 锁中毒会导致后台 panic。
- 高频 update_progress 每次拿 store 锁，DB 写入压力高。

整改：

1. 建立 bounded worker pool。
2. `active` 从 HashMap + flags 改为 `DownloadJobState`。
3. 进度写入节流，例如 250ms 或每 512KB。
4. 锁获取统一 `lock_or_recover` 或返回 `Result`。
5. JobProvider 与 UI 共享 snapshot，不重复算 active/progress。

测试：

- max_concurrent=2 时同时 active 不超过 2。
- pause/cancel 不 panic。
- store lock poison 测试。
- range resume、retryable status、speed limit 回归。

### 8.3 FTP/SFTP/SSH runtime

文件：

- `crates/qingqi-feature-ftp-sftp-ssh-client/src/runtime.rs`
- `crates/qingqi-feature-ftp-sftp-ssh-client/src/protocols.rs`
- `crates/qingqi-feature-ftp-sftp-ssh-client/src/transfer.rs`

证据：

- `runtime.rs:53`：subscribers 为 `Arc<Mutex<Vec<mpsc::Sender<_>>>>`。
- `runtime.rs:764`：download 开线程。
- `runtime.rs:802`：upload 开线程。
- `runtime.rs:859-991`：terminal 线程内创建 Tokio runtime 并 `block_on`。
- `protocols.rs:100-135`：current_thread runtime + block_on。

风险：

- session 关闭时线程生命周期不透明。
- events emit 对 subscriber Vec 上锁，慢 receiver 可能影响清理。
- terminal 和 transfer 的 shutdown/cancel 不统一。

整改：

- `RemoteRuntime` 增加 runtime supervisor：
  - transfer worker handles。
  - terminal handle。
  - shutdown token。
- 事件总线换 `tokio::sync::broadcast` 或 `crossbeam_channel`。
- `SessionRuntime` Drop 时明确关闭 terminal command channel。
- download/upload task 绑定 transfer id，可取消。

测试：

- close session 后 terminal revision 不再变化。
- 取消传输后状态 terminal。
- 断线后不会继续持有 session lock。

### 8.4 HTTP Capture engine/proxy

文件：

- `crates/qingqi-feature-http-capture/src/engine.rs`
- `crates/qingqi-feature-http-capture/src/proxy_handler.rs`
- `crates/qingqi-feature-http-capture/src/certificate.rs`

证据：

- `engine.rs:68`：`self.ca_manager.lock().unwrap().status()`。
- `engine.rs:117-186`：后台线程内创建 runtime 并启动 proxy。
- `proxy_handler.rs:231`：mock response builder `unwrap()`。
- `certificate.rs` 多处 shell command / filesystem 阻塞。

风险：

- CA lock poison 会 panic。
- proxy stop 没有清晰 join 或 shutdown completion 反馈。
- mock status/header 非法时 response builder unwrap 会 panic。

整改：

- lock unwrap 改错误恢复。
- proxy start 返回 `ProxyHandle { stop_tx, join_handle }`。
- stop 等待 thread join 或设置超时。
- mock response 构建失败时返回 500 fallback 并记录 tracing。
- 证书安装动作异步化，UI 展示 loading/error。

### 8.5 QR Code view

文件：`crates/qingqi-feature-qr-code/src/view.rs`。

证据：

- `36`：`pending_action: Arc<Mutex<Vec<QrBackgroundResult>>>`。
- 多处 `cx.spawn` / background action。
- `472`：`self.input.clone().expect("qr input missing")`。

整改：

- 用 `Entity<QrJobState>` 或单独 `QrTaskQueue` 替代 Arc Mutex Vec。
- input 初始化变为非 Option，或 render fallback。
- 保存/扫描/复制动作统一 result enum。

## 9. P2 存储与 migration

当前多个 feature 自己处理 schema：

- API Debugger：`data_source.rs` 大量 schema 与 migration，unwrap 55。
- Download Manager：`store.rs` unwrap 50。
- Quick Launch：`store.rs` expect 29。
- Clipboard：`history_store.rs` unwrap 23、expect 29。
- HTTP Capture：`store.rs`、`mock_store.rs` 各 unwrap 20。

问题：

- 旧 schema 到新 schema 的测试不统一。
- `ALTER TABLE` 幂等策略分散。
- schema version table 不统一。
- 数据库 key 与 manifest/database spec 的关系不够可视。

整改：

新增 `qingqi-plugin::database::MigrationRunner` 或 `qingqi-core` helper：

```rust
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub run: fn(&rusqlite::Transaction<'_>) -> anyhow::Result<()>,
}
```

能力：

- `schema_version` 表统一。
- transaction 包裹。
- migration name 记录。
- 幂等 column/index helper。
- 旧 schema fixture 测试 helper。

先迁移顺序：

1. Download Manager。
2. Quick Launch。
3. HTTP Capture。
4. API Debugger。
5. Clipboard。

## 10. P2 平台层 unsafe 整改

### 10.1 tray static mut

证据：

- `crates/qingqi-platform/src/tray.rs:164`：`static mut CURRENT_TRAY: Option<TrayIcon> = None;`
- `tray.rs:168-176`：replace/with_tray 使用 unsafe。

风险：

- 注释说主线程访问，但类型系统没有保证。

整改：

```rust
static CURRENT_TRAY: OnceLock<Mutex<Option<TrayIcon>>> = OnceLock::new();
```

或把 TrayIcon 存入 app-level `BackgroundSupervisor`，由主线程 owner 管理。

### 10.2 clipboard unsafe 密度高

证据：

- `crates/qingqi-platform/src/clipboard.rs` 有 125 处 unsafe。
- Windows clipboard、macOS Objective-C runtime、NSData/NSArray 转换都在同一文件。

整改：

拆分：

```text
clipboard/
  mod.rs
  windows.rs
  macos.rs
  unsupported.rs
```

要求：

- 每个 unsafe fn 写 SAFETY 条件。
- FFI 指针转换集中封装。
- Windows GlobalLock/Unlock 使用 RAII guard。
- macOS Objective-C msg_send wrapper 限定返回类型。

### 10.3 power/theme observer

证据：

- `power.rs:393`：`panic!("IOPSNotificationCreateRunLoopSource returned NULL")`。
- `theme.rs:198`、`power.rs:378`：unsafe impl Send。

整改：

- panic 改 Result。
- unsafe impl Send 添加 SAFETY 注释，说明 CFRunLoopSource 指针跨线程策略。
- Drop 中失败记录 warning。

### 10.4 low_level_hook

证据：

- `low_level_hook.rs:287`：Windows hook callback。
- `low_level_hook.rs:301`：raw ctx ptr deref。

整改：

- 保留现结构，但补充 callback 生命周期测试/文档。
- Drop 时先 PostThreadMessage，再 join，当前若 message pump 异常需可观测日志。

## 11. P3 Cargo 依赖治理

当前 crate-local 版本：

- `crates/qingqi-feature-api-debugger/Cargo.toml`：`serde_yaml = "0.9.34"`。
- `crates/qingqi-feature-download-manager/Cargo.toml`：`urlencoding = "2.1.3"`。
- `crates/qingqi-feature-http-capture/Cargo.toml`：
  - `http-body-util = "0.1"`
  - `hyper = { version = "1", ... }`
  - `hyper-util = { version = "0.1", ... }`
  - `rustls = { version = "0.23", ... }`
  - `tokio-rustls = "0.26"`

整改：

- 把跨 crate 或核心网络/TLS 依赖上收到根 `Cargo.toml [workspace.dependencies]`。
- `tokio-rustls` 根已经有 workspace 版本，HTTP Capture 应改 `tokio-rustls.workspace = true`。
- 调整后运行：

```bash
cargo tree -d
cargo check --workspace
```

## 12. 分阶段整改路线

### 阶段 0：确认当前 dirty worktree

目标：避免误覆盖。

动作：

1. `git status --short`
2. 确认 16 个 modified 文件归属。
3. 如果这些是未完成 feature，先让作者完成或 stash 到独立分支。

### 阶段 1：文档校准

任务：

- 更新 `AGENT.md`。
- 更新 `README.md`。
- 在 AGENT 加入依赖体检、测试命令、多 agent/dirty worktree 规则。

验收：

```bash
rg -n "pre-split|src/app|src/core|src/platform|src/features" AGENT.md README.md
```

### 阶段 2：低风险 warning 清理

任务：

- `tray.rs:43` 删除 mut。
- `apps.rs:136` 删除或接回 `convert_icon_with_image`。
- `apps/windows.rs:47` 使用或删除 `icon_index`。
- `macos.rs:91` 删除或实现 `prompt_accessibility`。
- HTTP Capture mock 字段接线或删除。

验收：

```bash
cargo check --workspace
```

### 阶段 3：插件 SDK 安全边界

任务：

- `PluginManager::open()` mode mismatch 运行时报错。
- ManifestBuilder。
- 前缀命名统一。
- capability 声明草案。

验收：

```bash
cargo test -p qingqi-core
cargo test -p qingqi-plugin
cargo check --workspace
```

### 阶段 4：共享 UI 基建

任务：

- Button/IconButton。
- Chip/SegmentedControl/StatusPill。
- OverlayHost variants。
- DataTableShell。

样板迁移：

1. Image Compress。
2. Download Manager。
3. QR Code。

验收：

```bash
cargo check -p qingqi-ui
cargo check -p qingqi-feature-image-compress
cargo check -p qingqi-feature-download-manager
```

### 阶段 5：大型 view 拆分

顺序：

1. Quick Launch overlay/editor。
2. API Debugger collection/editor/response/environment。
3. FTP view sidebar/browser/terminal/overlay。
4. HTTP Capture detail/mock/capture table。

原则：

- 先移动代码，不改行为。
- 每次只拆一个区域。
- 拆出纯函数后立即加测试。

### 阶段 6：后台任务模型

顺序：

1. Download Manager worker pool。
2. API Debugger job executor。
3. FTP runtime supervisor。
4. HTTP Capture proxy handle。
5. QR/Image background result state。

验收：

```bash
cargo test -p qingqi-feature-download-manager
cargo test -p qingqi-feature-api-debugger
cargo test -p qingqi-feature-ftp-sftp-ssh-client
cargo test -p qingqi-feature-http-capture
```

### 阶段 7：存储 migration helper

顺序：

1. helper + docs。
2. Download Manager 迁移。
3. Quick Launch 迁移。
4. HTTP Capture 迁移。
5. API Debugger / Clipboard 迁移。

验收：

- 每个 store 都有 old schema fixture。
- `cargo test --workspace -j 1 --quiet`。

## 13. 可直接派工的任务列表

### 文档任务

1. `docs: update AGENT for current workspace boundaries`
2. `docs: update README crate structure`
3. `docs: mark GPT-5.4 split plan as historical execution reference`

### 插件 SDK 任务

4. `plugin: turn view mode debug assertion into runtime error`
5. `plugin: add manifest builder`
6. `plugin: clarify command prefixes`
7. `plugin: draft capability declarations`
8. `core: make feature registry build diagnostics explicit`

### UI 任务

9. `ui: add complete button and icon button components`
10. `ui: add chip, segmented control, and status pill variants`
11. `ui: expand overlay host for dialog, sheet, drawer, menu`
12. `ui: add data table shell`
13. `image-compress: migrate buttons/chips/table to shared UI`
14. `download-manager: migrate table/settings/buttons to shared UI`
15. `quick-launch: replace local overlay shells with ActiveOverlay`

### 服务任务

16. `download-manager: replace per-download threads with bounded workers`
17. `download-manager: remove lock unwraps and throttle progress persistence`
18. `api-debugger: introduce bounded job executor`
19. `api-debugger: reuse HTTP client and move file IO off UI`
20. `ftp: add runtime supervisor and shutdown tokens`
21. `http-capture: make proxy lifecycle joinable`

### 平台任务

22. `platform: replace tray static mut with owned tray state`
23. `platform: split clipboard by OS and document unsafe invariants`
24. `platform: make power/theme observer failures return Result`
25. `platform: clean current cargo check warnings`

### 存储任务

26. `database: add migration runner helper`
27. `download-manager: migrate store to migration runner`
28. `quick-launch: migrate store to migration runner`
29. `http-capture: migrate capture/mock stores to migration runner`

## 14. 最终验收清单

每个阶段至少跑：

```bash
cargo fmt --all
cargo check --workspace
```

跨 crate 或服务重构后跑：

```bash
cargo test --workspace -j 1 --quiet
cargo clippy --workspace --all-targets
```

架构边界检查：

```bash
cargo tree -p qingqi-app | rg "qingqi-feature"
cargo tree -p qingqi-core | rg "qingqi-feature"
cargo tree -p qingqi-plugin | rg "qingqi-(core|app|platform|feature)"
```

future incompat：

```bash
cargo report future-incompatibilities --id 1
```

UI 手动验证：

- 启动器打开/关闭。
- 每个插件窗口打开、重开、关闭。
- 暗色/亮色主题。
- 小窗口/中窗口/宽窗口。
- overlay Esc、点击外部、取消按钮。
- disabled 按钮不可点击。
- 空态、加载态、错误态、权限态。

## 15. 总结

当前 Qingqi 的 crate 边界已经健康，主要风险不在 workspace 拆分，而在四个方向：

1. 文档滞后，会误导后续 agent 和开发者。
2. UI 组件库能力不足，导致插件 view 继续复制控件和 overlay。
3. 大型 view 文件职责过多，必须按区域拆分。
4. 后台任务与平台 unsafe 需要更强生命周期、错误处理和测试约束。

推荐先做 P0 文档与 warning 清理，再做插件 SDK 的运行时约束和共享 UI 基建。这样后续拆大 view 和服务重构时，代码边界会清晰很多，也更适合多 agent 分工。
