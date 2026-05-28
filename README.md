# Qingqi

`qingqi` 是 `suishou` 的 Rust + GPUI 重写实验版。首要目标是保留核心体验：应用常驻、启动器唤起内置能力、优先实现剪贴板历史；暂不支持第三方插件加载。

## 当前范围

- Rust 2024 + `gpui = 0.2.2`，macOS 使用 `macos-blade` 后端以避免构建期依赖 Xcode `metal` 命令。
- 内置插件注册表，不扫描第三方插件目录。
- 剪贴板插件后台轻量轮询，历史写入 SQLite。
- 插件窗口按需创建，关闭时丢弃视图状态，释放插件 session。
- 常驻进程只保留核心状态和剪贴板监听，不把历史列表长期放在内存里。

## 运行

```bash
cargo check
cargo run
```

当前机器若没有 Rust 工具链，请先安装 `rustup`。数据默认写到系统数据目录下的 `qingqi/`，也可以用 `QINGQI_DATA_DIR` 覆盖。

如果改回 GPUI 默认 macOS Metal 后端，需要安装完整 Xcode，并确认 `xcrun -sdk macosx metal` 可用；当前项目先选择 `macos-blade` 来降低本地构建门槛。

## 设计取舍

- 第三方插件加载先不做，避免常驻进程引入动态加载、隔离和卸载复杂度。
- 剪贴板监听只比较文本内容，优先把内存占用压低；图片和文件历史会在后续补齐。
- UI 打开时分页读取历史，关闭时释放 `ClipboardPanel` 的列表、搜索和选择状态。
- `PluginManager` 常驻只保存 runtime 工厂；插件窗口持有 session，窗口关闭后 session drop，后台剪贴板服务不持有历史列表。

## 架构文档

- [核心架构规范](docs/core-architecture-spec.md)：后续修改 `app`、`core`、`platform`、事件、后台任务、插件生命周期时的准则。
- [架构调整方案](docs/architecture-adjustment-plan.md)：Rust/GPUI 专家视角的阶段化调整建议。
- [当前架构概览](docs/architecture.md)：现有分层、事件流、长期任务和迁移规则摘要。
