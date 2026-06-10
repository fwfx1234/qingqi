
一、现状问题分析

通过调研当前 qingqi-feature-ftp-sftp-ssh-client 插件（共12883行代码），发现以下严重问题：

1. 架构问题

- view/mod.rs 4473行：严重违反单一职责，UI、状态、业务逻辑混在一起
- service.rs 1327行：既有领域逻辑又有UI状态管理
- 职责不清：runtime.rs、service.rs、backend.rs 职责重叠

2. 违反编码规范

- ❌ 命名：FtpSftpSshView 应为 SshView
- ❌ 没有 view-model 模式
- ❌ render 中可能有锁和计算
- ❌ 硬编码颜色：rgb(0x1E293B)
- ❌ 没有虚拟化列表

3. 功能混乱

- FTP/SFTP/SSH 混在一起，应该专注SSH
- 传输、终端、文件浏览耦合严重

二、重新设计目标

聚焦SSH/SFTP，提供：
- SSH终端连接
- SFTP文件浏览和传输
- 多会话管理
- 详细传输日志

三、新架构设计

3.1 目录结构

crates/qingqi-feature-ssh/
├── Cargo.toml
└── src/
    ├── lib.rs              # 导出 + databases()
    ├── manifest.rs         # 元数据
    ├── plugin.rs           # impl Plugin
    ├── model.rs            # 领域类型
    ├── store.rs            # 数据库
    ├── service.rs          # 核心服务
    ├── connection.rs       # SSH连接池
    ├── terminal.rs         # 终端引擎
    ├── transfer.rs         # 传输队列
    ├── sftp.rs             # SFTP客户端
    └── view/
        ├── mod.rs          # SshView主入口
        ├── sidebar.rs      # 左侧Profile列表
        ├── session_tabs.rs # 顶部Session Tab栏
        ├── file_tree.rs    # 文件树面板
        ├── terminal_pane.rs # 终端面板
        ├── transfer_panel.rs # 传输记录面板
        └── settings_dialog.rs # 设置弹窗

3.2 数据模型 (model.rs)

// ============ Profile 配置 ============
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthConfig,
    pub paths: PathConfig,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthConfig {
    pub method: AuthMethod,  // Password | PrivateKey | Agent
    pub password: String,
    pub private_key_path: String,
    pub passphrase: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathConfig {
    pub remote_root: String,  // 默认 "~"
    pub local_root: String,   // 默认 "~/Downloads"
}

// ============ Session 会话 ============
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

#[derive(Clone, Debug)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub profile_id: i64,
    pub title: String,
    pub endpoint: String,
    pub status: SessionStatus,  // Connecting | Connected | Failed
    pub has_terminal: bool,
    pub message: String,
}

// ============ 文件系统 ============
#[derive(Clone, Debug)]
pub struct RemoteEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_at: String,
}

// ============ 传输任务 ============
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransferId(Uuid);

#[derive(Clone, Debug)]
pub struct TransferTask {
    pub id: TransferId,
    pub session_id: SessionId,
    pub direction: TransferDirection,  // Upload | Download
    pub status: TransferStatus,  // Queued | Running | Completed | Failed
    pub local_path: String,
    pub remote_path: String,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub message: String,
    pub logs: Vec<String>,  // 详细日志
}

3.3 服务层 (service.rs)

pub struct SshService {
    database: Arc<DatabaseService>,
    cache_dir: PathBuf,
    connection_pool: Arc<ConnectionPool>,
    sessions: Arc<Mutex<HashMap<SessionId, SessionState>>>,
    revision: Arc<AtomicU64>,
}

struct SessionState {
    profile_id: i64,
    summary: SessionSummary,
    terminal: Option<Arc<TerminalEngine>>,
    sftp: Option<Arc<SftpClient>>,
    transfer_queue: Arc<TransferQueue>,
    remote_cwd: String,
    remote_entries: Vec<RemoteEntry>,
}

impl SshService {
    // ===== Profile 管理 =====
    pub fn list_profiles(&self) -> Result<Vec<Profile>>;
    pub fn create_profile(&self, draft: ProfileDraft) -> Result<Profile>;
    pub fn update_profile(&self, id: i64, draft: ProfileDraft) -> Result<Profile>;
    pub fn delete_profile(&self, id: i64) -> Result<bool>;

    // ===== Session 管理 =====
    pub fn open_session(&self, profile_id: i64) -> Result<SessionId>;
    pub fn close_session(&self, session_id: &SessionId) -> Result<()>;
    pub fn session_summaries(&self) -> Vec<SessionSummary>;
    pub fn session_snapshot(&self, id: &SessionId) -> Option<SessionSnapshot>;

    // ===== 终端操作 =====
    pub fn terminal_frame(&self, id: &SessionId) -> Option<TerminalFrame>;
    pub fn send_terminal_input(&self, id: &SessionId, input: TerminalInput) -> Result<()>;

    // ===== SFTP 文件操作 =====
    pub fn list_directory(&self, id: &SessionId, path: &str) -> Result<Vec<RemoteEntry>>;
    pub fn enter_directory(&self, id: &SessionId, path: &str) -> Result<()>;
    pub fn parent_directory(&self, id: &SessionId) -> Result<()>;
    pub fn create_directory(&self, id: &SessionId, path: &str) -> Result<()>;
    pub fn rename_entry(&self, id: &SessionId, old: &str, new: &str) -> Result<()>;
    pub fn remove_entry(&self, id: &SessionId, path: &str, is_dir: bool) -> Result<()>;

    // ===== 传输操作 =====
    pub fn upload_file(&self, id: &SessionId, local: &Path, remote: &str) -> Result<TransferId>;
    pub fn download_entry(&self, id: &SessionId, remote: &str, local: &Path) -> Result<TransferId>;
    pub fn all_transfer_snapshots(&self) -> Vec<TransferSnapshot>;
    pub fn cancel_transfer(&self, id: &TransferId);

    // ===== 事件订阅 =====
    pub fn subscribe_events(&self) -> UnboundedReceiver<SshEvent>;

    fn bump_revision(&self);
}

3.4 视图层 (view/mod.rs)

pub struct SshView {
    service: Arc<SshService>,
    focus_handle: FocusHandle,

    // ===== View Model (render-ready data) =====
    vm: SshViewModel,

    // ===== UI State =====
    selected_profile_id: Option<i64>,
    selected_session_id: Option<SessionId>,
    selected_remote_path: Option<String>,
    transfer_panel_expanded: bool,

    // ===== Overlays =====
    profile_editor: Option<ProfileEditorState>,
    settings_dialog: Option<SettingsDialogState>,
    context_menu: Option<ContextMenuState>,

    // ===== 后台任务 =====
    event_task: Option<Task<()>>,
    last_revision: u64,
}

// ===== View Model (在数据变化时计算一次，render只读) =====
struct SshViewModel {
    profiles: Vec<ProfileItem>,
    sessions: Vec<SessionTabItem>,
    file_tree: FileTreeViewModel,
    terminal: TerminalViewModel,
    transfers: TransferPanelViewModel,
}

struct ProfileItem {
    id: i64,
    name: String,
    endpoint: String,
    protocol_badge: String,
    is_connected: bool,
    is_selected: bool,
}

struct SessionTabItem {
    session_id: SessionId,
    title: String,
    is_selected: bool,
    status_color: Hsla,
}

struct FileTreeViewModel {
    current_path: String,
    parent_path: Option<String>,
    entries: Vec<FileEntryRow>,
}

struct FileEntryRow {
    path: String,
    name: String,
    icon: IconName,
    size_text: String,
    is_dir: bool,
    is_selected: bool,
}

struct TerminalViewModel {
    status: TerminalStatus,
    lines: Vec<TerminalLine>,
    cursor_visible: bool,
}

struct TransferPanelViewModel {
    active_count: usize,
    completed_count: usize,
    failed_count: usize,
    rows: Vec<TransferRowViewModel>,
}

struct TransferRowViewModel {
    id: TransferId,
    direction_icon: IconName,
    file_name: String,
    progress_percent: u8,
    status_text: String,
    status_color: Hsla,
    speed_text: String,
    logs: Vec<String>,  // 详细日志
}

impl SshView {
    pub fn new(service: Arc<SshService>, cx: &mut Context<Self>) -> Self;

    // ===== 数据更新 (计算 view-model) =====
    fn rebuild_view_model(&mut self);
    fn rebuild_profiles(&mut self) -> Vec<ProfileItem>;
    fn rebuild_sessions(&mut self) -> Vec<SessionTabItem>;
    fn rebuild_file_tree(&mut self) -> FileTreeViewModel;
    fn rebuild_terminal(&mut self) -> TerminalViewModel;
    fn rebuild_transfers(&mut self) -> TransferPanelViewModel;

    // ===== 事件处理 =====
    fn on_profile_select(&mut self, id: i64);
    fn on_profile_connect(&mut self, id: i64);
    fn on_session_select(&mut self, id: SessionId);
    fn on_session_close(&mut self, id: SessionId);
    fn on_file_entry_click(&mut self, path: String);
    fn on_file_entry_double_click(&mut self, path: String);
    fn on_terminal_key(&mut self, event: &KeyDownEvent);
    fn on_transfer_cancel(&mut self, id: TransferId);

    // ===== Overlay 管理 =====
    fn open_profile_editor(&mut self, mode: EditorMode);
    fn close_profile_editor(&mut self);
    fn open_settings_dialog(&mut self);
    fn close_settings_dialog(&mut self);
}

impl Render for SshView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // render 只读 self.vm，不做任何计算
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(render_main_layout(&self.vm, cx))
            .when_some(self.profile_editor.as_ref(), |root, editor| {
                root.child(render_profile_editor_overlay(editor, cx))
            })
    }
}

四、界面设计（按用户需求）

**核心布局：左侧栏独立，右侧主区域有自己的 tab 栏**

```
┌──────────────┬──────────────────────────────────────────────────┐
│ ● ● ● SSH    │ [Session 1] [Session 2] [Session 3]  [+]        │ <- 顶栏被分隔线切开
│   连接管理    │                                                  │    左：交通灯+标题
│              │                                                  │    右：Session Tab栏
│ [新建]       │                                                  │
├             ┼──────────────────┬───────────────────────────────┤
│              │ 文件树            │  终端                          │
│ Profile 列表 │                  │                               │
│              │                  │                               │
│ ┌──────────┐ │ ~/project/       │  $ ssh user@host              │
│ │ Profile1 │ │ ├─ src/          │  Last login: ...              │
│ │ [已连接] │ │ ├─ docs/         │  $ ls -la                     │
│ └──────────┘ │ └─ README.md     │  $ cd /var/log                │
│              │                  │  $ _                          │
│ ┌──────────┐ │ [上传] [刷新]    │                               │
│ │ Profile2 │ │ [新建文件夹]     │  [滚动条]                      │
│ │          │ │                  │                               │
│ └──────────┘ │                  │                               │
│              │                  │                               │
│ ┌──────────┐ │                  │                               │
│ │ Profile3 │ │                  │                               │
│ │          │ │                  │                               │
│ └──────────┘ │                  │                               │
│              │                  │                               │
│     [设置]   │                  │                               │
├──────────────┴──────────────────┴───────────────────────────────┤
│ 传输记录 (3 个进行中, 12 个已完成)         [展开/收起] [清空]    │
│ ▶ upload: project.zip → /home/user/  [████████--] 80% 2.1MB/s │
│   [详细日志] 2024-06-10 10:23:45 开始上传                       │
│              2024-06-10 10:23:46 已传输 1.2MB                  │
│              2024-06-10 10:23:47 速度: 2.1MB/s                 │
│ ▶ download: backup.tar.gz ← /backup/ [██████████] 100% 完成   │
│   [详细日志] 2024-06-10 10:20:12 开始下载                       │
│              2024-06-10 10:20:45 下载完成                       │
└────────────────────────────────────────────────────────────────┘
```

**关键布局特征：**
1. **顶部被竖直分隔线切开**：
   - 左侧：交通灯 + "SSH 连接管理" 标题 + 新建按钮
   - 右侧：Session Tab 栏（类似浏览器标签）
   
2. **左侧边栏完整独立**：
   - 从顶到底完整的边栏
   - 顶部：标题区
   - 中间：Profile 列表（滚动）
   - 底部：设置按钮

3. **右侧主工作区**：
   - 顶部：Session Tab 栏
   - 中间：左右分割（文件树 + 终端）
   - 底部：传输面板（可收起）

**组件详细设计：**

**1. 左侧边栏 (sidebar.rs)**

顶部标题区（固定高度）：
- 交通灯（macOS 风格）
- 标题："SSH 连接管理"
- 新建按钮

中间 Profile 列表（可滚动）：
- Profile 卡片样式：
  - 协议徽章 (SSH/SFTP)
  - 名称（大字）
  - endpoint (user@host:port，小字，monospace）
  - 连接状态指示器（左侧色条：未连接=透明，已连接=绿色，选中=青色）
  - 右侧连接/切换按钮
- 支持双击连接
- 支持右键菜单（编辑、删除、测试连接）

底部（固定）：
- 设置按钮（打开设置弹窗）

**2. Session Tab 栏 (session_tabs.rs)**

位置：右侧主区域顶部
- 每个 Session 一个 Tab
- Tab 内容：
  - 标题 (user@host)
  - 状态指示点（连接中=黄色，已连接=绿色，失败=红色）
  - 关闭按钮（hover 显示）
- 右侧 [+] 按钮（快速新建连接）
- 支持拖拽排序
- 下划线指示当前选中

**3. 文件树面板 (file_tree.rs)**

顶部工具栏（固定）：
- 当前路径面包屑（可点击跳转）
- 操作按钮：[上传] [刷新] [新建文件夹]

文件列表（虚拟化滚动）：
- 父目录 ".." 行（始终在顶部）
- 文件/文件夹行：
  - 图标（文件夹/文件，根据扩展名）
  - 名称（文件夹带 "/" 后缀）
  - 大小/类型（右侧）
  - 选中状态（青色背景 + 边框）
- 双击：进入文件夹 / 下载文件
- 右键菜单：
  - 文件夹：进入、重命名、删除
  - 文件：下载、重命名、删除
- 拖放支持：拖入文件 = 上传到当前目录

**4. 终端面板 (terminal_pane.rs)**

顶部状态栏（固定，暗色背景）：
- 终端图标
- 当前路径提示（如 "user@host ~/project"）
- 连接状态指示

终端内容区（可滚动）：
- 使用 alacritty_terminal 渲染
- 完整的 ANSI 颜色支持
- 光标显示
- 鼠标支持（如果服务端支持）
- 滚动历史（支持 PageUp/PageDown）
- 自动聚焦（切换 Session 时）

**5. 传输面板 (transfer_panel.rs)**

位置：底部，横跨整个窗口宽度

顶部控制栏（始终可见）：
- 左侧：统计信息 "传输记录 (3 个进行中, 12 个已完成, 2 个失败)"
- 右侧：[展开/收起] [清空已完成]

展开内容（可收起）：
- 传输任务列表（虚拟化，最多显示 50 条）
- 每行显示：
  - 展开图标（可展开查看详细日志）
  - 方向图标（↑ 上传 / ↓ 下载）
  - 文件名
  - 路径（小字，灰色）
  - 进度条（运行中时）
  - 百分比 / 状态
  - 速度（运行中时）
  - 取消按钮（运行中时）
- 展开详细日志：
  - 时间戳 + 日志行（小字，monospace）
  - 每个阶段：排队、开始、传输中（每秒一条）、完成/失败
  - 错误信息（红色）

状态颜色：
- 排队：灰色
- 运行中：青色 + 进度动画
- 完成：绿色
- 失败：红色
- 取消：橙色

**6. 设置弹窗 (settings_dialog.rs)**

居中模态弹窗，半透明背景遮罩

内容（两列布局）：
左侧：Profile 表单
- 基本信息：
  - 名称
  - 主机
  - 端口（默认 22）
  - 用户名
- 认证方式（单选）：
  - [ ] 密码
  - [ ] 私钥
  - [ ] SSH Agent
- 认证凭据（根据选择显示）：
  - 密码：密码输入框
  - 私钥：路径选择器 + 密码短语
  - Agent：无需配置
- 路径配置：
  - 远程根目录（默认 ~）
  - 本地下载目录（默认 ~/Downloads）
- 备注（可选）

右侧：操作按钮
- [测试连接] - 在保存前测试
- [保存] - 保存并关闭
- [取消] - 关闭不保存

底部：
- 如果是编辑模式，显示 [删除 Profile] 按钮（红色）

五、布局实现要点

**5.1 分离式顶栏的关键原理**

与传统的横贯全宽标题栏（如 Chrome）不同，本设计采用**分离式顶栏**：

```
传统布局（Chrome 风格）：
┌────────────────────────────────────┐
│ ● ● ●  全宽标题栏                   │ <- 横贯全宽
├────────┬───────────────────────────┤
│ 侧边栏 │ 主内容                     │
│        │                           │
└────────┴───────────────────────────┘

本设计（分离式）：
┌──────────┬────────────────────────┐
│ ● ● ● 标题│ Tab 栏                  │ <- 被分隔线切开
├──────────┼────────────────────────┤
│ 侧边栏   │ 主内容                  │
│          │                         │
└──────────┴────────────────────────┘
```

**实现方式：**

```rust
// ❌ 错误：全宽标题栏
div()
    .size_full()
    .flex_col()
    .child(div().w_full().child("全宽标题"))  // ❌
    .child(
        div().flex_1().flex()
            .child(sidebar)
            .child(main_area)
    )

// ✅ 正确：分离式
div()
    .size_full()
    .flex()  // 注意：flex 而非 flex_col
    .child(
        // 左侧完整列（包含自己的顶部）
        div()
            .w(px(280.0))
            .h_full()
            .flex_col()
            .child(div().h(px(52.0)).child("● ● ● 标题"))  // 左侧顶部
            .child(div().flex_1().child(profile_list))     // 左侧内容
            .child(div().h(px(48.0)).child(settings_btn))  // 左侧底部
    )
    .child(
        // 右侧完整列（包含自己的顶部）
        div()
            .flex_1()
            .h_full()
            .flex_col()
            .child(div().h(px(44.0)).child(session_tabs))  // 右侧顶部
            .child(div().flex_1().child(main_content))      // 右侧内容
    )
```

**关键点：**
1. 顶层不是 `flex_col`，而是 `flex`（水平布局）
2. 左侧边栏是完整的 `h_full()` 列，自带顶部
3. 右侧主区域是完整的 `h_full()` 列，自带 tab 栏
4. 两者之间自然形成竖直分隔线

**5.2 macOS 风格交通灯**

左侧顶部需要渲染 macOS 风格交通灯（三个圆点）：

```rust
fn mac_traffic_lights() -> impl IntoElement {
    div()
        .flex()
        .gap(px(8.0))
        .child(traffic_light_dot(rgb(0xED6A5E)))  // 红色
        .child(traffic_light_dot(rgb(0xF5BF4F)))  // 黄色
        .child(traffic_light_dot(rgb(0x61C554)))  // 绿色
}

fn traffic_light_dot(color: Hsla) -> impl IntoElement {
    div()
        .size(px(12.0))
        .rounded(px(6.0))
        .bg(color)
}
```

**5.3 高度对齐**

确保左右顶部高度一致，视觉上形成统一的"顶栏"：
- 左侧标题区：`h(px(52.0))`
- 右侧 tab 栏：`h(px(44.0))` + `pt(px(8.0))` = 视觉上 52px

**5.4 底部传输面板横跨全宽**

传输面板在主 flex 布局之外，单独作为一个子元素：

```rust
div()
    .size_full()
    .flex_col()  // 这里是 flex_col
    .child(
        // 顶部+中间：左右分割的主体
        div()
            .flex_1()
            .flex()
            .child(sidebar)
            .child(main_area)
    )
    .child(
        // 底部：传输面板（横跨全宽）
        div()
            .w_full()
            .h(if expanded { px(300.0) } else { px(48.0) })
            .child(transfer_panel)
    )
```

六、性能优化铁律

1. View-Model 模式

// ❌ 错误：render中计算
fn render(&mut self, ...) -> impl IntoElement {
    let profiles = self.service.list_profiles().unwrap();  // ❌
    profiles.sort();  // ❌
}

// ✅ 正确：事件触发时计算，render只读
fn on_data_changed(&mut self, cx: &mut Context<Self>) {
    self.vm.profiles = self.rebuild_profiles();  // 计算一次
    cx.notify();
}

fn render(&mut self, ...) -> impl IntoElement {
    div().children(self.vm.profiles.iter().map(render_profile))  // 只读
}

2. 虚拟化列表

// ✅ 文件列表使用 uniform_list
uniform_list(cx.entity(), "files", self.vm.file_tree.entries.len(),
    |this, range, _w, _cx| {
        this.vm.file_tree.entries[range].iter().map(render_file_row).collect()
    })

3. 无锁 render

// ❌ 禁止
fn render(&mut self, ...) {
    let data = self.service.sessions.lock().unwrap();  // ❌
}

// ✅ 数据已在 vm 中
fn render(&mut self, ...) {
    div().children(self.vm.sessions.iter().cloned())  // ✅
}

4. 语义化样式

// ❌ 硬编码
.bg(rgb(0x1E293B))

// ✅ 语义token
.bg(ui::bg_surface())
.text_color(theme::semantic().text_primary)

六、实现检查清单

阶段1：模型与服务

- [ ] model.rs：所有领域类型，纯数据，无GPUI
- [ ] store.rs：Profile持久化，无GPUI
- [ ] connection.rs：SSH连接池
- [ ] terminal.rs：终端引擎
- [ ] sftp.rs：SFTP客户端
- [ ] transfer.rs：传输队列
- [ ] service.rs：组装上述模块，Arc共享

阶段2：视图层

- [ ] view/mod.rs：SshView 主结构，SshViewModel
- [ ] view/sidebar.rs：Profile列表组件
- [ ] view/session_tabs.rs：Tab栏组件
- [ ] view/file_tree.rs：文件树组件（虚拟化）
- [ ] view/terminal_pane.rs：终端面板
- [ ] view/transfer_panel.rs：传输面板（带详细日志）
- [ ] view/settings_dialog.rs：设置弹窗

阶段3：验证

- [ ] 无 unwrap() / expect()
- [ ] 无硬编码颜色
- [ ] 所有列表虚拟化
- [ ] render 无 IO/锁/计算
- [ ] 异步有 generation guard
- [ ] 命名符合规范 (SshView, SshService)

七、关键代码示例

service.rs 核心结构

pub struct SshService {
    database: Arc<DatabaseService>,
    cache_dir: PathBuf,
    connection_pool: Arc<ConnectionPool>,
    sessions: Arc<Mutex<HashMap<SessionId, SessionState>>>,
    event_bus: EventBus,
    revision: Arc<AtomicU64>,
}

impl SshService {
    pub fn open_session(&self, profile_id: i64) -> Result<SessionId> {
        let profile = self.load_profile(profile_id)?;
        let session_id = SessionId::new();

        // 异步连接
        let pool = Arc::clone(&self.connection_pool);
        let event_bus = self.event_bus.clone();
        let sid = session_id.clone();

        thread::spawn(move || {
            match pool.connect(&profile) {
                Ok(conn) => {
                    let terminal = conn.open_terminal().ok();
                    let sftp = conn.open_sftp().ok();
                    // ... 初始化 session
                    event_bus.emit(SshEvent::SessionConnected(sid));
                }
                Err(e) => {
                    event_bus.emit(SshEvent::SessionFailed(sid, e.to_string()));
                }
            }
        });

        Ok(session_id)
    }

    pub fn snapshot(&self) -> SshSnapshot {
        let sessions = self.sessions.lock().unwrap();
        SshSnapshot {
            profiles: self.load_profiles().unwrap_or_default(),
            sessions: sessions.values().map(|s| s.summary.clone()).collect(),
            revision: self.revision.load(Ordering::SeqCst),
        }
    }
}

view/mod.rs 核心结构

impl SshView {
    fn start_event_loop(&mut self, cx: &mut Context<Self>) {
        let service = Arc::clone(&self.service);
        self.event_task = Some(cx.spawn(async move |view, acx| {
            let mut rx = service.subscribe_events();
            while let Some(event) = rx.recv().await {
                let _ = view.update(acx, |view, cx| {
                    view.on_service_event(&event, cx);
                });
            }
        }));
    }

    fn on_service_event(&mut self, event: &SshEvent, cx: &mut Context<Self>) {
        match event {
            SshEvent::SessionConnected(_) |
            SshEvent::SessionChanged(_) => {
                self.rebuild_view_model();
                cx.notify();
            }
            _ => {}
        }
    }

    fn rebuild_view_model(&mut self) {
        let snapshot = self.service.snapshot();
        self.vm = SshViewModel {
            profiles: self.build_profile_items(&snapshot),
            sessions: self.build_session_tabs(&snapshot),
            file_tree: self.build_file_tree(&snapshot),
            terminal: self.build_terminal(&snapshot),
            transfers: self.build_transfers(&snapshot),
        };
        self.last_revision = snapshot.revision;
    }
}

// ===== 布局实现：分离式顶栏 =====
impl Render for SshView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = theme_mode::is_dark();
        
        div()
            .size_full()
            .bg(ui::bg_base())
            .flex()
            .flex_col()
            .child(
                // 主体区域：左右分割
                div()
                    .flex_1()
                    .flex()
                    .child(
                        // 左侧边栏（完整高度，包含自己的顶部）
                        render_sidebar(
                            &self.vm.profiles,
                            self.selected_profile_id,
                            cx,
                            dark,
                        )
                    )
                    .child(
                        // 右侧主区域（包含自己的 tab 栏）
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(
                                // 右侧顶部：Session Tab 栏
                                render_session_tabs(
                                    &self.vm.sessions,
                                    self.selected_session_id,
                                    cx,
                                )
                            )
                            .child(
                                // 右侧内容：文件树 + 终端
                                div()
                                    .flex_1()
                                    .flex()
                                    .child(render_file_tree(&self.vm.file_tree, cx, dark))
                                    .child(render_terminal(&self.vm.terminal, cx, dark))
                            )
                    )
            )
            .child(
                // 底部传输面板（横跨全宽）
                render_transfer_panel(
                    &self.vm.transfers,
                    self.transfer_panel_expanded,
                    cx,
                    dark,
                )
            )
            // Overlays
            .when_some(self.profile_editor.as_ref(), |root, editor| {
                root.child(render_profile_editor_overlay(editor, cx))
            })
    }
}

// view/sidebar.rs
fn render_sidebar(
    profiles: &[ProfileItem],
    selected_id: Option<i64>,
    cx: &mut Context<SshView>,
    dark: bool,
) -> impl IntoElement {
    div()
        .w(px(280.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(ui::bg_surface())
        .border_r_1()
        .border_color(ui::border())
        .child(
            // 顶部标题区（包含交通灯）
            div()
                .h(px(52.0))
                .flex()
                .items_center()
                .px_3()
                .border_b_1()
                .border_color(ui::border())
                .child(mac_traffic_lights())  // macOS 交通灯
                .child(
                    div()
                        .ml_2()
                        .text_size(px(15.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("SSH 连接管理")
                )
                .child(
                    // 右侧新建按钮
                    Button::new("new-profile")
                        .icon(IconName::Plus)
                        .ghost()
                        .small()
                        .on_click(cx.listener(|view, _, w, cx| {
                            view.open_profile_editor(EditorMode::Create, w, cx);
                        }))
                )
        )
        .child(
            // Profile 列表（可滚动）
            div()
                .flex_1()
                .overflow_y_scroll()
                .p_2()
                .children(profiles.iter().map(|profile| {
                    render_profile_card(profile, selected_id == Some(profile.id), cx, dark)
                }))
        )
        .child(
            // 底部设置按钮
            div()
                .h(px(48.0))
                .flex()
                .items_center()
                .justify_center()
                .border_t_1()
                .border_color(ui::border())
                .child(
                    Button::new("settings")
                        .icon(IconName::Settings)
                        .label("设置")
                        .ghost()
                        .on_click(cx.listener(|view, _, w, cx| {
                            view.open_settings_dialog(w, cx);
                        }))
                )
        )
}

// view/session_tabs.rs
fn render_session_tabs(
    sessions: &[SessionTabItem],
    selected_id: Option<SessionId>,
    cx: &mut Context<SshView>,
) -> impl IntoElement {
    div()
        .h(px(44.0))
        .flex()
        .items_center()
        .px_2()
        .bg(ui::bg_surface())
        .border_b_1()
        .border_color(ui::border())
        .child(
            TabBar::new("sessions")
                .underline()
                .selected_index(
                    sessions.iter().position(|s| Some(s.session_id) == selected_id)
                        .unwrap_or(0)
                )
                .children(sessions.iter().map(|session| {
                    render_session_tab(session, selected_id == Some(session.session_id), cx)
                }))
        )
        .child(
            Button::new("new-session")
                .icon(IconName::Plus)
                .ghost()
                .xsmall()
                .on_click(cx.listener(|view, _, _, cx| {
                    // 快速连接逻辑
                }))
        )
}

八、迁移策略

不是重构，是重写：

1. 新建 crates/qingqi-feature-ssh/
2. 并行开发，旧代码保持运行
3. 阶段性迁移：
  - 先迁移 Profile 数据库（兼容旧schema）
  - 新插件可以读取旧 Profile
4. 切换：qingqi/src/main.rs 注册新插件
5. 删除旧插件

九、常见错误与陷阱

**9.1 布局错误**

❌ **错误1：顶栏横贯全宽**
```rust
// 这会让顶栏横跨整个窗口，而不是分离式
div().flex_col()
    .child(div().w_full().child("标题栏"))  // ❌
    .child(div().flex().child(sidebar).child(main))
```

✅ **正确：左右各自独立的顶部**
```rust
div().flex()
    .child(div().h_full().flex_col().child("左侧标题").child(sidebar))  // ✅
    .child(div().h_full().flex_col().child("右侧tab").child(main))     // ✅
```

❌ **错误2：忘记虚拟化大列表**
```rust
// Profile 列表超过 50 个时会卡顿
div().children(profiles.iter().map(render_profile))  // ❌
```

✅ **正确：使用 uniform_list**
```rust
uniform_list(cx.entity(), "profiles", profiles.len(), ...)  // ✅
```

**9.2 数据流错误**

❌ **错误3：render 中访问 service**
```rust
fn render(&mut self, ...) -> impl IntoElement {
    let data = self.service.list_profiles().unwrap();  // ❌ IO
    data.sort();  // ❌ 计算
}
```

✅ **正确：render 只读 view-model**
```rust
fn render(&mut self, ...) -> impl IntoElement {
    div().children(self.vm.profiles.iter().cloned())  // ✅
}
```

❌ **错误4：忘记 generation guard**
```rust
cx.spawn(async move |view, acx| {
    let result = heavy_work().await;
    view.update(acx, |view, cx| {
        view.apply(result);  // ❌ 可能已过期
    });
})
```

✅ **正确：加入 generation guard**
```rust
self.generation = self.generation.wrapping_add(1);
let gen = self.generation;
cx.spawn(async move |view, acx| {
    let result = heavy_work().await;
    view.update(acx, |view, cx| {
        if view.generation != gen { return; }  // ✅
        view.apply(result);
    });
})
```

**9.3 样式错误**

❌ **错误5：硬编码颜色**
```rust
.bg(rgb(0x1E293B))  // ❌
.text_color(hsla(0.0, 0.0, 0.6, 1.0))  // ❌
```

✅ **正确：语义化 token**
```rust
.bg(ui::bg_surface())  // ✅
.text_color(theme::semantic().text_secondary)  // ✅
```

❌ **错误6：固定高度列表**
```rust
.h(px(500.0))  // ❌ 固定高度
```

✅ **正确：flex 布局**
```rust
.flex_1()  // ✅ 自动填充
.min_h(px(0.0))  // ✅ 防止溢出
```

**9.4 命名错误**

❌ **错误7：违反命名规范**
```rust
struct SshPanel { ... }  // ❌ 应该是 SshView
struct SshRuntime { ... }  // ❌ 应该是 SshService
```

✅ **正确：遵循规范**
```rust
struct SshView { ... }  // ✅
struct SshService { ... }  // ✅
struct ProfileItem { ... }  // ✅ view-model 后缀 Item/ViewModel
```

**9.5 错误处理**

❌ **错误8：unwrap 滥用**
```rust
let data = self.service.data().unwrap();  // ❌
let guard = self.lock.lock().unwrap();  // ❌
```

✅ **正确：降级或传播错误**
```rust
let data = self.service.data().unwrap_or_default();  // ✅
let guard = self.lock.lock().map_err(|_| anyhow!("lock poisoned"))?;  // ✅
```

**9.6 日志记录**

❌ **错误9：传输日志丢失**
```rust
// 只记录最终状态
TransferTask { 
    status: Completed,
    message: "完成",
    logs: vec![],  // ❌ 没有详细日志
}
```

✅ **正确：记录每个阶段**
```rust
TransferTask {
    status: Completed,
    message: "完成",
    logs: vec![
        "2024-06-10 10:23:45.123 [INFO] 加入传输队列".into(),
        "2024-06-10 10:23:45.234 [INFO] 开始上传 project.zip".into(),
        "2024-06-10 10:23:46.100 [INFO] 已传输 1.2MB (40%)".into(),
        "2024-06-10 10:23:47.050 [INFO] 已传输 2.4MB (80%)".into(),
        "2024-06-10 10:23:47.890 [INFO] 上传完成，耗时 2.7s".into(),
    ],  // ✅
}
```

十、验证清单（提交前必查）

**代码质量：**
- [ ] 无 `unwrap()` / `expect()`（测试除外）
- [ ] 无 `lock().unwrap()`
- [ ] 无硬编码颜色 `rgb(0x...)`
- [ ] 无硬编码字体 `.font_family("...")`

**架构规范：**
- [ ] model.rs 无 GPUI 依赖
- [ ] store.rs 无 GPUI 依赖
- [ ] service.rs 无 GPUI 依赖
- [ ] view 文件正确分离（mod.rs < 500 行）

**性能规范：**
- [ ] 所有列表（>20 项）使用 `uniform_list`
- [ ] render 无 IO/锁/计算
- [ ] view-model 模式正确实现
- [ ] 异步操作有 generation guard

**UI 规范：**
- [ ] 分离式顶栏布局正确
- [ ] 左侧边栏包含交通灯
- [ ] Session tab 栏在右侧顶部
- [ ] 传输面板可展开收起
- [ ] 传输日志详细完整

**功能完整性：**
- [ ] Profile CRUD 完整
- [ ] Session 多标签管理
- [ ] 文件树支持上传/下载/重命名/删除
- [ ] 终端支持完整输入输出
- [ ] 传输面板显示详细日志（每秒一条）
- [ ] 设置弹窗功能完整

**测试验证：**
- [ ] `cargo check --workspace` 通过
- [ ] `cargo clippy --workspace` 无警告
- [ ] Profile 创建、编辑、删除流程
- [ ] 连接 SSH 并打开终端
- [ ] 浏览 SFTP 文件树
- [ ] 上传下载文件并查看日志
- [ ] 同时打开 3 个 Session 测试

---

**这份设计文档可以直接交给低级模型实现。**

**关键原则：**
- 严格分层：model → service → view
- View-Model 模式
- 虚拟化列表
- 无锁 render
- 分离式顶栏布局
- 详细的传输日志（每秒一条）

**预期工作量：**
- 模型层（model.rs, store.rs）：1-2 天
- 服务层（service.rs, connection.rs, terminal.rs, sftp.rs, transfer.rs）：3-4 天
- 视图层（view/*.rs）：3-4 天
- 测试与调试：2-3 天
- **总计：约 10-12 天**
