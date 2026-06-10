# Qingqi

轻量级 Rust + GPUI 桌面工具集，聚焦常驻启动器与高频工具。

## 快速开始

```bash
# 开发
cargo run

# 构建
cargo build --release
```

## 核心功能

- 🚀 **全局启动器** - 统一搜索命令、插件、应用
- 📋 **剪贴板历史** - 文本/图片/文件历史管理
- 📥 **下载管理** - 多任务下载与状态追踪
- 🔧 **开发工具** - API调试、HTTP抓包、JSON解析
- 🌐 **远程连接** - FTP/SFTP/SSH 客户端
- 🖼️ **图像工具** - 压缩、二维码识别/生成
- ⚙️ **系统设置** - 主题、热键、行为配置

## 技术特点

- **快速响应** - GPUI原生渲染，启动器毫秒级响应
- **插件化架构** - 13个独立插件，清晰的边界
- **本地优先** - SQLite持久化，无云依赖
- **跨平台** - macOS / Linux / Windows

## 系统要求

- Rust 2024 Edition
- macOS 10.15+ / Windows 10+ / Linux (X11/Wayland)

## 目录结构

```
qingqi/
├── crates/              # Workspace（19个crate）
│   ├── qingqi/          # 主程序（bin）
│   ├── qingqi-plugin/   # 插件SDK
│   ├── qingqi-ui/       # UI组件库
│   └── qingqi-feature-* # 功能插件
├── docs/                # 架构与规范文档
└── README.md            # 本文件
```

## 开发

```bash
# 检查
cargo check --workspace

# 测试
cargo test --workspace

# 格式化
cargo fmt --all

# Lint
cargo clippy --workspace --all-targets
```

数据目录：`~/Library/Application Support/qingqi` (macOS)

环境变量：
- `QINGQI_DATA_DIR` - 自定义数据目录
- `RUST_LOG` - 日志级别（默认 `qingqi=info,warn`）

## 贡献

欢迎提交 Issue 和 PR。开发规范见 `.claude/CLAUDE.md`（AI助手会自动读取）。

## 构建发布

项目配置了 GitHub Actions 自动构建：

- 推送 tag（`v0.1.0`）触发自动发布
- 支持 macOS / Linux / Windows 三平台

## 许可证

MIT License

## 相关链接

- [GPUI](https://github.com/zed-industries/zed) - UI框架
- [问题反馈](https://github.com/fwfx1234/qingqi/issues)
