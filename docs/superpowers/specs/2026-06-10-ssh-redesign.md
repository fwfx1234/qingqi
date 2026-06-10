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

## 六、性能优化铁律

这是对编码层面的硬性约束，任何违反都会导致 UI 卡顿，code review 时必须拒绝。

### 6.1 View-Model 模式

数据变化时在事件回调中计算一次 view-model，render 只读不计算。

- **正确做法**：收到 SshEvent 后调用 `rebuild_view_model()` 从 service snapshot 重建 `self.vm`，然后 `cx.notify()`。render 方法直接使用 `self.vm.profiles`、`self.vm.file_tree` 等字段。
- **禁止**：在 render 中调用 `self.service.list_profiles()`、排序数组、格式化字符串、过滤集合。所有计算必须在 rebuild 阶段完成。

### 6.2 虚拟化列表

超过一屏的列表必须使用 `uniform_list` 而非全量渲染。

- Profile 列表（左侧边栏）— 超过 20 个时虚拟化
- 文件树列表 — 文件数超过一屏时虚拟化，1000+ 文件不能卡顿
- 传输任务列表 — 超过 20 条时虚拟化
- **禁止**：`div().children(profiles.iter().map(...))` 直接全量渲染

### 6.3 无锁 render

render 方法中禁止任何形式的锁操作。

- **禁止**：`self.service.sessions.lock()`、`self.service.sessions.lock().unwrap()`、`mutex.lock()`
- **原因**：render 在主线程执行，持锁可能阻塞整个 UI。数据应已在 view-model 中就绪。

### 6.4 精准 notify

- 使用 `cx.notify()` 只标脏当前 view，触发局部重绘
- 禁止 `window.refresh()` 全局重绘（仅确需时用）
- 每个子 view 的变化只 notify 自己，不通知父级

### 6.5 实体只创建一次

TextInput、FocusHandle 等实体在 SshView 构造函数中 `cx.new()` 一次，存储在 struct 字段中。render 方法复用已有实体。

- **禁止**：在 render 中 `cx.new(TextInput::new())`，每次重绘都会泄漏实体
- Profile 编辑弹窗中的输入框在弹窗打开时创建，关闭时销毁

### 6.6 异步 Generation Guard

所有 `cx.spawn` 异步操作必须带 generation guard，防止回调时 view 状态已过期。

- 在 spawn 前递增 `self.generation`（wrapping_add），保存到局部变量
- 在 `view.update` 回调中首先检查 `view.generation == gen`，不匹配则直接 return
- 适用场景：`open_session` 的连接等待、文件上传下载的进度回调、终端输出的流式读取

### 6.7 语义化样式

- 禁止硬编码颜色：`rgb(0x...)`、`hsla(...)`
- 使用语义化 token：`ui::bg_surface()`、`ui::text_primary()`、`ui::border()`、`ui::text_secondary()`
- 禁止硬编码字体：`font_family("PingFang SC")`，用 `ui::font_ui()` 等 token

### 6.8 布局防溢出

- 弹性区域使用 `flex_1()` + `min_h(px(0.0))`，防止内容撑破容器
- 固定高度区域明确指定 `h(px(N))`，不依赖内容撑高
- 终端面板和文件树使用 `flex_1()` 自动填充剩余空间

---

## 七、GUI 实现要点总结

### 7.1 分离式顶栏实现

- 顶层使用水平 flex（不是 flex_col）
- 左侧：独立自包含列（w(280px), h_full, flex_col）
- 右侧：独立自包含列（flex_1, h_full, flex_col）
- 左右各自拥有顶部区域，两者顶部之间自然形成竖直分隔线
- macOS 交通灯用三个 12px 圆形 div 表示

### 7.2 传输面板位置

传输面板在右侧列的 Tab 内容区域内（文件树/终端的下方），不横跨全窗口。每个 Session 的传输记录独立。默认收缩为单行控制栏，展开后撑起约 200px 高度。

### 7.3 FTP 日志终端

TerminalPane 对 SSH/FTP 透明。FTP 模式下，终端内容为带颜色的命令响应日志（发送=青色，接收=灰色，错误=红色），底部有 FTP 原生命令输入行。终端顶部状态栏正常显示连接信息。

### 7.4 Profile 编辑弹窗

Overlay 模式：半透明遮罩 + 居中模态表单。认证方式区域根据 protocol 选择动态切换显示内容（SSH 三种认证选项，FTP 用户名密码）。

---

## 八、错误处理规范

这是对编码层面的硬性约束，违反会导致运行时崩溃或不可恢复的错误。

### 8.1 禁止项

- `unwrap()` — 非测试代码、非数学不变量证明时禁止
- `expect()` — 同上
- `lock().unwrap()` — Mutex/RwLock 的锁中毒必须处理，不能 panic

### 8.2 正确做法

**传播错误**：使用 `?` 操作符将错误向上传播，让调用方决定如何处理。

**错误上下文**：使用 `.with_context(|| format!("描述正在做什么"))` 为错误附加语义信息，方便定位问题。例如读取文件时附加文件路径，连接服务器时附加主机地址。

**锁恢复**：`Mutex` 中毒（另一个线程 panic 时持有锁）使用 `lock_or_recover` 或 `mutex.lock().map_err(|_| anyhow!("锁中毒: 描述"))` 降级而非 panic。

**降级策略**：对于非关键数据，使用 `unwrap_or(default)`、`unwrap_or_default()`、`unwrap_or_else(|| fallback)` 提供合理的回退值，而不是因一个非关键字段缺失就让整个程序崩溃。

### 8.3 适用范围

此规范适用于所有层（model、store、service、view），尤其是 service.rs 中的网络操作和 store.rs 中的数据库操作，这些操作的失败是预期的（网络断开、权限不足、磁盘满），必须以 Result 传播，不能 panic。

---

## 九、传输日志格式规范

传输任务的日志（TransferTask.logs）是用户诊断传输问题的主要手段，必须满足以下标准。

### 9.1 日志格式

每条日志的格式为：`时间戳 [级别] 内容`

- **时间戳**：`YYYY-MM-DD HH:MM:SS` 格式，精确到秒
- **级别**：INFO（正常进度）、WARN（重试/降级）、ERROR（失败原因）
- **内容**：用中文描述当前发生的事件

### 9.2 必须记录的阶段

每个传输任务至少包含以下日志条目：

| 阶段 | 示例 | 说明 |
|------|------|------|
| 入队 | `10:23:45 [INFO] 加入传输队列` | 任务创建时立即记录 |
| 开始 | `10:23:46 [INFO] 开始上传 project.zip (3.2MB)` | 传输实际开始时，含文件名和大小 |
| 进度 | `10:23:47 [INFO] 已传输 1.2MB (40%)` | 每秒至少一条，直到完成 |
| 完成 | `10:23:50 [INFO] 完成，耗时 4.6s，平均 712 KB/s` | 含总耗时和平均速度 |
| 失败 | `10:23:48 [ERROR] 失败: 网络连接断开` | 含具体错误原因 |
| 取消 | `10:23:47 [WARN] 已取消` | 用户主动取消时记录 |

### 9.3 日志数量

- 运行中的传输每秒至少产生一条进度日志
- 不存在只有开始和完成两条日志的传输任务
- 日志总数与传输时长成正比

---

## 十、验证清单（提交前必查）

这是完成的定义，所有项必须在 PR 前通过。

### 10.1 代码质量

- 无 `unwrap()` / `expect()`（测试除外）
- 无 `lock().unwrap()`
- 无硬编码颜色 `rgb(0x...)` 或 `hsla(...)`
- 无硬编码字体 `.font_family("...")`
- `cargo fmt --all` 通过
- `cargo clippy --workspace --all-targets` 无警告

### 10.2 架构规范

- model.rs 无 GPUI 依赖（`cargo tree -p qingqi-feature-ssh -e no-dev | grep gpui` 确认）
- store.rs 无 GPUI 依赖
- service.rs 无 GPUI 依赖
- view 文件正确分离，mod.rs 不超过 500 行
- 不违反工作区架构不变量（I1-I4）

### 10.3 性能规范

- Profile 列表（>20 项）使用 `uniform_list`
- 文件列表（>一屏）使用 `uniform_list`
- 传输任务列表（>20 条）使用 `uniform_list`
- render 方法无 IO、锁操作、计算、排序
- 异步操作有 generation guard
- 终端滚动缓冲区保留最近 5000 行，不无限增长

### 10.4 UI 规范

- 分离式顶栏：顶层 `flex` 非 `flex_col`，左右独立列
- 左侧边栏包含 macOS 交通灯（三个 12px 圆点）
- Session Tab 栏在右侧区域顶部（不横贯左侧）
- 传输面板在 Tab 内容内，可展开收起
- Profile 编辑弹窗为居中模态 overlay，ESC 可关闭
- FTP 终端显示命令响应日志，发送/接收/错误用颜色区分
- 所有可交互元素有 hover 和 focus 状态

### 10.5 功能完整性

- Profile CRUD：创建、编辑（含协议切换时认证方式联动）、删除、测试连接
- Session：开多个 Session，切换 Tab，关闭 Session，各 Session 独立操作
- 文件浏览：列出目录、进入子目录、返回上级、创建文件夹、重命名、删除
- 文件传输：上传、下载、拖放上传、取消传输、查看日志（确认每秒至少一条）
- 终端：SSH 交互式 shell（ANSI 颜色、光标、输入），FTP 命令日志（发送/接收颜色区分、手动输入）
- 设置弹窗：表单验证、认证方式动态切换、保存和取消

### 10.6 测试验证

- `cargo check --workspace` 通过
- `cargo build -p qingqi-feature-ssh` 成功
- 新建、编辑、删除 Profile 流程
- SSH 连接并打开终端，执行基本命令
- FTP 连接并浏览文件树
- 上传/下载文件，展开传输日志确认格式正确
- 同时打开 3 个不同类型 Session，切换操作互不干扰

---

## 十一、关键决策记录

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

## 十二、Crate 目录结构

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

## 十三、迁移路径

1. 新建 `crates/qingqi-feature-ssh/`，添加 workspace 成员
2. 并行开发，旧插件 `qingqi-feature-ftp-sftp-ssh-client` 保持运行
3. 新 store 兼容读取旧 Profile schema
4. 功能完整后在 `crates/qingqi/src/features/registry.rs` 切换注册
5. 确认无问题后删除旧 crate
