# SSH 远程管理插件 — 重新设计规格

> 基于 `docs/ssh-redesign.md` 讨论确认后形成的最终设计规格
> 日期：2026-06-10

---

## 一、目标与范围

重新实现 SSH 远程管理插件，替代现有 `qingqi-feature-ftp-sftp-ssh-client`（12883 行，架构混乱）。新插件命名为 `qingqi-feature-ssh`，是一个重写而非重构。

### 协议支持

同时支持四种远程协议：SSH（含 SFTP 文件浏览）、FTP、FTPS。底层库使用 `russh`（SSH/SFTP）和 `suppaftp`（FTP/FTPS）。

> 注：SFTP 是 SSH 的子系统，通过 SSH 连接后打开 SFTP channel 实现文件操作。因此 ProtocolType 枚举只有 Ssh、Ftp、Ftps 三个变体，选择 SSH 即同时获得终端和 SFTP 文件浏览能力。

### 核心功能

- SSH/SFTP：交互式终端、文件浏览、上传下载
- FTP/FTPS：命令日志终端、文件浏览、上传下载
- 多 Session 标签页管理
- 传输任务队列（含详细日志，每秒一条）
- Profile 持久化配置

---

## 二、架构决策

### 2.1 策略：重写而非重构

- 新建 `crates/qingqi-feature-ssh/`
- 与旧插件并行开发，旧代码保持运行
- 完成后在 `registry.rs` 切换注册，删除旧 crate

### 2.2 协议抽象：RemoteProtocol trait

采用 trait 对象抽象四协议差异，而非枚举分发或双轨分离：

- **选型理由**：四协议同步开发，trait 抽象扩展成本最低；与 service → session → view 分层契合；单元测试可 mock
- **ConnectionPool** 持有 `HashMap<ProfileId, Arc<dyn RemoteProtocol>>`，通过 `ProtocolRegistry`（工厂注册表）根据 `ProtocolType` 创建对应实例
- **终端统一**：SSH 走 PTY 交互模式，FTP/FTPS 走命令日志模式。两者通过 `TerminalOutput` 枚举和 `TerminalEngine` 统一

### 2.3 FTP/FTPS 的终端处理

FTP 连接后终端显示命令/响应日志（非交互式 shell）。日志行用颜色区分方向：发送的命令用青色，接收的响应用灰色，错误用红色。底部保留手动输入框，用户可输入 FTP 原生命令。`TerminalPane` 对两种模式透明 — 拿到 `TerminalFrame` 就渲染，不关心来源。

---

## 三、数据模型

### 3.1 核心类型

**Profile（配置持久化实体）**
- id、name、host、port（SSH=22、FTP=21、FTPS=990）
- protocol: ProtocolType 枚举（Ssh / Ftp / Ftps）
- auth: AuthConfig 枚举，按协议区分：
  - SSH：SshAuthMethod（Password / PrivateKey / Agent）
  - FTP/FTPS：用户名 + 密码
- paths: PathConfig（remote_root、local_root）
- note、created_at、updated_at

**SessionSummary（会话摘要，render 数据）**
- session_id、profile_id、title（如 "user@host"）、endpoint
- protocol: ProtocolType、status: SessionStatus（Connecting/Connected/Failed）
- terminal_kind: TerminalKind（Shell / Log，FTP 为 Log）
- has_terminal: bool、message: String

**RemoteEntry（远程文件条目）**
- path、name、is_dir、size、modified_at

**TransferTask（传输任务）**
- id、session_id、direction（Upload/Download）
- status（Queued/Running/Completed/Failed/Cancelled）
- local_path、remote_path、transferred_bytes、total_bytes
- started_at、finished_at、message
- logs: Vec&lt;String&gt;，带时间戳的详细日志

### 3.2 存储设计

`ProfileStore` 基于 qingqi-plugin 的 DatabaseService（SQLite）。profiles 表包含 id、name、protocol、host、port、auth_json（JSON 序列化，适配多协议认证差异）、remote_root、local_root、note、created_at、updated_at。auth_json 用 JSON 字段是为了无需动态 schema 变更即可支持 SSH 三种认证方式和 FTP 认证方式。

旧插件 Profile 数据可兼容读取（新 store 读取旧表 schema，迁移到新表）。

### 3.3 规范要求

- model.rs 无 GPUI 依赖，纯数据类型
- store.rs 无 GPUI 依赖
- 所有类型实现 Clone + Debug
- id 类型使用 newtype 包装 Uuid

---

## 四、服务层

### 4.1 SshService 结构

核心服务，组装所有子模块，通过 Arc 共享给 View 层。

组成部分：
- database: Arc&lt;DatabaseService&gt;（Profile 持久化）
- connection_pool: Arc&lt;ConnectionPool&gt;（持有 ProtocolRegistry，管理活跃连接）
- sessions: Arc&lt;Mutex&lt;HashMap&lt;SessionId, SessionState&gt;&gt;&gt;（会话状态）
- event_tx: broadcast::Sender&lt;SshEvent&gt;（事件广播）
- revision: Arc&lt;AtomicU64&gt;（版本号，View 层据此判断是否需要重建 view-model）

### 4.2 ConnectionPool 与 ProtocolRegistry

`ProtocolRegistry` 是协议工厂注册表，在 service 初始化时注册 ssh/ftp/ftps 三个工厂函数。`ConnectionPool` 持有 registry，按 profile_id 管理活跃连接。对外暴露 `get_or_connect(profile: &Profile) -> Result<Arc<dyn RemoteProtocol>>`，自动根据 profile.protocol 创建对应连接实例（已连接则复用，否则新建）。同时提供 disconnect 和 close_all 方法。

### 4.3 SessionState

每个活跃 Session 的内部状态，包含：profile_id、protocol、summary（SessionSummary）、terminal（Option&lt;Arc&lt;TerminalEngine&gt;&gt;，SSH=PTY 终端，FTP=日志终端）、entries（当前目录文件列表）、remote_cwd、transfer_queue。

### 4.4 公开 API

**Profile 管理**：list、create、update、delete、test_connection（返回连接状态和耗时）

**Session 管理**：open_session（根据 profile 创建 Session，异步连接）、close_session、session_summaries、session_snapshot

**终端操作**：terminal_snapshot、send_terminal_input、resize_terminal

**文件操作**：list_directory、enter_directory、parent_directory、create_directory、rename_entry、remove_entry

**传输操作**：upload（返回 TransferId）、download、transfer_snapshots（按 session_id 过滤，因为传输面板在 Tab 内）、cancel_transfer

**事件订阅**：subscribe() 返回 broadcast::Receiver&lt;SshEvent&gt;，View 层通过此通道感知数据变化

### 4.5 事件类型

SshEvent 枚举包含：ProfileCreated、ProfileUpdated、ProfileDeleted、SessionOpened、SessionConnected（连接已建立，可创建终端和 SFTP 客户端）、SessionDataChanged（终端输出/目录列表变化/传输进度）、SessionClosed、TransferChanged（进度更新/日志新增）。

### 4.6 规范要求

- service.rs 无 GPUI 依赖
- 所有公开方法返回纯数据
- 无 unwrap/expect，错误通过 anyhow::Result 传播

---

## 五、视图层

### 5.1 整体布局：分离式顶栏

窗口顶层是水平 flex 布局（不是 flex_col），分为左右两列：

**左侧列（宽 280px，完整高度）**：
- 顶部 52px：macOS 交通灯（红黄绿三个圆点，各 12px）+ "远程管理" 标题 + 新建按钮 [+]
- 中间 flex_1：Profile 虚拟化列表。每张卡片含协议徽章、名称、endpoint（user@host:port，monospace）、左侧 3px 状态色条（绿色=已连接，透明=未连接，选中=青色背景）。双击连接，右键菜单（编辑、删除、测试连接）
- 底部 48px：设置按钮，居中

**右侧列（flex_1 弹性宽度）**：
- 顶部 44px：Session Tab 栏。每个 Tab 含标题（user@host）、状态圆点（黄色=连接中、绿色=已连接、红色=失败）、关闭按钮（hover 显示）。右侧 [+] 快速新建连接按钮
- 中间 flex_1：水平分割（左侧文件树 + 右侧终端）
- 底部：传输面板（嵌入 Tab 内容内，默认收缩为单行状态栏"3 进行中, 12 已完成"，展开后约 200px）

### 5.2 数据流：View-Model 模式

SshView 在构造时启动后台事件循环，订阅 SshService 的 broadcast channel。收到事件后调用 `rebuild_view_model()` 从 service snapshot 重新计算 `SshViewModel`，然后 `cx.notify()`。

render 方法严格只读 `self.vm`，不做任何 IO、锁操作、计算或排序。各子 view 渲染函数接收 view-model 的对应切片引用。

### 5.3 SshViewModel 结构

包含五个子 view-model，均为纯数据、render-ready：

- **profiles**: Vec&lt;ProfileItem&gt; — id、名称、endpoint、协议徽章、是否已连接、是否选中
- **sessions**: Vec&lt;SessionTabItem&gt; — session_id、标题、选中状态、状态色
- **file_tree**: FileTreeViewModel — 当前路径、父路径、文件条目列表（路径、名称、图标名、大小文本、是否目录、是否选中）
- **terminal**: TerminalViewModel — 终端状态、行列表（含颜色属性）、光标可见性
- **transfers**: TransferPanelViewModel — 活跃/完成/失败计数、传输行列表（方向图标、文件名、进度、状态、速度、展开日志）

### 5.4 子组件职责

**sidebar.rs**：渲染左侧边栏，包括顶部标题区（交通灯+标题+新建按钮）、Profile 虚拟化列表（uniform_list）、底部设置按钮。处理双击连接、右键菜单事件。

**session_tabs.rs**：渲染 Session Tab 栏，使用 gpui-component 的 TabBar 组件，带选中下划线指示。处理 Tab 切换、关闭、新建事件。

**file_tree.rs**：渲染文件树面板。顶部工具栏固定（面包屑路径+上传/刷新/新建文件夹按钮），文件列表用 uniform_list 虚拟化。支持单击选中、双击进入/下载、拖放上传（ExternalPaths）、右键菜单（进入、下载、重命名、删除）。FTP 协议下部分操作根据权限置灰。

**terminal_pane.rs**：渲染终端面板。顶部状态栏显示图标+user@host:path+连接状态。内容区渲染 `TerminalFrame`（两种模式透明：SSH 的 ANSI 输出 / FTP 的日志行）。处理键盘输入、鼠标、滚轮、track_focus。

**transfer_panel.rs**：渲染传输面板。默认收缩为单行状态栏（"传输记录（X 进行中, Y 已完成）" + 展开/收起按钮）。展开后显示传输任务列表（uniform_list），每行含展开按钮、方向图标、文件名、路径、进度条、状态/速度、取消按钮。展开后可查看详细日志（monospace 字体，带时间戳，自动滚动到最新）。

**settings_dialog.rs**：渲染 Profile 编辑 overlay。半透明背景遮罩，居中模态弹窗。表单两列布局：基本信息（名称、协议选择下拉、主机、端口）、认证方式（根据协议条件显示：SSH 显示密码/私钥/Agent 单选，FTP 显示用户名+密码）、路径配置、备注。右侧操作按钮：测试连接、保存、取消。编辑模式底部显示红色删除按钮。ESC 关闭。

### 5.5 规范要求

- render 无 IO/锁/计算
- 所有列表使用 uniform_list 虚拟化
- 异步操作有 generation guard
- 实体只创建一次，不在 render 中 cx.new
- 使用语义化颜色 token（ui::bg_surface() 等），禁止硬编码 rgb(...)
- 命名遵循规范：SshView、SshService、ProfileItem、SessionTabItem

---

## 六、GUI 实现要点总结

### 6.1 分离式顶栏实现

- 顶层使用水平 flex（不是 flex_col）
- 左侧：独立自包含列（w(280px), h_full, flex_col）
- 右侧：独立自包含列（flex_1, h_full, flex_col）
- 左右各自拥有顶部区域，两者顶部之间自然形成竖直分隔线
- macOS 交通灯用三个 12px 圆形 div 表示

### 6.2 传输面板位置

传输面板在右侧列的 Tab 内容区域内（文件树/终端的下方），不横跨全窗口。每个 Session 的传输记录独立。默认收缩为单行控制栏，展开后撑起约 200px 高度。

### 6.3 FTP 日志终端

TerminalPane 对 SSH/FTP 透明。FTP 模式下，终端内容为带颜色的命令响应日志（发送=青色，接收=灰色，错误=红色），底部有 FTP 原生命令输入行。终端顶部状态栏正常显示连接信息。

### 6.4 Profile 编辑弹窗

Overlay 模式：半透明遮罩 + 居中模态表单。认证方式区域根据 protocol 选择动态切换显示内容（SSH 三种认证选项，FTP 用户名密码）。

---

## 七、关键决策记录

| 决策 | 结论 | 日期 |
|------|------|------|
| 开发策略 | 重写（新 crate qingqi-feature-ssh） | 2026-06-10 |
| SSH 底层库 | russh | 2026-06-10 |
| FTP/FTPS 底层库 | suppaftp | 2026-06-10 |
| 协议架构 | RemoteProtocol trait 抽象 | 2026-06-10 |
| 四协议支持 | SSH/SFTP/FTP/FTPS 同步开发 | 2026-06-10 |
| FTP 终端 | 命令日志终端模式，支持手动输入 | 2026-06-10 |
| 布局 | 分离式顶栏，传输面板在 Tab 内容内 | 2026-06-10 |
| 命名 | SshView、SshService、SshPlugin | 2026-06-10 |

---

## 八、Crate 目录结构

```
crates/qingqi-feature-ssh/
├── Cargo.toml
└── src/
    ├── lib.rs                  # 导出 + databases() + build()
    ├── manifest.rs             # 元数据（PLUGIN_ID="ssh"）
    ├── plugin.rs               # impl Plugin
    ├── model.rs                # 领域类型（纯数据，无 GPUI）
    ├── store.rs                # Profile 持久化（无 GPUI）
    ├── service.rs              # 核心服务组装
    ├── connection.rs           # ConnectionPool + ProtocolRegistry
    ├── protocol/
    │   ├── mod.rs              # RemoteProtocol trait
    │   ├── ssh.rs              # russh 实现
    │   └── ftp.rs              # suppaftp FTP/FTPS 实现
    ├── terminal.rs             # 终端引擎（PTY + 日志双模式）
    ├── transfer.rs             # 传输队列
    └── view/
        ├── mod.rs              # SshView + SshViewModel
        ├── sidebar.rs          # 左侧边栏
        ├── session_tabs.rs     # Session Tab 栏
        ├── file_tree.rs        # 文件树面板
        ├── terminal_pane.rs    # 终端面板
        ├── transfer_panel.rs   # 传输记录面板
        └── settings_dialog.rs  # 设置弹窗
```

## 九、迁移路径

1. 新建 `crates/qingqi-feature-ssh/`，添加 workspace 成员
2. 并行开发，旧插件 `qingqi-feature-ftp-sftp-ssh-client` 保持运行
3. 新 store 兼容读取旧 Profile schema
4. 功能完整后在 `crates/qingqi/src/features/registry.rs` 切换注册
5. 确认无问题后删除旧 crate
