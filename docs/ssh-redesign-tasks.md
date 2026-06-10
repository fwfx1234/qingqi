# SSH插件重新设计 - 执行任务清单

## 阶段0：准备工作（1天）

### T0.1 创建新插件骨架
- [ ] 创建 `crates/qingqi-feature-ssh/` 目录
- [ ] 创建 `Cargo.toml`，添加依赖：
  - russh, russh-sftp
  - alacritty_terminal
  - tokio, anyhow, uuid, serde
  - qingqi-plugin, qingqi-ui, gpui
- [ ] 创建基础文件结构（空文件）
- [ ] 在 workspace `Cargo.toml` 中注册新 crate

### T0.2 设置数据库表
- [ ] 在 `manifest.rs` 定义 PLUGIN_ID = "ssh"
- [ ] 设计 profiles 表 schema（兼容旧插件）
- [ ] 在 `lib.rs` 导出 `databases()` 函数

---

## 阶段1：模型与存储层（2天）

### T1.1 领域模型 (model.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/model.rs`

**任务：**
- [ ] 定义 `Profile` 结构体（id, name, host, port, username, auth, paths）
- [ ] 定义 `AuthConfig` 和 `AuthMethod` 枚举（Password | PrivateKey | Agent）
- [ ] 定义 `PathConfig`（remote_root, local_root）
- [ ] 定义 `SessionId` newtype（包装 Uuid）
- [ ] 定义 `SessionSummary` 和 `SessionStatus` 枚举
- [ ] 定义 `RemoteEntry`（path, name, is_dir, size, modified_at）
- [ ] 定义 `TransferId` 和 `TransferTask`（含 logs: Vec<String>）
- [ ] 定义 `TransferDirection` 和 `TransferStatus` 枚举
- [ ] 为所有类型实现 `Clone`, `Debug`
- [ ] **验证：** 无 GPUI 依赖，`cargo check -p qingqi-feature-ssh` 通过

### T1.2 数据库存储 (store.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/store.rs`

**任务：**
- [ ] 实现 `ProfileStore` 结构体
- [ ] 实现 `create_tables()` - 创建 profiles 表
- [ ] 实现 `list_profiles()` -> `Result<Vec<Profile>>`
- [ ] 实现 `get_profile(id)` -> `Result<Option<Profile>>`
- [ ] 实现 `create_profile(draft)` -> `Result<Profile>`
- [ ] 实现 `update_profile(id, draft)` -> `Result<Option<Profile>>`
- [ ] 实现 `delete_profile(id)` -> `Result<bool>`
- [ ] 添加索引：`CREATE INDEX idx_profiles_name ON profiles(name)`
- [ ] **验证：** 无 GPUI 依赖，写单元测试

---

## 阶段2：服务层（4天）

### T2.1 SSH连接池 (connection.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/connection.rs`

**任务：**
- [ ] 定义 `SshConnection` trait（抽象 russh 连接）
- [ ] 实现 `ConnectionPool` 结构体（HashMap<i64, Box<dyn SshConnection>>）
- [ ] 实现 `connect(profile)` -> `Result<()>` （异步连接）
- [ ] 实现 `get(profile_id)` -> `Option<&dyn SshConnection>`
- [ ] 实现 `remove(profile_id)` -> `Option<Box<dyn SshConnection>>`
- [ ] 实现 `close_all()`
- [ ] 处理重连逻辑
- [ ] **验证：** 可以连接测试 SSH 服务器

### T2.2 SFTP客户端 (sftp.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/sftp.rs`

**任务：**
- [ ] 实现 `SftpClient` 结构体（包装 russh-sftp）
- [ ] 实现 `list_directory(path)` -> `Result<Vec<RemoteEntry>>`
- [ ] 实现 `create_directory(path)` -> `Result<()>`
- [ ] 实现 `rename_entry(old, new)` -> `Result<()>`
- [ ] 实现 `remove_file(path)` -> `Result<()>`
- [ ] 实现 `remove_directory(path)` -> `Result<()>`
- [ ] 实现 `upload_file(local, remote, progress_cb)` -> `Result<()>`
- [ ] 实现 `download_file(remote, local, progress_cb)` -> `Result<()>`
- [ ] **验证：** 可以列出、上传、下载文件

### T2.3 终端引擎 (terminal.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/terminal.rs`

**任务：**
- [ ] 实现 `TerminalEngine` 结构体（包装 alacritty_terminal）
- [ ] 实现 `TerminalFrame` 结构体（快照数据）
- [ ] 实现 `TerminalInput` 枚举（Key | Paste | Resize）
- [ ] 实现 `start_pty(ssh_channel)` - 启动伪终端
- [ ] 实现 `write_input(bytes)` - 写入用户输入
- [ ] 实现 `read_output()` - 读取终端输出
- [ ] 实现 `snapshot()` -> `TerminalFrame` - 生成渲染快照
- [ ] 处理窗口大小调整
- [ ] **验证：** 可以在终端中执行命令

### T2.4 传输队列 (transfer.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/transfer.rs`

**任务：**
- [ ] 实现 `TransferQueue` 结构体
- [ ] 实现 `enqueue(task)` - 加入队列
- [ ] 实现并发限制（每个 session 最多 3 个并发）
- [ ] 实现 `start_worker()` - 后台工作线程
- [ ] 实现 `cancel(transfer_id)` - 取消传输
- [ ] 实现 `snapshot()` -> `Vec<TransferTask>` - 获取所有任务
- [ ] **关键：实现详细日志记录**
  - 排队时记录："加入传输队列"
  - 开始时记录："开始上传/下载 {文件名}"
  - 每秒记录进度："已传输 {size} ({percent}%)"
  - 完成时记录："上传/下载完成，耗时 {duration}"
  - 失败时记录："失败: {error}"
- [ ] **验证：** 上传文件并查看日志，每秒至少一条

### T2.5 核心服务 (service.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/service.rs`

**任务：**
- [ ] 定义 `SshService` 结构体
  - database: Arc<DatabaseService>
  - cache_dir: PathBuf
  - connection_pool: Arc<ConnectionPool>
  - sessions: Arc<Mutex<HashMap<SessionId, SessionState>>>
  - event_bus: EventBus
  - revision: Arc<AtomicU64>
- [ ] 定义 `SessionState` 内部结构
- [ ] 定义 `SshEvent` 枚举（SessionConnected, SessionChanged, TransfersChanged 等）
- [ ] 实现 Profile 管理方法（list, create, update, delete）
- [ ] 实现 Session 管理方法（open, close, summaries, snapshot）
- [ ] 实现终端操作方法（terminal_frame, send_input）
- [ ] 实现 SFTP 文件操作方法（list_directory, enter_directory, create_directory 等）
- [ ] 实现传输操作方法（upload_file, download_entry, all_transfer_snapshots, cancel_transfer）
- [ ] 实现事件发布（subscribe_events, emit_event）
- [ ] **验证：** 无 GPUI 依赖，所有公开方法返回纯数据

---

## 阶段3：视图层（4天）

### T3.1 主视图结构 (view/mod.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/view/mod.rs`

**任务：**
- [ ] 定义 `SshView` 结构体
  - service: Arc<SshService>
  - focus_handle: FocusHandle
  - vm: SshViewModel
  - selected_profile_id, selected_session_id, selected_remote_path
  - overlays: profile_editor, settings_dialog, context_menu
  - event_task: Option<Task<()>>
- [ ] 定义完整的 `SshViewModel` 结构体
  - profiles: Vec<ProfileItem>
  - sessions: Vec<SessionTabItem>
  - file_tree: FileTreeViewModel
  - terminal: TerminalViewModel
  - transfers: TransferPanelViewModel
- [ ] 实现 `new(service, cx)` - 构造函数
- [ ] 实现 `start_event_loop()` - 订阅 service 事件
- [ ] 实现 `on_service_event()` - 处理事件并重建 view-model
- [ ] 实现 `rebuild_view_model()` - 从 service snapshot 构建 vm
- [ ] 实现各个 `rebuild_xxx()` 方法
- [ ] 实现 `Render` trait - **关键：分离式顶栏布局**
- [ ] **验证：** render 无 IO/锁/计算，只读 self.vm

### T3.2 左侧边栏 (view/sidebar.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/view/sidebar.rs`

**任务：**
- [ ] 实现 `render_sidebar()` 函数
  - 顶部标题区（交通灯 + "SSH 连接管理" + 新建按钮）
  - Profile 列表（虚拟化，使用 uniform_list）
  - 底部设置按钮
- [ ] 实现 `render_profile_card()` - 单个 Profile 卡片
  - 协议徽章
  - 名称 + endpoint
  - 连接状态色条（左侧 3px）
  - 连接/切换按钮
- [ ] 实现 `mac_traffic_lights()` - macOS 交通灯
- [ ] 处理事件：双击连接、右键菜单
- [ ] **验证：** Profile 超过 50 个时不卡顿

### T3.3 Session Tab栏 (view/session_tabs.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/view/session_tabs.rs`

**任务：**
- [ ] 实现 `render_session_tabs()` 函数
- [ ] 使用 `gpui-component::TabBar` 组件
- [ ] 实现 `render_session_tab()` - 单个 Tab
  - 标题
  - 状态指示点
  - 关闭按钮
- [ ] 添加 [+] 快速新建按钮
- [ ] 处理事件：切换 Session、关闭 Session
- [ ] **验证：** Tab 切换流畅

### T3.4 文件树面板 (view/file_tree.rs) - 优先级：中
**文件：** `crates/qingqi-feature-ssh/src/view/file_tree.rs`

**任务：**
- [ ] 实现 `render_file_tree()` 函数
- [ ] 顶部工具栏（面包屑 + 上传/刷新/新建文件夹按钮）
- [ ] 文件列表（虚拟化，使用 uniform_list）
- [ ] 实现 `render_file_entry_row()` - 单个文件/文件夹行
  - 图标
  - 名称
  - 大小/类型
- [ ] 实现拖放支持（ExternalPaths）
- [ ] 实现右键菜单
- [ ] 处理事件：单击选中、双击打开、拖放上传
- [ ] **验证：** 超过 100 个文件时不卡顿

### T3.5 终端面板 (view/terminal_pane.rs) - 优先级：中
**文件：** `crates/qingqi-feature-ssh/src/view/terminal_pane.rs`

**任务：**
- [ ] 实现 `render_terminal()` 函数
- [ ] 顶部状态栏（图标 + 路径提示 + 连接状态）
- [ ] 终端内容渲染（基于 TerminalFrame）
  - 逐行渲染文本
  - ANSI 颜色支持
  - 光标显示
- [ ] 实现键盘输入处理（on_key_down）
- [ ] 实现鼠标事件处理（点击、滚动）
- [ ] 实现 track_focus
- [ ] **验证：** 可以执行命令并查看输出

### T3.6 传输面板 (view/transfer_panel.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/view/transfer_panel.rs`

**任务：**
- [ ] 实现 `render_transfer_panel()` 函数
- [ ] 顶部控制栏（统计 + 展开/收起 + 清空按钮）
- [ ] 传输任务列表（虚拟化，使用 uniform_list）
- [ ] 实现 `render_transfer_row()` - 单个传输任务行
  - 展开按钮
  - 方向图标
  - 文件名 + 路径
  - 进度条
  - 状态 + 速度
  - 取消按钮
- [ ] 实现 `render_transfer_logs()` - 展开的详细日志
  - 时间戳 + 日志内容（monospace）
  - 自动滚动到最新
- [ ] 实现展开/收起动画
- [ ] **验证：** 传输时日志每秒至少一条，可展开查看

### T3.7 设置弹窗 (view/settings_dialog.rs) - 优先级：中
**文件：** `crates/qingqi-feature-ssh/src/view/settings_dialog.rs`

**任务：**
- [ ] 实现 `render_profile_editor_overlay()` 函数
- [ ] 半透明背景遮罩
- [ ] 居中弹窗
- [ ] Profile 表单（两列布局）
  - 基本信息（名称、主机、端口、用户名）
  - 认证方式（单选）
  - 认证凭据（条件显示）
  - 路径配置
  - 备注
- [ ] 右侧操作按钮（测试连接、保存、取消）
- [ ] 底部删除按钮（编辑模式）
- [ ] 处理 ESC 关闭
- [ ] **验证：** 表单验证正确

---

## 阶段4：插件装配（1天）

### T4.1 插件实现 (plugin.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/plugin.rs`

**任务：**
- [ ] 定义 `SshPlugin` 结构体
- [ ] 实现 `Plugin` trait
  - `manifest()` - 返回插件元数据
  - `commands()` - 返回空 Vec（无命令）
  - `view()` - 创建并返回 SshView
- [ ] 实现 `new(database, paths)` 构造函数
- [ ] 初始化 SshService
- [ ] **验证：** 插件可以注册和启动

### T4.2 清单文件 (manifest.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/manifest.rs`

**任务：**
- [ ] 定义 `PLUGIN_ID = "ssh"`
- [ ] 定义 `manifest()` 函数返回 `Manifest`
  - id: "ssh"
  - name: "SSH 连接管理"
  - description: "SSH/SFTP 终端与文件传输"
  - version: "0.1.0"
  - accent: PluginAccent::Cyan
- [ ] **验证：** manifest 信息正确显示

### T4.3 导出接口 (lib.rs) - 优先级：高
**文件：** `crates/qingqi-feature-ssh/src/lib.rs`

**任务：**
- [ ] 导出所有公开模块
- [ ] 实现 `databases()` 函数
- [ ] 实现 `build(database, paths)` 函数
- [ ] **验证：** `cargo build -p qingqi-feature-ssh` 成功

---

## 阶段5：集成与测试（2天）

### T5.1 主程序集成
**文件：** `crates/qingqi/src/main.rs`

**任务：**
- [ ] 在 `Cargo.toml` 添加 `qingqi-feature-ssh` 依赖
- [ ] 在 `main.rs` 注册新插件
- [ ] 暂时注释掉旧插件（不删除）
- [ ] **验证：** 程序启动，新插件出现在启动器

### T5.2 功能测试
**测试场景：**

- [ ] **Profile 管理**
  - 创建新 Profile
  - 编辑 Profile
  - 删除 Profile
  - 测试连接

- [ ] **SSH 连接**
  - 双击 Profile 连接
  - 查看连接状态
  - 打开终端
  - 执行命令（ls, cd, pwd）
  - 关闭 Session

- [ ] **多 Session**
  - 同时打开 3 个 Session
  - 切换 Tab
  - 独立操作互不干扰

- [ ] **文件浏览**
  - 列出远程目录
  - 进入子目录
  - 返回上级目录
  - 创建文件夹
  - 重命名文件/文件夹
  - 删除文件/文件夹

- [ ] **文件传输**
  - 上传单个文件
  - 上传多个文件
  - 下载文件
  - 拖放上传
  - 取消传输
  - 查看传输日志（确认每秒至少一条）
  - 展开/收起传输面板

- [ ] **UI 测试**
  - 分离式顶栏布局正确
  - 左侧交通灯显示
  - Profile 列表滚动流畅（创建 100 个测试）
  - 文件列表虚拟化（测试 1000 个文件）
  - 传输日志详细完整

### T5.3 性能测试
**测试项：**

- [ ] Profile 列表 100+ 项，滚动流畅（60fps）
- [ ] 文件列表 1000+ 项，滚动流畅
- [ ] 同时 5 个传输任务，UI 不卡顿
- [ ] 终端快速输出（cat large_file），渲染流畅
- [ ] 内存占用合理（< 200MB 空闲状态）

### T5.4 代码审查清单
**提交前检查：**

- [ ] `cargo fmt --all`
- [ ] `cargo check --workspace`
- [ ] `cargo clippy --workspace --all-targets` 无警告
- [ ] 无 `unwrap()` / `expect()`（测试除外）
- [ ] 无 `lock().unwrap()`
- [ ] 无硬编码颜色 `rgb(0x...)`
- [ ] model/store/service 无 GPUI 依赖
- [ ] 所有列表使用 `uniform_list`
- [ ] render 无 IO/锁/计算
- [ ] 异步操作有 generation guard
- [ ] 命名符合规范（SshView, SshService）

---

## 阶段6：文档与收尾（1天）

### T6.1 用户文档
- [ ] 更新 README.md
- [ ] 添加使用截图
- [ ] 编写快速开始指南

### T6.2 开发者文档
- [ ] 更新架构文档
- [ ] 添加代码注释
- [ ] 编写 CHANGELOG

### T6.3 清理旧代码
- [ ] 确认新插件功能完整
- [ ] 删除旧插件 `qingqi-feature-ftp-sftp-ssh-client`
- [ ] 清理未使用的依赖
- [ ] 提交 git commit

---

## 时间估算

| 阶段 | 工作量 | 说明 |
|------|--------|------|
| 阶段0：准备 | 0.5天 | 创建骨架 |
| 阶段1：模型与存储 | 2天 | 纯数据层 |
| 阶段2：服务层 | 4天 | 核心业务逻辑 |
| 阶段3：视图层 | 4天 | UI 实现 |
| 阶段4：插件装配 | 1天 | 组装与集成 |
| 阶段5：测试 | 2天 | 功能与性能测试 |
| 阶段6：文档 | 0.5天 | 文档与清理 |
| **总计** | **14天** | 约 2-3 周 |

---

## 关键里程碑

- **M1（Day 3）**：模型与存储层完成，可以 CRUD Profile
- **M2（Day 7）**：服务层完成，可以连接 SSH 并上传下载文件
- **M3（Day 11）**：视图层完成，UI 完整可用
- **M4（Day 14）**：测试通过，文档完成，可发布

---

## 优先级说明

- **高优先级**：核心功能，必须实现
- **中优先级**：重要功能，应该实现
- **低优先级**：锦上添花，可以后续迭代

**按优先级执行顺序：**
1. 模型层 → 存储层 → 服务层核心
2. 视图层主结构 → 左侧边栏 → Tab 栏 → 传输面板
3. 文件树 → 终端 → 设置弹窗
4. 测试 → 文档

**交付标准：**
- 所有高优先级任务完成
- 功能测试通过
- 性能测试通过
- 代码审查清单全部 ✅

---

**这份任务清单可以直接作为施工蓝图，分配给团队成员或分阶段实施。**
