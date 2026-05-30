# Qingqi

`qingqi` 是一个基于 Rust + GPUI 的桌面工具集合，聚焦“常驻启动器 + 内置高频工具”体验。

项目目标：

- 启动快、交互轻，优先保证启动器和核心功能响应速度。
- 内置常用工具（剪贴板、应用启动、下载、SFTP/SSH 等）统一入口。
- 以本地数据和本地执行为主，避免不必要的运行时耦合。

## 核心能力

- 全局启动器：统一搜索命令、插件入口和上下文动作。
- 应用快速启动：本机应用索引、搜索、启动、使用排序。
- 剪贴板历史：文本/图片/文件历史管理、筛选、快捷操作。
- 下载管理：多任务下载、状态追踪与持久化。
- FTP/SFTP/SSH：连接配置、文件传输、终端与日志视图。
- 系统设置：主题、行为、缓存与运行参数管理。

## 技术栈

- Rust 2024
- GPUI `0.2.2`
- SQLite（本地状态与索引持久化）
- 平台能力封装（`src/platform`）

## 项目结构

- `src/app`：应用入口、启动器、主题、窗口与运行时编排。
- `src/core`：命令、插件协议、快捷键、存储和基础抽象。
- `src/features`：各业务功能模块（插件实现）。
- `src/platform`：系统相关能力（应用扫描、剪贴板、shell、tray 等）。
- `assets`：图标与静态资源（`app-icon.svg` / `tray-icon.svg` 为源文件，构建时由 `build.rs` 生成 bundle 所需 PNG）。

## 设计与优化文档

- [架构设计](docs/architecture.md)
- [工程约定](docs/conventions.md)
- [插件界面问题清单与优化方案](docs/plugin-ui-optimization-plan.md)
- [gpui-component 使用指南](docs/gpui-component-guide.md)

## 本地开发

```bash
cargo check
cargo run
```

可选：

```bash
cargo test
```

数据默认写入系统数据目录下的 `qingqi/`，可通过 `QINGQI_DATA_DIR` 覆盖。

## 日志

默认日志级别为 `qingqi=info,warn`。可通过 `RUST_LOG` 调整：

```bash
RUST_LOG="qingqi=trace,warn" cargo run
```

## 自动打包

项目已配置 GitHub Actions 自动打包工作流：`.github/workflows/release.yml`

- 触发方式：
  - 推送 tag（如 `v0.1.0`）
  - 手动触发 `workflow_dispatch`
- 构建目标：
  - Linux `x86_64-unknown-linux-gnu`
  - macOS `aarch64-apple-darwin`
  - Windows `x86_64-pc-windows-msvc`
- 产物：
  - Workflow Artifacts
  - Tag 场景下自动上传到 GitHub Release
