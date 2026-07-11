# Qingqi 项目问题清单与修复工单

> 审计日期：2026-07-10
>
> 审计基线：`main@bef74d5d902a`，并包含审计时工作区中尚未提交的源码变更
>
> 工具链：`rustc 1.95.0`、`cargo 1.95.0`
>
> 审计环境：macOS Darwin 25.5.0 arm64
>
> 审计原则：按要求未读取、未引用项目已有文档；结论仅来自源码、清单、构建配置、CI 配置、静态分析和测试执行结果

> 目标读者：负责拆分任务的高级工程师，以及一次只执行一张明确工单的低级修复模型

## 0. 给修复模型的执行区

本节是本文档的主要交付物。后续审计章节用于提供证据和背景，不应要求低级模型一次读完。任务分配者每次只复制一张 `FIX-*` 工单，以及本节的通用规则给修复模型。

审计快照包含未提交源码，不能仅由顶部 commit SHA 完整重建。源码链接和行号只用于定位审计时证据；真正派单前，任务分配者必须先把需要保留的改动整合到一个明确 SHA，再让模型在该 SHA 上重新确认问题。

### 0.1 使用规则

1. 一次会话只执行一张工单，不得同时处理多个 ID。
2. 不得直接在审计工作区执行修复。每张工单必须从任务分配者指定的基线 SHA 建立独立 worktree/分支；发现工作区已有非本工单改动时立即停止，绝不能覆盖、回退或格式化这些改动。
3. 只允许修改工单“允许修改”列出的文件。发现必须修改其他文件时停止并报告，不得自行扩大范围。
4. 先运行工单的复现命令或按符号名阅读指定函数，确认问题仍存在，再编辑。若目标 SHA 已修复、代码结构已变化或无法复现，输出“工单已过期/需重新审计”并停止；不得为了匹配旧行号而改代码。
5. 必须新增或更新能在修复前失败、修复后通过的测试。只有工单明确标为“机械变更”时，才可使用工单指定的静态不变量检查代替新增测试；禁止删除测试、降低断言、添加 `#[ignore]` 或用 sleep 掩盖竞争。
6. 禁止用 `unwrap_or_default`、空 `match`、吞错误、扩大 `allow` lint 等方式让检查表面通过。
7. 禁止顺手重命名、重排模块、升级依赖或执行 workspace 全量格式化。
8. 当前全局 `cargo fmt --check` 和测试基线不是绿色。每张工单只对本任务负责，必须记录“新增失败”和“既有失败”的区别。
9. `L1` 工单可以交给低级模型独立完成；`L2` 可以实现但必须高级工程师逐行复核；`L3` 不得直接实现，只能在高级工程师先给出接口和状态机后继续。
10. 安全、文件覆盖、数据库迁移和 Windows FFI 即使测试通过，也必须人工复核。

### 0.2 修复模型固定输出格式

修复模型完成一张工单后必须按以下格式回答：

```text
工单：FIX-XXX
基线：开始修复时的 commit SHA
根因：用 1-3 句话说明，不复述需求
修改文件：逐项列出
实现：逐项列出实际改动
新增测试：测试名和覆盖场景
验证命令：命令 + 退出状态 + 关键结果
未完成/残余风险：没有则写“无”
范围检查：`git status --short` 和 `git diff --stat` 摘要，确认未修改范围外文件
```

### 0.3 可以直接复制给模型的提示词

```text
只执行下面这一张 Qingqi 修复工单。严格遵守“允许修改”和“禁止修改”。
先确认问题仍存在，再做最小实现并添加回归测试。不要处理其他告警，不要全量
格式化，不要改动用户已有未提交内容。开始时记录基线 SHA，完成后按工单文档规定
的固定格式报告，并给出 `git status --short` 与 `git diff --stat` 摘要。

<在这里粘贴一张 FIX 工单>
```

### 0.4 问题分派总表

| ID | 已确认问题 | 优先级 | 模型等级 | 前置依赖 | 处理方式 |
|---|---|---:|---:|---|---|
| FIX-001 | macOS 默认主题与测试断言不一致 | P0 | L1 | 无 | 直接修复 |
| FIX-002 | 文件日志 guard 在主循环前释放 | P1 | L1 | 无 | 直接修复 |
| FIX-003 | Input undo/redo 的 replace 与 selection 范围可能越界 | P1 | L1 | 无 | 直接修复 |
| FIX-004 | HTTP 代理默认监听所有网卡 | P0 | L1 | 无 | 直接修复 |
| FIX-005 | 下载文件名可逃逸下载目录 | P0 | L2 | 无 | 直接修复，安全复核 |
| FIX-006 | CA 私钥没有主动收紧权限 | P0 | L2 | 无 | 先修 Unix，Windows 转 DESIGN-006 |
| FIX-007 | 覆盖原图会先截断源文件 | P0 | L2 | 平台原子替换方案 | 直接修复，双平台复核 |
| FIX-008 | 下载续传未校验 Content-Range 且 200 计数错误 | P1 | L2 | FIX-005 | 直接修复 |
| FIX-009 | Input masked 状态未参与绘制 | P0/P1 | L2 | FIX-003 | 直接修复，安全复核 |
| FIX-010 | SSH 无条件信任主机密钥 | P0 | L2 | 无 | 先实现严格模式 |
| FIX-011 | 防窥非 Esc 关闭只改状态、不关 overlay | P1 | L2 | 无 | 跨 crate 生命周期修复，GPUI 复核 |
| FIX-012 | 托盘启动时未经选择请求公网 IP | P1 | L1 | 无 | 直接修复 |
| FIX-013 | 托盘事件每次向任务向量追加已完成 Task | P1 | L2 | 无 | 直接修复 |
| FIX-014 | Input disabled/read-only 没有覆盖所有写入口 | P1 | L2 | FIX-009 | 直接修复 |
| FIX-015 | 下载“重试次数”实际没有重新请求 | P1 | L2 | FIX-008 | 直接修复 |
| FIX-016 | API 查询参数和表单参数没有 URL 编码 | P1 | L1 | 无 | 直接修复 |
| FIX-017 | SSH/FTP 密码和私钥口令输入框未启用遮罩 | P0 | L1 | FIX-009 | 直接修复，安全复核 |
| FIX-018 | API Bearer、Basic 密码和 API key value 未启用遮罩 | P1 | L1 | FIX-009 | 直接修复，安全复核 |
| FIX-019 | 热键链路向 stdout 输出临时调试信息 | P2 | L1 | 无 | 直接修复 |
| FIX-020 | workspace 有 94 个 Rust 文件不满足 rustfmt | P1/P2 | L1 | 功能修复均已合入 | 独立机械提交 |
| FIX-021 | PR 与发布流程缺少基础质量门禁 | P0 | L2 | FIX-001/020，双平台基线绿色 | CI 复核后合入 |
| FIX-022 | 防窥自定义图片没有选择入口，保存的是旧快照 | P1 | L2 | FIX-011 | 直接修复，GPUI 复核 |
| DESIGN-001 | SSH/API 凭据迁入系统凭据库 | P0/P1 | L3 | 高级设计 | 不直接实现 |
| DESIGN-002 | 抓包 body 流式限额和并发请求关联 | P0/P1 | L3 | 高级设计 | 先写设计和失败测试 |
| DESIGN-003 | 下载 worker join、删除竞争和完整状态机 | P1 | L3 | FIX-008/015 | 不直接实现 |
| DESIGN-004 | API async client、真实取消和流式大文件 | P1 | L3 | FIX-016 | 不直接实现 |
| DESIGN-005 | GPUI TaskSupervisor 和阻塞 receiver bridge | P1 | L3 | 高级设计 | 不直接实现 |
| DESIGN-006 | Windows hook/ACL/进程树等平台资源 | P1 | L3 | Windows 环境 | 不直接实现 |
| DESIGN-007 | 统一数据库迁移器和历史数据恢复 | P1 | L3 | 高级设计 | 不直接实现 |
| DESIGN-008 | 剪贴板隐私、BlobStore 和安全删除 | P1 | L3 | 高级设计 | 不直接实现 |
| DESIGN-009 | 快速启动风险等级和跨平台执行语义 | P1 | L3 | 产品决策 | 不直接实现 |
| DESIGN-010 | 双平台签名、公证、SBOM 和发布凭据 | P1 | L3 | 发布环境 | 人工实施 |
| DESIGN-011 | 二维码敏感历史的默认策略和旧数据迁移 | P1 | L3 | 产品与安全决策 | 不直接实现 |

#### 0.4.1 P0/P1 审计项覆盖矩阵

这张表用于检查任务是否漏派。“直接工单”只解决可局部证明的部分，“后续设计”仍未完成时，不得把原审计项标记为整体关闭。

| 审计项 | 直接工单 | 后续设计/人工任务 | 整体关闭条件 |
|---|---|---|---|
| P0-01 SSH 身份与凭据 | FIX-009/010/017 | DESIGN-001 | 主机身份、屏幕显示、落盘秘密均关闭 |
| P0-02 HTTP 抓包安全 | FIX-004/006 | DESIGN-001/002 | 监听、CA key、body 限额、关联、脱敏均关闭 |
| P0-03 下载路径逃逸 | FIX-005 | 无 | 所有文件名来源和最终 join 均通过测试 |
| P0-04 原图非原子覆盖 | FIX-007 | 平台负责人确认 replace API | macOS/Windows 故障注入通过 |
| P0-05 质量门禁缺失 | FIX-001/020/021 | DESIGN-010 | PR 和 tag 构建均不能绕过基础检查 |
| P1-01 下载状态语义 | FIX-008/015 | DESIGN-003 | 续传、重试、删除和 shutdown 状态机均关闭 |
| P1-02 API 调试器可靠性 | FIX-016/018 | DESIGN-001/004 | 编码、秘密、取消、流式 I/O 均关闭 |
| P1-03 剪贴板隐私 | 无 | DESIGN-008 | 默认策略、BlobStore、删除补偿和迁移完成 |
| P1-04 Input 契约 | FIX-003/009/014 | 多行、搜索和附属控件另行拆单 | 编辑、遮罩、IME、Unicode 和绘制矩阵通过 |
| P1-05 后台任务监督 | FIX-013 | DESIGN-005 | owner、取消、join 和阻塞 bridge 完成 |
| P1-06 Windows hook | 无 | DESIGN-006 | Windows 真机 shutdown/资源测试通过 |
| P1-07 防窥功能 | FIX-011/022 | 无 | 所有关闭入口和图片配置均可用 |
| P1-08 数据迁移 | 无 | DESIGN-007 | 注册、事务、备份、坏数据和恢复完成 |
| P1-09 快速启动风险 | 无 | DESIGN-009 | 风险确认、quota 和平台 capability 完成 |
| P1-10 日志 guard | FIX-002 | 无 | 退出前日志完整且 guard 生命周期测试通过 |
| P1-11 托盘后台任务 | FIX-012 | DESIGN-005 | 公网请求需 opt-in，周期任务可停止并 join |
| P1-12 发布可信度 | FIX-021 | DESIGN-010 | 双平台签名、公证、校验和、SBOM 完成 |
| P1-13 二维码历史 | 无 | DESIGN-011 | 默认、提示、保留期、清除和旧数据迁移完成 |

### 0.5 推荐执行顺序

同一文件上的任务必须串行，不能让多个模型并行修改。

```text
第一批：FIX-001 -> FIX-002 -> FIX-004 -> FIX-012

第一批独立清理：FIX-019

第二批：FIX-005 -> FIX-008 -> FIX-015

第三批基础组件：FIX-003 -> FIX-009 -> FIX-014

第三批业务调用方：FIX-009 -> FIX-017
                  FIX-009 -> FIX-018

第四批：FIX-006 -> FIX-010 -> FIX-007 -> FIX-013 -> FIX-016

第四批防窥链路：FIX-011 -> FIX-022

第五批：全部功能分支合入并稳定后执行 FIX-020 -> FIX-021

第六批：由高级工程师逐项完成 DESIGN-001 至 DESIGN-011 的接口设计，
        再拆出新的 L1/L2 工单。
```

第一批是小范围确定性修复，用于验证低级模型的执行质量。第二、三批分别集中下载和输入组件，避免交叉冲突。`FIX-017` 与 `FIX-018` 在 `FIX-009` 合入后可以并行，但不能提前实施。第四批需要安全或平台复核。任何一批都不应以“顺便清理告警”为理由扩大 diff。

### 0.6 可直接执行的修复工单

#### FIX-001 修正平台默认主题测试

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，可稳定复现 |
| 优先级/等级 | P0 / L1 |
| 允许修改 | `crates/qingqi-app/src/app/theme_store.rs` |
| 禁止修改 | 生产默认主题逻辑、主题 JSON、其他测试 |
| 复现 | `cargo test -p qingqi-app theme_store --locked`，当前 2 个失败 |

**根因**

生产代码在 macOS 返回 `macOS Classic`，两个测试仍硬编码断言 `Default`。这是测试契约漂移，不应为了让测试通过而改回生产默认值。

**实施步骤**

1. 在 `persists_theme_name` 中把初始断言改为 `default_theme_name()`。
2. 在 `legacy_format_defaults_theme_name` 中做同样修改。
3. 保留后续自定义主题保存、重新加载断言不变。
4. 不增加 `cfg(target_os)` 重复逻辑，测试直接复用生产默认函数。

**必须测试**

- 原有 11 个 `theme_store` 测试全部通过。
- 非 macOS 编译时 `default_theme_name()` 仍为 `Default`，生产函数已有 cfg 即可，不复制测试实现。

**验收命令**

```bash
cargo test -p qingqi-app theme_store --locked
cargo check -p qingqi-app --all-targets --locked
git diff --check
```

#### FIX-002 保持日志 WorkerGuard 到应用退出

| 属性 | 内容 |
|---|---|
| 状态 | 已确认 |
| 优先级/等级 | P1 / L1 |
| 允许修改 | `crates/qingqi-app/src/app/runtime.rs` |
| 禁止修改 | 日志格式、日志级别、保留天数、其他启动逻辑 |
| 定位 | `run()` 对 `AppHost` 的解构，当前为 `_log_guard: _` |

**实施步骤**

1. 把字段解构为真正的局部绑定，例如 `_log_guard`，不能继续使用通配符 `_`。
2. 让绑定活到 plugin shutdown 和 database shutdown 完成之后，由函数作用域自然 drop。
3. 不要在 `app.run()` 前显式 `drop`，也不要泄漏为 `Box::leak` 或全局静态。
4. 添加一条简短注释说明该变量负责保持 non-blocking 日志 worker 存活。

**必须验证**

- 编译器没有 unused warning。
- 使用临时数据目录启动应用，启动后触发一条运行期日志，退出后日志文件中能找到启动和运行期记录。

**验收命令**

```bash
cargo check -p qingqi-app --all-targets --locked
cargo test -p qingqi-app --lib --locked
git diff --check
```

#### FIX-003 修复 Input undo/redo 范围钳制

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，undo/redo 的加法、replace range 和恢复 selection 均缺少统一钳制 |
| 优先级/等级 | P1 / L1 |
| 允许修改 | `crates/qingqi-ui/src/components/input/state.rs`，以及同目录新增的纯单元测试模块 |
| 禁止修改 | 输入组件渲染、移动算法、LSP stub、公开 API |
| 定位 | `undo()`、`redo()` 中根据 change 计算 replace range 的代码 |

**实施步骤**

1. `start` 先钳制到 `self.text.len()`。
2. `end` 使用 `start.saturating_add(replaced_len).min(self.text.len())`，不能写成 `start + len.min(total)`。
3. undo 和 redo 共用一个私有纯函数，例如 `clamped_replace_range(start, len, total) -> Range<usize>`，避免两个实现再次漂移。
4. replace 完成后，恢复的 selection/cursor 结束位置也必须用 `saturating_add(...).min(self.text.len())`；不能修完 replace 后留下越界 selection。
5. 不改变合法 change 的正常 selection 语义。

**必须新增测试**

- `start=5, len=10, total=8` 返回 `5..8`。
- `start > total` 返回 `total..total`。
- `len=usize::MAX` 不溢出。
- 正常范围保持不变。
- 构造异常长的 old/new text 后执行 undo 和 redo，断言 replace 不 panic，最终 selection 完全位于新文本长度内。

**验收命令**

```bash
cargo test -p qingqi-ui --lib --locked
cargo check -p qingqi-ui --all-targets --locked
git diff --check
```

#### FIX-004 HTTP 抓包默认仅监听回环地址

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，当前固定 `0.0.0.0` |
| 优先级/等级 | P0 / L1 |
| 允许修改 | `crates/qingqi-feature-http-capture/src/engine.rs`、`crates/qingqi-feature-http-capture/tests/proxy_runtime.rs` |
| 禁止修改 | 新增远程代理模式、认证协议、UI 大改、CA 逻辑 |
| 定位 | `CaptureEngine::start` 创建 `SocketAddr` 的位置 |

**实施步骤**

1. IPv4 默认地址改为 `127.0.0.1`。
2. 不要通过 UI 文案假装支持局域网访问；远程代理另开 DESIGN 工单。
3. 更新占用端口测试，让它占用与生产一致的 loopback 地址。
4. 如果状态结构会保存监听地址，断言其中的 IP 为 loopback。

**必须新增测试**

- 启动后可从 `127.0.0.1` 连接。
- `ProxyState::Running` 或等价状态报告 loopback。
- 端口被 loopback listener 占用时启动返回错误。

**验收命令**

```bash
cargo test -p qingqi-feature-http-capture --locked
cargo check -p qingqi-feature-http-capture --all-targets --locked
git diff --check
```

#### FIX-005 净化下载文件名并防止目录逃逸

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，可由 `%2e%2e%2f` 构造 |
| 优先级/等级 | P0 / L2，必须安全复核 |
| 允许修改 | `crates/qingqi-feature-download-manager/src/model.rs`、`service.rs` 及其测试 |
| 禁止修改 | 下载调度、UI、数据库 schema、Cargo 依赖 |
| 定位 | `extract_file_name`、`guess_file_name`、`resolve_save_path_in_dir` |

**目标行为**

所有来源的文件名最终只能是一个普通文件名组件，不能包含父目录、根目录、盘符、正反斜杠、NUL、控制字符或 Windows 设备名。

**实施步骤**

1. 新增唯一入口 `sanitize_file_name(raw: &str) -> String`，不要在三个调用点分别实现。
2. percent decode 之后再净化；Content-Disposition 和 URL 路径都必须走该函数。
3. 同时把 `/` 和 `\\` 视为分隔符，只保留最后一个非空组件。
4. `.`、`..`、空值回退为 `download`。
5. 去除控制字符，替换 Windows 非法字符 `< > : " / \\ | ? *`，移除尾随空格和点。
6. 对大小写不敏感的 `CON`、`PRN`、`AUX`、`NUL`、`COM1..9`、`LPT1..9` 加安全前缀。
7. `resolve_save_path_in_dir` 再次调用净化函数，形成防御纵深。
8. 保留现有同名文件编号逻辑，本工单不重写并发命名。

**必须新增表驱动测试**

```text
../secret.txt
%2e%2e%2fsecret.txt
..%5csecret.txt
/etc/passwd
C:\\Windows\\win.ini
\\\\server\\share\\a.txt
CON
nul.txt
name\0.txt
normal file.zip
中文图片.png
```

每个结果必须满足：非空、不是 `.`/`..`、`Path::new(result).components().count() == 1`，并且 `dir.join(result).parent() == dir`。

**验收命令**

```bash
cargo test -p qingqi-feature-download-manager --lib --locked
cargo check -p qingqi-feature-download-manager --all-targets --locked
git diff --check
```

#### FIX-006 收紧 CA 私钥文件权限

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，普通 `fs::write` 不主动收紧权限 |
| 优先级/等级 | P0 / L2，必须安全复核 |
| 允许修改 | `crates/qingqi-feature-http-capture/src/certificate.rs` 及其测试 |
| 禁止修改 | CA 算法、证书主题、系统信任安装、Cargo 依赖 |
| 本工单平台 | Unix/macOS；Windows ACL 留给 DESIGN-006 |

**实施步骤**

1. 新增私有函数 `write_private_key(path, pem)`，证书仍可使用普通公开权限，私钥不能。
2. Unix 使用 `OpenOptionsExt::mode(0o600)` 创建；写完后再用 `set_permissions(0o600)` 修复已存在文件权限。
3. 写入错误必须返回，禁止只记录 warning 后继续启动代理。
4. 不要把私钥内容写入日志或错误信息。
5. 现有私钥加载时也检查权限；权限过宽时至少自动收紧并记录不含路径秘密的 warning。

**必须新增测试**

- 新生成 key 的 mode 与 `0o777` 后等于 `0o600`。
- 预先创建为 `0o644` 的测试 key 经修复后为 `0o600`。
- 写入不可写目录返回错误。

**验收命令**

```bash
cargo test -p qingqi-feature-http-capture certificate --locked
cargo check -p qingqi-feature-http-capture --all-targets --locked
git diff --check
```

#### FIX-007 覆盖原图改为同目录临时文件加原子替换

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，当前先 `File::create(source)` |
| 优先级/等级 | P0 / L2，必须高级和双平台复核 |
| 允许修改 | `crates/qingqi-feature-image-compress/src/service.rs`、必要的平台原子替换 helper、对应 Cargo manifest 和测试 |
| 禁止修改 | 压缩参数、UI 布局、输出命名、其他文件写入逻辑 |
| 前置条件 | 高级工程师先确认 macOS/Windows 的 replace API；未确认则停止 |

**目标行为**

编码、flush、校验或替换任一步失败时，原文件字节必须保持不变。

**实施步骤**

1. 非覆盖模式保持现状。
2. 覆盖模式在源文件同目录创建唯一临时文件，确保同一文件系统。
3. `write_image` 只写临时文件；完成后 flush 并 `sync_all`。
4. 重新用 `ImageReader` 解码临时文件，确认格式可读且尺寸与输入一致。
5. 保留原文件权限；使用经平台负责人确认的原子 replace API 替换。
6. 任何失败都删除临时文件并返回错误，绝不能先删除原文件。
7. 不允许用“copy temp -> source”冒充原子替换。

**必须新增故障测试**

- 编码前错误、编码中错误、校验失败、replace 失败时原文件 SHA-256 不变。
- 成功后新文件可解码，尺寸一致，无临时文件残留。
- 覆盖与非覆盖路径分别测试。

**验收命令**

```bash
cargo test -p qingqi-feature-image-compress --lib --locked
cargo check -p qingqi-feature-image-compress --all-targets --locked
git diff --check
```

#### FIX-008 修正下载续传范围和进度

| 属性 | 内容 |
|---|---|
| 状态 | 已确认 |
| 优先级/等级 | P1 / L2 |
| 允许修改 | `crates/qingqi-feature-download-manager/src/service.rs` 及测试 |
| 禁止修改 | 数据库 schema、ETag 持久化、下载 UI、重试逻辑 |
| 前置依赖 | FIX-005 |

**实施步骤**

1. 把 `parse_content_range` 改为解析 `start`、`end`、`total` 的结构体，不再只返回 total。
2. 发送 Range 前读取本地文件长度；不存在或长度与数据库进度不一致时，从 0 开始且不发 Range。
3. 收到 206 时要求 `Content-Range.start == initial_downloaded`，否则返回明确错误，禁止 append。
4. 初始进度大于 0 但收到 200 时，创建/截断文件并把本次 `downloaded` 和 atomic progress 都重置为 0。
5. 206 append 前再次确认文件长度等于起点。
6. `file_size` 使用 total，不把剩余 Content-Length 当总大小。

**必须新增测试**

- `bytes 100-199/200` 正确解析。
- 起点不匹配拒绝写入。
- 续传请求收到 200 后文件内容只等于新响应，进度从 0 计算。
- 本地文件缺失但数据库进度非 0 时不 append。
- malformed/`*` range 有明确结果。

**验收命令**

```bash
cargo test -p qingqi-feature-download-manager --lib --locked
cargo check -p qingqi-feature-download-manager --all-targets --locked
git diff --check
```

#### FIX-009 让 masked 状态真正控制 Input 绘制

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，paint 始终读取原文 |
| 优先级/等级 | P0/P1 / L2，必须安全和 GPUI 复核 |
| 允许修改 | `crates/qingqi-ui/src/components/input/element.rs`、`state.rs`、必要的同目录纯 helper 和测试 |
| 禁止修改 | disabled/read-only、搜索、LSP、多行重构、SSH 业务代码 |
| 前置依赖 | FIX-003 |

**目标行为**

`InputState.masked == true` 时，屏幕绘制、selection 宽度、cursor 宽度和 IME 标记宽度都基于掩码文本，不出现原字符；底层 value 保持原文供认证使用。

**实施步骤**

1. paint 读取 state 时同时读取 `masked`。
2. 新增纯 helper，把完整原文或“截至某个原文字节 offset 的前缀”映射为固定掩码字符。
3. offset 必须先钳制到 UTF-8 char boundary，禁止直接用原文字节 offset 切掩码字符串。
4. 正文、selection start/end、cursor、IME start/end 统一走 helper，不能只替换正文绘制。
5. placeholder 不遮罩；底层 Rope 和 `value()` 不改变。
6. 给 `InputState` 增加只读查询 `is_masked() -> bool`，供调用方回归测试确认配置；不得暴露底层文本或新增第二个状态来源。
7. 本工单不实现眼睛按钮，也不决定密码复制策略。

**必须新增纯测试**

- ASCII、中文、emoji、组合字符不会 panic。
- 任意合法原文 offset 得到的显示前缀不含原字符。
- masked=false 返回原始显示文本。
- offset 位于多字节字符中时安全向前钳制。

**验收命令**

```bash
cargo test -p qingqi-ui --lib --locked
cargo check -p qingqi-ui --all-targets --locked
git diff --check
```

#### FIX-010 SSH 默认严格校验 known_hosts

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，当前无条件 `Ok(true)` |
| 优先级/等级 | P0 / L2，必须安全复核 |
| 允许修改 | `crates/qingqi-feature-ssh/src/protocol/ssh.rs` 及该 crate 测试 |
| 禁止修改 | 凭据存储、认证方法、连接池、UI 确认弹窗、russh 版本 |
| 本工单范围 | 只做安全的“已知匹配接受；未知和变更拒绝”严格模式 |

**实施步骤**

1. 给 `Handler` 增加当前 host 和 port，并在 `connect()` 构造 handler 时传入。
2. 调用 russh 0.54.5 已提供的 `russh::keys::check_known_hosts(host, port, key)`，禁止自己解析 OpenSSH 文件。
3. 返回 `Ok(true)` 仅限已记录且完全匹配。
4. 未知 host 返回 `Ok(false)` 并记录指纹；key changed 错误向上传播，不能回退为接受。
5. 日志只包含 endpoint、算法和 SHA-256 指纹，不包含私钥或凭据。
6. UI 首次确认和 `learn_known_hosts` 是后续高级设计，本工单不能自动学习未知 key。

**必须新增测试**

- 临时 known_hosts 中匹配 key 返回 true。
- 未知 host 返回 false。
- 同算法不同 key 返回 KeyChanged 错误。
- 非 22 端口使用 `[host]:port` 语义。

如标准路径难以测试，可以把调用封装为接受显式 path 的私有 helper，生产传标准路径，测试传临时路径；仍必须使用 russh 的 `check_known_hosts_path`。

**验收命令**

```bash
cargo test -p qingqi-feature-ssh --lib --locked
cargo check -p qingqi-feature-ssh --all-targets --locked
git diff --check
```

#### FIX-011 统一关闭防窥 overlay

| 属性 | 内容 |
|---|---|
| 状态 | 已确认 |
| 优先级/等级 | P1 / L2，必须 GPUI 生命周期复核 |
| 允许修改 | `crates/qingqi-plugin/src/plugin.rs`、`crates/qingqi-app/src/app/window_controller.rs`、`crates/qingqi-feature-anti-peeping/src/plugin.rs` 及对应测试 |
| 禁止修改 | 其他 `WindowView` 实现、自定义图片 UI、窗口视觉样式、manifest、插件 manager 锁模型 |
| 定位 | `WindowView::on_close`、`PluginWindow::Drop`、`close_idle`、`AntiPeepingView::on_close`、Esc 路径 |

**实施约束**

当前 `PluginWindow::Drop` 调用的 `WindowView::on_close` 没有 `App` 参数，不能从这里更新 GPUI window handle。GPUI 0.2.2 的 `Context::on_release` 会提供 `&mut App`，本工单必须利用该生命周期回调，不能使用全局裸指针、伪造 Context 或固定延迟。

**实施步骤**

1. 在 `WindowView` 增加一个带 `&mut App` 的向后兼容关闭 hook，例如默认实现调用旧 `on_close()`；不得一次修改所有现有 view 实现。
2. `PluginWindow` 创建时通过 `cx.on_release(...)` 注册关闭回调并 `detach`，在实体释放且仍有 `App` 时只执行一次 app-aware hook。
3. 给 `PluginWindow` 增加幂等关闭标记/私有函数，避免 on-release 与 `Drop` 重复调用 view 关闭和 `close_idle_plugin`。`Drop` 只保留无 App 的兜底，不得成为正常关闭路径。
4. `AntiPeepingView` 覆盖 app-aware hook：先把 active 设为 false，再调用现有 `close_overlays` drain 全部 handle。
5. overlay 自身 Esc 路径也复用同一个幂等清理 helper；不能维护第二套状态转换。
6. 某个 handle 更新失败时继续 drain 其余 handles，并记录不含用户图片路径的 debug/warn。

**必须新增测试**

- 用 `#[gpui::test]` 证明 `PluginWindow` entity release 会调用 app-aware hook，且调用次数恰好为 1。
- 旧式只实现 `on_close()` 的测试 view 仍收到一次回调，证明兼容默认实现有效。
- 打开后 Esc 和关闭插件窗口都关闭全部 handles、清空 active；连续关闭两次不 panic。
- 多显示器中一个 handle 更新失败，其他 handle 仍被 drain。

**验收命令**

```bash
cargo test -p qingqi-feature-anti-peeping --lib --locked
cargo check -p qingqi-feature-anti-peeping --all-targets --locked
git diff --check
```

#### FIX-012 公网 IP 改为显式开启

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，start_background 会立即访问 ipify |
| 优先级/等级 | P1 / L1 |
| 允许修改 | `crates/qingqi-feature-tray/src/settings.rs`、`service.rs`、`settings_view.rs` 及测试 |
| 禁止修改 | 网速采样算法、托盘布局、第三方 endpoint |

**实施步骤**

1. 在 `NetworkSpeedSettings` 增加默认 false 的 `public_ip_enabled`，反序列化旧配置时必须得到 false。
2. `start_background` 仅在该开关为 true 时调用 `refresh_ip_cache_background`。
3. popup 打开时也遵守开关；关闭后清空已缓存公网 IP。
4. 设置页增加明确的二元开关，文案说明会访问 `api.ipify.org`。
5. 本地 IP 检测不受该开关影响。

**必须新增测试**

- 默认和旧配置都不启用公网 IP。
- false 时不调用 fetch。为此注入私有 fetch function/trait，不要在测试访问外网。
- true 时只触发一次并遵守 `ip_refreshing`。
- 从 true 切为 false 清除缓存。

**验收命令**

```bash
cargo test -p qingqi-feature-tray --lib --locked
cargo check -p qingqi-feature-tray --all-targets --locked
git diff --check
```

#### FIX-013 固定托盘事件任务数量

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，当前每次事件递归 rearm 并 push Task |
| 优先级/等级 | P1 / L2 |
| 允许修改 | `crates/qingqi-app/src/app/background.rs` 及测试 |
| 禁止修改 | 其他 listener、托盘 action 语义、TaskSupervisor 架构 |
| 本工单范围 | 只消除 task 向量随事件增长；阻塞 recv 留给 DESIGN-005 |

**实施步骤**

1. `start_tray_events` 只创建两个长期 task：icon loop 和 menu loop。
2. 每个 task 内部循环等待事件、切回 UI 线程处理，然后继续等待。
3. 删除 `arm_tray_*` 的递归 rearm 和事件级 `push_background_task`。
4. task 向量中始终只保留这两个 owner handle。
5. receiver 返回 None 时循环退出，不忙轮询。

**必须新增测试或可测试计数**

- 连续注入多次 icon/menu 事件后 supervisor task 数不增加。
- receiver 关闭后 task 退出。
- 现有重复点击过滤和 action mapping 测试继续通过。

**验收命令**

```bash
cargo test -p qingqi-app background --locked
cargo check -p qingqi-app --all-targets --locked
git diff --check
```

#### FIX-014 统一 Input disabled/read-only 写保护

| 属性 | 内容 |
|---|---|
| 状态 | 已确认 |
| 优先级/等级 | P1 / L2 |
| 允许修改 | `crates/qingqi-ui/src/components/input/input.rs`、`state.rs` 及测试 |
| 禁止修改 | masked 绘制、清空/眼睛按钮 UI、多行布局、LSP |
| 前置依赖 | FIX-009 |

**实施步骤**

1. 增加唯一私有判断 `can_edit() -> bool`，结果为 `!disabled && !read_only`。
2. 所有写操作入口最前面调用它：backspace、delete、cut、paste、undo、redo、indent/outdent、IME replace/mark、Enter 插入换行和 clean。
3. copy、selection、方向移动和只读聚焦仍允许；disabled 不应获得编辑焦点。
4. 提供明确的 `set_disabled`、`set_read_only` API 并通知重绘。
5. `Input::disabled` 不能只改 builder 外观。若无法安全同步 entity state，应删除误导 builder API并更新调用点；不要保留两份真相。

**必须新增表驱动测试**

- 每个写动作在 normal/read-only/disabled 三种模式下的结果。
- read-only 可 copy 和 select，不能 cut/paste。
- disabled 鼠标点击不获取编辑焦点。
- IME 回调不能绕过写保护。

**验收命令**

```bash
cargo test -p qingqi-ui --lib --locked
cargo check -p qingqi-ui --all-targets --locked
git diff --check
```

#### FIX-015 实现真实、可计数的下载重试

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，当前设置 Pending 后仍被外层写为 Failed |
| 优先级/等级 | P1 / L2 |
| 允许修改 | `crates/qingqi-feature-download-manager/src/service.rs` 及测试 |
| 禁止修改 | UI、数据库 schema、续传协议、任务删除 |
| 前置依赖 | FIX-008 |

**目标语义**

`retry_limit` 表示首次请求失败后最多额外尝试的次数。只有网络错误、408、425、429 和 5xx 可自动重试。取消、暂停、4xx 永久错误和文件写入错误不重试。

**实施步骤**

1. 删除把 `downloaded` 当 attempts 的逻辑。
2. 把单次 HTTP 请求/写入结果和重试循环分开，attempt 使用局部整数。
3. 每次失败后检查 cancel/pause，再等待有上限的指数退避；测试中可注入 sleeper，禁止真实长 sleep。
4. 重试期间任务保持 Downloading，不在每次尝试之间写 Pending/Failed。
5. 最后一次失败才写 Failed；成功只写一次 Completed。
6. 记录 attempt、max_attempts、status 的结构化日志，不记录 Cookie/header。

**必须新增本地 server 测试**

- 前两次 500、第三次 200，实际请求 3 次并成功。
- `retry_limit=0` 只请求 1 次。
- 404 不重试。
- transport error 可重试。
- 退避期间取消后不再请求。

**验收命令**

```bash
cargo test -p qingqi-feature-download-manager --lib --locked
cargo check -p qingqi-feature-download-manager --all-targets --locked
git diff --check
```

#### FIX-016 使用结构化 API 编码查询参数和表单

| 属性 | 内容 |
|---|---|
| 状态 | 已确认，当前直接拼接 `key=value` |
| 优先级/等级 | P1 / L1 |
| 允许修改 | `crates/qingqi-feature-api-debugger/src/service.rs`、必要的 crate Cargo manifest 和测试 |
| 禁止修改 | async 化、取消、history、UI、代码生成器的其他语言格式 |

**实施步骤**

1. 构造实际请求 URL 时使用 `url::Url` 的 query pair API，或 reqwest 的结构化 `.query()`；禁止手写 `?`/`&` 拼接。
2. 普通 query 和 query-located auth 使用同一编码路径。
3. `application/x-www-form-urlencoded` 使用成熟 serializer，例如 `url::form_urlencoded::Serializer` 或 reqwest `.form()`。
4. 保持 path segment 与 query value 区分，不能对整个 URL 做一次 percent encode。
5. cURL/code snippet 展示必须和真实 URL 一致；如果本工单需要修改 code_gen，先报告扩大文件范围。

**必须新增测试**

```text
key="a b"       -> a+b 或 a%20b，按选定 serializer 固定
key="a&b"       -> 不产生第二个参数
value="x=y"     -> 等号被编码
value="中文"    -> UTF-8 percent encoding
value="+%#?"    -> round-trip 后值不变
空值             -> 参数存在且值为空
```

测试应把生成 URL 重新交给 `Url` 解析并比较 pairs，不只比较脆弱字符串。

**验收命令**

```bash
cargo test -p qingqi-feature-api-debugger --lib --locked
cargo check -p qingqi-feature-api-debugger --all-targets --locked
git diff --check
```

#### FIX-017 为 SSH/FTP 凭据输入启用遮罩

| 属性 | 内容 |
|---|---|
| 状态 | 已确认；密码与私钥口令当前按普通文本输入初始化 |
| 优先级/等级 | P0 / L1，必须安全复核 |
| 允许修改 | `crates/qingqi-feature-ssh/src/view/mod.rs` 及该 crate 内对应测试 |
| 禁止修改 | `qingqi-ui`、凭据持久化、认证协议、表单布局、眼睛按钮、日志 |
| 前置依赖 | FIX-009 已合入，并且 `InputState::is_masked()` 可用 |
| 定位 | `SshView::ensure_form_inputs` 中 `form_password` 和 `form_private_key_passphrase` 的初始化 |

**目标行为**

SSH 密码、FTP/FTPS 密码和 SSH 私钥口令始终以遮罩文本绘制；用户名、私钥路径和其他普通字段保持明文。遮罩只影响显示，保存和认证读取到的值必须保持原样。

**实施步骤**

1. 在 `ensure_form_inputs` 中创建 `form_password` 时调用 `set_masked(true, window, cx)`。
2. 创建 `form_private_key_passphrase` 时执行相同操作。
3. 可以提取一个仅在本模块使用的 `masked_form_input` helper，避免两个初始化分支漂移；不要为所有表单字段增加 `bool` 参数。
4. `fill_form_from_profile` 和 `reset_form` 只能更新 value，不能把 masked 状态重置为 false。
5. 不要遮罩 `form_username`、`form_private_key_path`、host 或 note。
6. 不要在本工单实现“显示密码”按钮，也不要修改明文数据库迁移；后者属于 DESIGN-001。

**必须新增验证**

- 使用 GPUI 0.2.2 已提供的 `#[gpui::test]` 和 `TestAppContext` 创建测试窗口；测试提取出的 `masked_form_input`，或在可控的 view fixture 中调用 `ensure_form_inputs`。
- 断言两个敏感 entity 的 `is_masked()` 为 true。
- 断言 `form_username` 和 `form_private_key_path` 的 `is_masked()` 为 false。
- 分别 `reset_value` 一个包含中文和 emoji 的秘密，断言 `value()` 原样返回，证明本工单没有破坏认证值。
- 禁止改成源码字符串匹配测试或只提交手工截图；如果 `#[gpui::test]` 无法编译，保留失败输出并停止扩大改动范围。

**人工冒烟**

分别打开 SSH 密码、SSH 私钥和 FTP 三种配置表单，输入可辨识字符串；屏幕上不得出现原字符，保存后仍能读取同一底层值。

**验收命令**

```bash
cargo test -p qingqi-feature-ssh --lib --locked
cargo check -p qingqi-feature-ssh --all-targets --locked
git diff --check
```

#### FIX-018 为 API 认证凭据输入启用遮罩

| 属性 | 内容 |
|---|---|
| 状态 | 已确认；三个认证秘密字段均由普通 `single_input` 创建 |
| 优先级/等级 | P1 / L1，必须安全复核 |
| 允许修改 | `crates/qingqi-feature-api-debugger/src/view/mod.rs`、`view/types.rs` 及该 crate 内对应测试 |
| 禁止修改 | `qingqi-ui`、请求编码、凭据持久化、环境变量编辑器、响应视图、眼睛按钮 |
| 前置依赖 | FIX-009 已合入，并且 `InputState::is_masked()` 可用 |
| 定位 | `ApiDebuggerView::new` 中五个 `auth_*_input` 的初始化 |

**目标行为**

Bearer token、Basic Auth 密码和 API key value 以遮罩文本绘制；Basic 用户名和 API key name 保持明文。切换认证类型、加载已有请求和同步服务更新后，敏感 entity 仍保持 masked。

**实施步骤**

1. 在 `view/types.rs` 增加 `masked_single_input`，复用现有 `input_state` 创建逻辑并在返回前调用 `set_masked(true, window, cx)`。
2. `auth_bearer_input`、`auth_basic_pass_input`、`auth_apikey_value_input` 改用该 helper。
3. `auth_basic_user_input`、`auth_apikey_name_input` 继续使用普通 `single_input`。
4. 检查 `editor.rs` 中加载认证表单的 `reset_value` 路径，确保它只更新 value，不重新创建未遮罩 entity。
5. 不要遮罩所有 header、query 或环境变量；这些字段可能含秘密，但需要独立的数据分类设计。
6. 不要改变实际 Authorization/API key 请求值，也不要在日志或失败断言中打印测试 token。

**必须新增验证**

- 使用 GPUI 0.2.2 已提供的 `#[gpui::test]` 和 `TestAppContext` 创建测试窗口，分别调用 `single_input` 与 `masked_single_input`。
- 构造五个认证输入，断言三个秘密字段 `is_masked() == true`，两个标识字段为 false。
- 对 Bearer、Basic password、API key value 分别执行一次 `reset_value`，再次断言 masked 状态仍为 true。
- 断言 `value()` 返回输入原值，且 `auth_form_inputs()`/现有序列化结果未把掩码字符写进请求。
- 禁止用检查源文件是否包含 `set_masked` 的伪测试代替行为测试；如果 `#[gpui::test]` 无法编译，保留失败输出并停止扩大改动范围。

**人工冒烟**

依次选择 Bearer Token、Basic Auth、API Key，确认秘密字段不显示原文，用户名和 key name 仍可读；发送到本地回显服务时认证值必须与输入一致。

**验收命令**

```bash
cargo test -p qingqi-feature-api-debugger --lib --locked
cargo check -p qingqi-feature-api-debugger --all-targets --locked
git diff --check
```

#### FIX-019 清除热键链路的临时 stdout 调试输出

| 属性 | 内容 |
|---|---|
| 状态 | 已确认；四个生产文件中共有 8 处 `!!!` 临时输出 |
| 优先级/等级 | P2 / L1，机械变更 |
| 允许修改 | `crates/qingqi-platform/src/hotkey.rs`、`crates/qingqi-app/src/app/background.rs`、`crates/qingqi-app/src/app/runtime.rs`、`crates/qingqi-app/src/core/shortcut.rs` |
| 禁止修改 | 热键注册/分发行为、线程模型、日志初始化、API 调试器生成的示例代码、其他告警 |

**实施步骤**

1. 删除所有包含 `!!!` 的 `println!`，不得保留为 stdout/stderr 输出。
2. 已有等价 `tracing` 记录时直接删除重复输出；没有等价记录且确有诊断价值时改为 `tracing::debug!` 或逐事件的 `trace!`。
3. 日志字段只记录数量、hotkey ID 和状态，不在 info 级别打印完整注册表、用户配置或错误 map。
4. `QINGQI_TEST_LAUNCHER` 自动打开行为保留，只把打印改为 debug 事件。
5. 不调整 `start_hotkey_events` 生命周期；该问题属于 DESIGN-005。

**静态不变量与回归验证**

- 对四个允许文件执行 `rg -n '!!!'` 必须零结果。
- `register_global_hotkeys` 的成功、部分失败和 manager 创建失败测试保持通过。
- `dispatch_global` 的已知/未知 ID 返回值保持不变。
- 不要求为删除临时输出新增脆弱的 stdout 捕获测试。

**验收命令**

```bash
rg -n '!!!' crates/qingqi-platform/src/hotkey.rs crates/qingqi-app/src/app/background.rs crates/qingqi-app/src/app/runtime.rs crates/qingqi-app/src/core/shortcut.rs
cargo test -p qingqi-platform --lib --locked
cargo test -p qingqi-app shortcut --locked
cargo check -p qingqi-app -p qingqi-platform --all-targets --locked
git diff --check
```

第一条命令的正确结果是退出码 1 且无匹配；修复模型必须在报告中明确这是“零结果”，不能误报为检查失败。

#### FIX-020 建立 workspace rustfmt 绿色基线

| 属性 | 内容 |
|---|---|
| 状态 | 已确认；审计工作区 `cargo fmt --all -- --check` 报告 94 个文件，但执行时必须重新计数 |
| 优先级/等级 | P1/P2 / L1，纯机械变更 |
| 允许修改 | rustfmt 实际改动的已跟踪 `*.rs` 文件 |
| 禁止修改 | Markdown、Cargo manifest、lockfile、workflow、生成资源；任何手工语义修改 |
| 前置条件 | 所有功能修复已合入；独立 worktree 完全干净；没有其他模型并行修改 Rust 文件 |

**实施步骤**

1. 记录基线 SHA、`git status --short` 和修复前 `cargo fmt --all -- --check` 退出码。
2. 只运行一次 `cargo fmt --all`；禁止手工顺便重命名、删 warning 或改注释内容。
3. 用 `git diff --name-only` 确认所有变更文件都以 `.rs` 结尾，任何其他文件出现即停止。
4. 审查 `git diff --stat`；若文件数相对复现结果异常增加，检查是否用了不同 toolchain。
5. 运行 workspace check 和 test。格式提交必须独立，不能与功能修复混在同一 commit。

**静态不变量与验收命令**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
git diff --check
git status --short
git diff --stat
```

`cargo fmt --all -- --check` 必须退出 0。若 check/test 出现基线不存在的新失败，不能以“只是格式化”忽略；停止并由高级工程师检查宏、注释或 cfg 附近的差异。

#### FIX-021 增加 PR 与发布基础质量门禁

| 属性 | 内容 |
|---|---|
| 状态 | 已确认；当前只有 tag/manual release workflow，直接构建上传 |
| 优先级/等级 | P0 / L2，必须 CI 与双平台复核 |
| 允许修改 | `.github/workflows/quality.yml`、`.github/workflows/release.yml` |
| 禁止修改 | Rust 源码、Cargo 依赖、签名凭据、分支保护设置、用 `continue-on-error` 放过失败 |
| 前置依赖 | FIX-001/020；macOS 本地 fmt/check/test 绿色；Windows 失败必须在 CI 中真实解决或报告 |

**目标行为**

每个 PR 自动执行基础检查；tag 发布在目标平台检查失败时不得上传产物。初始门禁不宣称已经完成签名、SBOM、`cargo audit` 或 Clippy warning 清零，这些仍由 DESIGN-010 和后续告警批次处理。

**实施步骤**

1. 新增 `quality.yml`，触发条件至少包含 `pull_request` 和对 `main` 的 push。
2. workflow 顶层设置 `permissions: contents: read`，增加按 workflow/ref 的 concurrency，并对更新后的 PR 取消旧任务。
3. Ubuntu job 只运行 `cargo fmt --all -- --check`，避免为 GPUI check 临时堆系统依赖。
4. macOS job 使用仓库 `rust-toolchain.toml` 固定的 1.95.0，执行 workspace check、Clippy 和 test；Clippy 本阶段不加 `-D warnings`，但任何 deny/error 都必须失败。
5. Windows job复用 release 中查找 `fxc.exe` 并设置 `GPUI_FXC_PATH` 的步骤，执行 workspace check 和 test。
6. 三个平台所有 Cargo 命令都带 `--locked`；禁止自动更新 lockfile或运行浮动 `rustup update`。
7. 在 `release.yml` 的 macOS/Windows build job 中，把对应 check/test 放在打包前；任一步失败时后续 bundle、archive 和 upload 不得执行。
8. 复用现有 `actions/checkout@v4`、`actions/cache@v4`，本工单不引入无固定主版本的任意第三方 action。
9. 仓库分支保护必须由管理员在 GitHub 设置 required checks；修复模型只在最终报告列为人工待办，不能声称已通过代码完成。

**必须验证**

- YAML 可被 `actionlint` 解析；环境没有 actionlint 时不得临时下载未校验二进制，改由 GitHub draft PR 验证。
- 创建一个只改空白的测试分支，quality jobs 全绿。
- 在临时验证分支故意制造 rustfmt 失败，确认 fmt job 红灯；随后撤销该临时验证 commit，不得合入。
- tag workflow 中检查步骤位于上传前，且没有 `if: always()`、`continue-on-error` 或 `|| true` 绕过。
- GitHub 上 macOS 与 Windows 实际运行绿色后工单才算完成，本地 YAML 检查不能替代。

**本地验收命令**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
git diff --check
```

#### FIX-022 补齐防窥自定义图片选择与保存闭环

| 属性 | 内容 |
|---|---|
| 状态 | 已确认；设置页只显示创建 view 时的 `draft_path`，没有输入/选择入口，保存永远使用旧快照 |
| 优先级/等级 | P1 / L2，必须 GPUI 复核 |
| 允许修改 | `crates/qingqi-feature-anti-peeping/src/plugin.rs`、该 crate `Cargo.toml` 及测试 |
| 禁止修改 | overlay 关闭状态机、窗口样式重构、图片裁剪编辑、全局配置格式、其他 feature crate |
| 前置依赖 | FIX-011；必须基于它合入后的 `plugin.rs` 实施，不能覆盖其生命周期修复 |

**目标行为**

用户可以选择或清除一张本地图片，看到当前草稿和明确错误，点击保存后更新运行时与持久化配置；取消选择不改变现有配置，重启后空值不会被解释成 `Some("")`。

**实施步骤**

1. 把不可变 `draft_path: String` 改成能被 click 回调更新且能触发窗口刷新的单一草稿状态；不要同时保留多个互不关联的 String。
2. 使用项目已有模式调用原生文件选择器，只允许单文件；按钮使用现有 `Button` 和 `FolderOpen` 图标，不手写 SVG。
3. 选择后先验证路径是普通文件，并用 `image` 解码验证支持的图片内容；不能只信扩展名。
4. 取消文件对话框时不清空草稿；无效/不可读图片显示错误且不能覆盖已保存设置。
5. 增加清除命令，清除只改变草稿；用户点击保存后才把运行时 `image_path` 置为 None。
6. `save_custom_image` 改为返回 `Result`，序列化、建目录、写文件失败必须显示给用户，不能继续 `unwrap_or_default` 或只写 warning。
7. 空值持久化为明确的 `null` 或缺省值；`load_custom_image` 对缺失、null、空白字符串都返回 None。
8. 保存成功后刷新窗口；下一次打开 overlay 使用新图。当前已打开 overlay 是否即时刷新必须写测试或在工单报告中明确产品语义。
9. 非 UTF-8 路径如果现有配置格式无法无损表示，必须显示“不支持该路径”的错误并停止，禁止 `to_string_lossy` 静默改路径。
10. 如需依赖，只能在本 crate manifest 引用 workspace 已有的 `rfd.workspace = true` 和 `image.workspace = true`；不得新增另一套文件选择或图片解码库。

**必须新增测试**

- `load_custom_image` 对缺失文件、null、`""` 和空白值均返回 None。
- 有效图片保存/加载 round-trip；无效字节、目录路径、不可读路径返回错误且旧配置不变。
- 取消选择不改变草稿，清除后保存得到 None。
- 保存函数遇到不可写目录返回 Err，不吞错误。
- 至少一个 `#[gpui::test]` 验证选择/清除状态变更会触发可见草稿更新；原生对话框本身可用注入的 picker 结果替身测试，不得在自动测试中弹真实窗口。

**验收命令**

```bash
cargo test -p qingqi-feature-anti-peeping --lib --locked
cargo check -p qingqi-feature-anti-peeping --all-targets --locked
git diff --check
```

### 0.7 禁止低级模型直接实现的任务

以下问题均已确认或高度可信，但缺少一个可以安全编码的局部合同。任务分配者不能只把问题描述交给低级模型并要求“修好”。必须先由高级工程师产出所列前置设计，再拆分新工单。

#### DESIGN-001 SecretStore 与明文凭据迁移

**问题**：SSH/FTP 密码、私钥口令、API token、环境变量目前明文保存。

**先决设计必须明确**：

- macOS Keychain 和 Windows Credential Manager 的统一 trait。
- credential ID 格式、访问控制、删除语义和 headless 测试替身。
- 旧 SQLite/JSON 明文到 secret reference 的原子迁移和回滚。
- 用户取消、系统凭据库锁定、记录存在但 secret 丢失的 UI 状态。
- `SecretString` 的 Debug、clone、zeroize 和日志策略。

设计未完成前，低级模型只允许添加“数据库不应出现测试秘密”的失败测试，不允许自行选择加密算法或硬编码密钥。

#### DESIGN-002 HTTP 抓包流式 body 和请求关联

**问题**：body 先整体 collect 再截断；单个 `pending` 不能可靠表示多 in-flight 请求。

**先决设计必须明确**：

- hudsucker 当前 handler clone/connection/request 生命周期。
- 转发完整 body 与只捕获有界副本的 streaming wrapper。
- request ID 从 request 到 response 的传递方式。
- HTTP/1 keep-alive/pipelining 和 HTTP/2 是否支持及测试矩阵。
- backpressure、最大内存、磁盘 quota 和脱敏策略。

低级模型不得简单把 `MAX_BODY_SIZE` 判断移到 `collect()` 前，因为标准 `collect()` 本身没有限流，也不能破坏上游转发。

#### DESIGN-003 下载 worker 生命周期和删除状态机

**问题**：删除 active task 只设置 flag，未等待线程退出就删除文件和数据库。

**先决设计必须明确**：

- `ActiveDownload` 如何持有 join handle，以及谁负责 join。
- `Canceling`/`Deleting` 状态和 UI 是否立即消失。
- worker 最终状态写入与 delete 的竞态仲裁。
- `.part` 文件保留、恢复和删除策略。
- app shutdown 的等待上限。

禁止低级模型通过固定 sleep、循环轮询 active map 或忽略数据库错误来“解决”竞争。

#### DESIGN-004 API async、真实取消和大文件 streaming

**问题**：每次发送创建 OS 线程和 blocking Client，取消只丢弃结果，body 全量进内存。

**先决设计必须明确**：

- 共享 async reqwest Client 的 owner 和 runtime。
- request handle、cancellation token、超时和 supersede 状态机。
- multipart 文件流、response preview 上限和保存到文件流程。
- 取消后 UI 何时允许下一次请求。
- history 存储的条数、字节和敏感脱敏上限。

禁止只把 `thread::spawn` 换成另一个 detached async task而不提供取消 owner。

#### DESIGN-005 GPUI TaskSupervisor 和同步 receiver bridge

**问题**：多个同步 `recv()` 占据 GPUI background executor，重复任务无统一 shutdown。

**先决设计必须明确**：

- process/plugin/window 三种 task owner。
- cancellation token 和 join timeout。
- 同步 OS receiver 到 async channel 的专用线程模型。
- app shutdown 顺序和任务泄漏指标。

低级模型可以先完成 FIX-013，但不能独立重写全部后台任务。

#### DESIGN-006 Windows 平台资源

包括低级键盘 hook 正确 `PostThreadMessageW`、join 和 `HOOK_CTX` 回收，CA key ACL，以及快速启动进程树 Job Object。必须在 Windows 主机实现和验证。禁止在 macOS 上仅凭 cfg 编译成功宣称完成。

#### DESIGN-007 MigrationRegistry

**问题**：各功能 DDL、版本写入和数据转换不在统一事务；SSH 迁移会吞坏行。

**先决设计必须明确**：migration trait、checksum、事务边界、备份、坏数据政策、版本 fixture 和恢复 UI。低级模型不得一次改动多个功能数据库，也不得通过 `unwrap_or(0)` 继续吞迁移失败。

#### DESIGN-008 剪贴板隐私与 BlobStore

**问题**：敏感内容默认保存；图片行删除后磁盘文件残留。

**先决设计必须明确**：首次授权、来源应用排除、敏感内容策略、FTS 取舍、blob 引用计数、SQLite 事务与文件删除失败的补偿、WAL 和安全删除声明。

#### DESIGN-009 快速启动风险和跨平台执行

**问题**：破坏性动作可从启动器直接执行，默认动作和解释器强绑定 macOS，输出/history 无界。

**先决设计必须明确**：RiskLevel、确认策略、平台 capability、Windows shell/进程树、输出 ring buffer 和 history quota。产品负责人必须决定哪些默认动作允许保留。

#### DESIGN-010 发布签名与供应链

需要真实 Apple/Windows 证书、CI secret、权限最小化、签名、公证、SBOM 和 provenance。低级模型可以生成 workflow 草案，但不能访问、打印或模拟真实凭据，也不能把“workflow 语法通过”当成签名完成。

#### DESIGN-011 二维码敏感历史策略与迁移

**问题**：保存、复制和扫描默认把完整二维码内容写入 `history.json`；Wi-Fi 密码、`otpauth://` 种子、登录链接和 token 会进入历史、搜索与导出。

**先决设计必须明确**：

- 历史功能默认关闭、首次询问还是按内容类型分级；这是产品决策，不能由模型猜测。
- `otpauth://`、Wi-Fi、认证 URL 和不透明 token 的敏感等级；未知内容必须采用什么默认策略。
- 仅保存 hash/类型/时间等元数据，还是把秘密交给 DESIGN-001 的 SecretStore；不得在 JSON 中自制可逆“加密”。
- 最大条数之外的保留天数、自动过期、单条删除、全部清除和导出前二次确认。
- 现有 `history.json` 的备份、迁移、用户拒绝迁移、崩溃恢复和不可恢复删除声明。
- UI 如何显示已隐藏内容，以及搜索在不保存明文后的预期行为。

设计完成后至少拆成“默认与设置”“存储迁移”“导出/删除”“安全回归测试”四张独立工单。低级模型不得先添加少量正则并宣称敏感识别完成，也不得仅遮住 UI 而保留落盘明文。

### 0.8 工单验收的统一拒绝条件

出现以下任一情况，工单应退回：

- 修改了允许范围外的业务文件，且没有事前报告。
- 非机械工单没有新增能证明问题的测试；机械工单没有执行其指定的静态不变量检查。
- 业务代码工单只验证 `cargo check`，没有运行目标 crate 测试；FIX-020/021 等 workspace/CI 工单按各自验收矩阵执行。
- 把已有失败测试删除、ignore 或改成弱断言。
- 使用 sleep 解决竞态，使用 clone/leak 解决生命周期，使用全局变量绕过 GPUI owner。
- 对路径、URL、HTTP header、known_hosts 或 JSON 使用新的手工解析器，而标准库/现有依赖已有结构化 API。
- 为通过 Clippy 添加宽泛 `#[allow(...)]`。
- 日志新增密码、token、Cookie、body、私钥或终端原文。
- 声称支持 Windows 但没有 Windows 运行结果。
- 工单完成后 `git diff` 包含大范围格式化或无关重排。

### 0.9 P2 质量问题的小工单生成规则

审计发现约 545 条 Clippy 告警，但不能创建一张“清零所有 warning”的工单。任务分配者必须在 FIX-020/021 后按下列规则生成小工单，低级模型不能自行扩大批次。

#### WARN 工单模板

1. 一张 `WARN-<crate>-<序号>` 只处理一个 crate、一个 lint 类型，且最多覆盖 20 个诊断位置。
2. 工单正文必须粘贴基线命令、基线 warning 数、每个诊断的 `file:line` 和 lint 名；不能只写“修一下警告”。
3. 允许修改范围默认仅为诊断所在 crate；需要跨 crate API 变化时升级为 L2 并重新派单。
4. 禁止新增 `#[allow]`、降低 workspace lint、删除代码绕过编译、把 `unwrap` 机械替换成 `unwrap_or_default`。
5. `dead_code` 必须先确认是未接线功能、平台 cfg、预留公共 API 还是可删除代码；不能由低级模型猜测产品意图。
6. GPUI entity/task 的 clone、borrow 和生命周期告警不得机械修复；任何 owner 变化均升级为 L2。
7. 验收时同一命令的目标 lint 数必须降为 0，其他 lint 数不能增加，目标 crate test/check 必须通过。

```text
工单：WARN-<crate>-NN
Lint：例如 clippy::collapsible_if
基线命令：cargo clippy -p <crate> --all-targets --locked --message-format short
基线数量：N
诊断位置：逐行列出 file:line
允许修改：目标 crate 中列出的文件
禁止修改：Cargo lint 配置、其他 crate、功能行为
验收：同一 Clippy 命令 + cargo test -p <crate> --locked + git diff --check
```

#### DOC 工单模板

公共 API 文档也必须按一个 crate 的一个模块拆分。每张 `DOC-<crate>-<module>` 只补 `//!`、`///`、`# Errors`、`# Panics`、`# Safety` 和最小可编译示例，不得借写文档重构 API。验收至少包含 `cargo doc -p <crate> --no-deps --locked`；涉及示例时还要执行 doctest。`unsafe` 文档必须由高级工程师复核其前置条件是否真实，而不是套模板。

## 1. 审计摘要（供任务拆解者）

Qingqi 已经形成了清晰可辨的 Rust workspace 分层：二进制组合根、GPUI 应用壳、插件核心、插件契约、共享 UI、平台适配以及独立功能 crate。中央化的 `AppPaths`、SQLite 连接池、插件 manifest、命令目录和平台模块说明项目已经超过原型阶段，具备继续工程化的基础。

但当前版本不具备安全发布条件。主要原因不是单一代码缺陷，而是安全边界、任务生命周期、数据迁移、共享输入组件和发布门禁同时存在缺口。SSH 无条件接受主机密钥，HTTP 抓包默认暴露局域网开放代理，下载文件名可逃逸目标目录，覆盖原图会先截断源文件；这些问题均应视为发布阻断项。与此同时，测试套件存在 2 个稳定失败，格式检查失败，生产目标约有 545 条 Clippy 告警，发布流水线却没有任何质量门禁。

### 1.1 总体判断

| 维度 | 结论 | 发布影响 |
|---|---|---|
| 架构设计 | 分层方向正确，但同步插件总锁、分散迁移和无统一任务监督器限制了可靠性 | P1 |
| 安全 | SSH、抓包代理、CA 私钥、下载路径和本地明文秘密存在高风险 | P0 |
| 功能设计 | 功能丰富，但取消、重试、关闭、隐私和跨平台语义不完整 | P0/P1 |
| GPUI 实现 | UI 与服务有初步隔离，但共享输入和后台任务生命周期不可靠 | P1 |
| 代码规范 | `fmt` 不通过，警告和超大文件较多，生产代码仍大量依赖 `unwrap`/`expect` | P1/P2 |
| 测试 | 纯逻辑模块已有一定覆盖，但共享 UI、托盘、防窥和跨平台关键路径缺口明显 | P0/P1 |
| 文档规范 | 未建立可执行的 API 文档、数据分级、迁移和威胁模型门禁 | P2 |
| CI/CD | 只有发布打包，无检查、签名、公证、审计、校验和和 SBOM | P0/P1 |

### 1.2 立即决策

1. 在 P0-01 至 P0-05 完成前暂停创建正式发布标签。
2. HTTP 抓包默认只监听 `127.0.0.1`，SSH 在 known-hosts 校验落地前不得以“安全连接”对外发布。
3. 临时隐藏或禁用“覆盖原图”，直到原子替换实现和故障注入测试完成。
4. 为下载文件名增加单组件净化和 canonical path containment 校验，并补回归测试。
5. 先建立 PR 质量流水线，再继续扩展功能。

## 2. 审查范围与方法

### 2.1 覆盖范围

- 20 个 workspace crate。
- 287 个 Rust 文件，共 86,880 行。
- 应用启动、GPUI 窗口和后台任务、插件注册和命令执行。
- 剪贴板、SSH/FTP、API 调试器、HTTP 抓包、下载、快速启动、图片压缩、二维码、防窥、托盘网速、系统设置等功能。
- SQLite schema 与迁移、本地 JSON/图片存储、网络和进程边界。
- macOS/Windows 平台 FFI 和 `unsafe` 使用。
- Cargo 工具链、依赖锁定、编译、格式、Clippy、测试和发布 workflow。
- Rust 语义导航使用 rust-analyzer 完成，文本检索仅用于字面量、配置和统计。

### 2.2 未覆盖内容

- 按要求没有读取或评价项目已有文档内容。
- 没有在真实 Windows 主机运行 GUI、低级键盘钩子和安装包。
- 没有执行正式渗透测试、模糊测试、长时间压力测试和网络故障注入。
- 被忽略的外网 HTTPS 解密测试没有运行；该测试依赖 `curl` 和外部网络。
- 本机未安装 `cargo-audit` 和 `cargo-deny`，因此无法给出完整 CVE 和许可证结论。

### 2.3 风险等级

| 等级 | 定义 | 处理时限 |
|---|---|---|
| P0 | 可造成凭据泄露、MITM、任意路径写入、用户数据损坏，或直接破坏发布可信度 | 下一次发布前 |
| P1 | 高概率造成错误行为、资源泄漏、功能不可用、隐私问题或跨平台故障 | 2-6 周 |
| P2 | 显著增加维护成本和回归概率，尚无立即灾难性后果 | 6-12 周 |
| P3 | 一致性、体验、可观测性和长期演进问题 | 持续治理 |

## 3. 项目量化结果

### 3.1 规模与热点

最大的生产文件包括：

| 文件 | 行数 | 判断 |
|---|---:|---|
| `qingqi-feature-quick-launch/src/view.rs` | 3,436 | 视图、交互状态和业务编排耦合过重 |
| `qingqi-feature-api-debugger/src/service.rs` | 3,063 | HTTP、变量、导入、持久化编排职责过多 |
| `qingqi-feature-image-compress/src/view.rs` | 2,550 | 批处理、文件操作和 UI 状态混合 |
| `qingqi-feature-ssh/src/view/mod.rs` | 2,168 | 连接、文件、终端和表单状态集中 |
| `qingqi-app/src/app/launcher.rs` | 1,877 | 搜索、插件模式、窗口行为和渲染集中 |
| `qingqi-feature-http-capture/src/view.rs` | 1,864 | 抓包列表、详情、设置和证书交互集中 |
| `qingqi-feature-download-manager/src/view.rs` | 1,718 | 设置、任务控制和渲染集中 |
| `qingqi-ui/src/components/input/state.rs` | 1,467 | 编辑器能力以未完成 stub 形式聚合 |

超大文件本身不是缺陷，但这些文件已同时拥有状态、I/O、领域逻辑和渲染职责，导致单元测试困难，并放大锁、取消和错误处理问题。

### 3.2 构建与测试

| 命令 | 结果 | 说明 |
|---|---|---|
| `cargo check --workspace --all-targets --locked` | 通过 | 仅证明当前 macOS 主机可编译 |
| `cargo fmt --all -- --check` | 失败 | workspace 大量文件未格式化 |
| `cargo test --workspace --locked --no-fail-fast` | 失败 | 436 通过、2 失败、1 忽略；另有 4 个 doctest 忽略 |
| `cargo test -p qingqi-app theme_store --locked` | 失败 | 9 通过、2 失败 |
| `qingqi-ui` 编译 | 通过但有 137 条告警 | 输入组件和大量 stub 是主要来源 |
| Clippy 生产目标扫描 | 约 545 条告警 | UI 约 205、SSH 108、快速启动 59、API 46、下载 35、应用层 25 |

主题失败证据：macOS 默认主题由 [`default_theme_name`](../crates/qingqi-app/src/app/theme_store.rs#L9-L17) 返回 `macOS Classic`，测试仍在 [`persists_theme_name`](../crates/qingqi-app/src/app/theme_store.rs#L314-L325) 和 [`legacy_format_defaults_theme_name`](../crates/qingqi-app/src/app/theme_store.rs#L330-L338) 断言 `Default`。

源码中约有 444 个测试标注，但分布不均：`qingqi-ui`、`qingqi-feature-anti-peeping`、`qingqi-feature-tray`、`qingqi-feature-gpui-demo` 和二进制入口为 0。HTTP 抓包真实 TLS 解密测试在 [`proxy_runtime.rs`](../crates/qingqi-feature-http-capture/tests/proxy_runtime.rs#L194) 被忽略。

### 3.3 依赖与供应链

- `Cargo.lock` 有 931 个 package 条目、826 个唯一包名；80 个包名同时存在多个版本，`windows-sys` 最多 5 个版本。
- 锁文件中没有 git source，这是正向信号。
- `gpui` 精确锁定为 `=0.2.2`，Rust 精确锁定为 `1.95.0`，可复现性基础较好。
- `russh 0.54.5` 触发 future-incompatibility；Cargo 报告可升级版本至少包括 `0.61.2`。
- 未配置 `deny.toml`，本机也没有 `cargo-audit`、`cargo-deny`。
- 只有一个 GitHub workflow，且只在标签或手工触发时打包。

## 4. 当前架构评价

### 4.1 当前依赖结构

```text
qingqi                         二进制与内置功能组合根
  -> qingqi-app                GPUI 生命周期、窗口、托盘、快捷键
  -> qingqi-core               PluginManager、命令目录、使用排序
  -> qingqi-feature-*          业务功能实现

qingqi-feature-*
  -> qingqi-plugin             插件契约、事件、数据库、路径、托盘接口
  -> qingqi-ui                 共享 GPUI 组件与主题
  -> qingqi-platform           macOS/Windows 能力封装

qingqi-plugin
  -> gpui / rusqlite           当前“插件 SDK”仍绑定宿主进程和 UI ABI
```

### 4.2 做得较好的部分

1. 功能以 crate 隔离，组合根在 [`features/registry.rs`](../crates/qingqi/src/features/registry.rs#L23-L129) 可直接看出产品构成。
2. `qingqi-platform` 承载大部分 FFI；238 处 `unsafe` 只分布在 9 个文件，除快速启动的 Unix 信号外基本没有泄漏到功能层。
3. [`DatabaseService`](../crates/qingqi-plugin/src/database.rs#L79-L221) 统一路径、连接池、WAL、foreign keys 和注册键，避免每个功能重复初始化连接。
4. 插件调用使用 `catch_unwind` 隔离 panic，命令缓存和使用记录有清晰边界。
5. 高风险功能大多已有 service/store/view 雏形，整改可以渐进完成，不需要整体重写。

### 4.3 核心架构问题

#### A. “插件”目前只是静态模块，不是外部扩展系统

[`PluginSource`](../crates/qingqi-core/src/registry.rs#L14-L24) 声明了 `External`，但注册器只接受进程内 Rust closure；全部功能在 [`qingqi/Cargo.toml`](../crates/qingqi/Cargo.toml#L9-L27) 静态链接。Rust trait object 也没有稳定 ABI、安全隔离或权限模型。

整改决策必须二选一：

1. 如果只需要内置模块，删除 `External` 和“动态插件”暗示，把概念改为 `Feature`，降低错误预期。
2. 如果确实需要第三方插件，采用进程隔离或 WASM/WASI，定义版本化 IPC、能力授权、资源配额和签名；不建议直接 `dlopen` Rust ABI。

#### B. 同步插件 trait 和全局总锁会把 I/O 风险带到 UI 线程

[`Plugin`](../crates/qingqi-plugin/src/plugin.rs#L156-L231) 的 `commands`、`open`、`handle_command`、`start_background` 全部为同步接口。应用把管理器放进 `Arc<Mutex<PluginManager>>`，并在持锁时调用插件，例如 [`open_window_view`](../crates/qingqi-app/src/app/window_controller.rs#L537-L558)。任何插件内数据库、文件或网络操作都可能阻塞 GPUI 主线程，并阻塞所有其他插件。

建议将插件接口拆成：静态 descriptor、纯同步 view factory、异步 command service 和显式 lifecycle handle。UI 线程只创建 entity 和提交命令；I/O 通过受监督任务返回不可变结果。

#### C. 缺少统一资源所有权模型

当前重复任务、OS 线程、通道、临时文件、数据库 blob 和窗口句柄由各功能自行管理。`detach()`、裸 `thread::spawn`、`Vec<Task>` 和 `Arc` 自循环没有统一关闭协议。结果是窗口关闭不等于功能停止，插件 shutdown 也不能保证资源释放。

#### D. 中央数据库服务没有中央迁移器

数据库连接已统一，但 schema 仍由各功能在 `open()` 时自行创建和升级。版本写入、DDL、数据转换、校验没有统一事务、checksum、备份和恢复机制，详见 P1-08。

## 5. P0 发布阻断项

### P0-01 SSH 主机身份、凭据和密码显示不安全

**证据**

- [`check_server_key`](../crates/qingqi-feature-ssh/src/protocol/ssh.rs#L43-L58) 记录指纹后无条件 `Ok(true)`，任何主机密钥都会被信任。
- [`AuthConfig`](../crates/qingqi-feature-ssh/src/model.rs#L55-L80) 直接持有 FTP/SSH 密码；私钥口令同样是普通 `String`。
- [`ProfileStore::create/update`](../crates/qingqi-feature-ssh/src/store.rs#L254-L303) 将整个认证配置序列化到 SQLite `auth_json`。
- SSH 数据 debug 日志写入最多 120 字节终端内容，见 [`ssh.rs`](../crates/qingqi-feature-ssh/src/protocol/ssh.rs#L60-L72)。
- 密码输入初始化没有调用 `set_masked`，见 [`view/mod.rs`](../crates/qingqi-feature-ssh/src/view/mod.rs#L352-L369)；共享输入绘制也完全没有读取 `masked`。

**影响**

攻击者可在首次连接或网络被劫持时实施 MITM。数据库、日志、备份或屏幕共享泄露会暴露凭据和终端敏感内容。这是远程连接工具的基础安全属性缺失。

**整改步骤**

1. 实现 OpenSSH `known_hosts` 语义：已知主机严格匹配，未知主机显示 SHA-256 指纹并要求用户确认，变更主机默认拒绝且明确告警。
2. 使用系统凭据存储保存密码和 passphrase：macOS Keychain、Windows Credential Manager。SQLite 只保存不可逆的 credential reference。
3. 引入 `SecretString` 或自定义 `Secret<T>`，禁止 `Debug`/`Display` 输出明文，离开连接流程后主动清零内存。
4. 密码、token、API key 输入必须默认遮罩，并提供可访问的按住显示/切换显示能力。
5. 删除终端原始内容 debug 预览，或仅在显式诊断模式下启用并做控制序列和敏感模式脱敏。
6. 为旧 `auth_json` 做一次性迁移：先写入系统凭据库，确认成功后事务内清除明文字段；迁移失败保持旧数据并提示用户。

**验收标准**

- 未知主机、已知主机、主机密钥变更三类集成测试全部通过。
- 数据库和正常日志中检索不到测试密码、token 和 passphrase。
- SSH/FTP 密码字段截图与像素测试不出现明文。
- 从旧数据库升级后连接可用，迁移失败不会丢失 profile。

**责任与估算**：SSH 负责人 + 平台安全负责人；8-12 人日。

### P0-02 HTTP 抓包形成开放代理，CA 私钥和捕获数据保护不足

**证据**

- [`CaptureEngine`](../crates/qingqi-feature-http-capture/src/engine.rs#L104-L126) 固定绑定 `0.0.0.0`，没有认证、来源 IP 限制或显式远程模式。
- [`generate_ca`](../crates/qingqi-feature-http-capture/src/certificate.rs#L92-L119) 使用普通 `fs::write` 写 CA 私钥；审计环境实测常见 `umask 022` 下权限为 `0644`。
- 请求和响应头、正文原样进入 [`captured_exchanges`](../crates/qingqi-feature-http-capture/src/store.rs#L16-L32)，没有 Authorization、Cookie、Set-Cookie 脱敏。
- 请求和响应都先调用 `body.collect()`，再在内存中截断 1 MB，见 [`proxy_handler.rs`](../crates/qingqi-feature-http-capture/src/proxy_handler.rs#L236-L282)。
- handler 只有一个 [`pending: Option<CaptureContext>`](../crates/qingqi-feature-http-capture/src/proxy_handler.rs#L23-L45)，无法表达同一连接上的多个 in-flight exchange。

**影响**

同一局域网设备可把应用当作匿名代理使用。CA 私钥泄露后可伪造任意受该 CA 信任的证书。大请求可造成内存耗尽，认证头和业务正文会长期明文落盘；并发请求还可能发生请求/响应错配。

**整改步骤**

1. 默认监听 `127.0.0.1` 和 `::1`；远程监听作为高级模式，必须显示风险确认、随机高强度 token、来源 CIDR 白名单和会话到期时间。
2. CA 私钥创建时使用原子 `create_new` 和 `0600` 权限；Windows 使用 ACL。启动时校验权限，过宽时拒绝 MITM 并提供修复。
3. CA 私钥优先放入系统密钥存储；文件只保存证书和不可导出的引用。提供轮换、撤销信任和彻底删除流程。
4. 使用限流 body wrapper，在读取阶段限制捕获副本；转发流量保持 streaming，不能为了展示聚合整个 body。
5. 默认脱敏认证头、Cookie、查询 token 和常见 JSON secret key；用户必须显式选择才可保存原文。
6. 用请求唯一 ID 映射上下文，验证 HTTP/1 keep-alive、pipelining 和 HTTP/2 多路复用行为。
7. 加磁盘配额、保留天数、单记录大小和“停止时自动清除”选项。

**验收标准**

- 默认端口从非本机地址无法连接。
- CA key 在 macOS/Linux 为 `0600`，Windows ACL 仅当前用户可读。
- 100 MB 请求不会让进程内存随 body 线性增长，且上游仍可正确收到完整流。
- 并发 100 个请求的 method、URL、状态和 body 不错配。
- 数据库默认不含测试 Authorization/Cookie 明文。

**责任与估算**：抓包负责人 + 安全负责人；10-15 人日。

### P0-03 下载文件名可逃逸保存目录

**证据**

- [`guess_file_name`](../crates/qingqi-feature-download-manager/src/model.rs#L194-L204) 对最后一个 URL segment 做 percent decode，但没有限制 `/`、`\\`、`..`、绝对路径和 Windows 设备名。
- [`resolve_save_path_in_dir`](../crates/qingqi-feature-download-manager/src/service.rs#L718-L745) 直接执行 `dir.join(file_name)`。

例如 URL 末段 `%2e%2e%2fconfig.json` 解码为 `../config.json`，拼接后可写到下载根目录之外。Content-Disposition 路径也需要同样处理。

**影响**

恶意下载 URL 可覆盖用户可写范围内的其他文件。结合自动下载或社会工程可造成配置篡改和数据破坏。

**整改步骤**

1. 使用 `Path::file_name()` 只保留单个普通组件，拒绝 `ParentDir`、`RootDir`、prefix 和分隔符。
2. 对空名、`.`、`..`、控制字符、尾随点/空格、Windows 保留名生成安全替代名。
3. 文件创建使用 `OpenOptions::create_new(true)`，在同一目录原子竞争命名，禁止“先 exists 再 create”的 TOCTOU。
4. 创建前 canonicalize 父目录，并验证目标父路径仍以 canonical 下载根目录开头。
5. URL 和 Content-Disposition 使用成熟解析库，不自行拆 `%` 或 header 参数。

**验收标准**

- 覆盖 `../`、`%2f`、`%5c`、绝对路径、UNC、盘符、NUL、Windows 设备名和 Unicode 混淆的参数化测试。
- 所有测试目标都位于临时下载根目录内。
- 并发创建同名文件不会覆盖已有文件。

**责任与估算**：下载负责人；3-5 人日。

### P0-04 图片“覆盖原图”不是原子操作，编码失败会损坏源文件

**证据**

- [`compress_file`](../crates/qingqi-feature-image-compress/src/service.rs#L215-L259) 在覆盖模式下把输出路径直接设置为源路径。
- [`write_image`](../crates/qingqi-feature-image-compress/src/service.rs#L263-L322) 首先 `File::create(output_path)`，这会在编码开始前截断原文件。
- 单条“覆盖”操作使用 [`std::fs::copy`](../crates/qingqi-feature-image-compress/src/view.rs#L423-L437) 直接覆盖，同样没有临时文件、fsync 或回滚。

**影响**

编码错误、磁盘写满、进程崩溃或断电都会永久破坏用户原图。该功能的名称会让用户合理期待安全替换，因此风险高于普通导出失败。

**整改步骤**

1. 在源文件同目录创建权限一致的临时文件。
2. 完整编码后 `flush`、`sync_all`，重新解码并校验格式与尺寸。
3. 保留原权限和必要元数据，再执行同文件系统原子 rename/replace。
4. Windows 使用支持 replace-existing 的平台 API；失败时保留源文件和临时文件诊断信息。
5. 默认关闭覆盖，并在结果大于原图或格式能力变化时二次确认。

**验收标准**

- 注入编码失败、磁盘写满、rename 失败时，源文件 hash 始终不变。
- 成功替换后文件可重新解码，临时文件被清理。
- macOS 和 Windows 均有集成测试。

**责任与估算**：图片功能负责人 + 平台负责人；4-6 人日。

### P0-05 发布流水线没有质量门禁，审计工作区基线为红灯

**证据**

- [`release.yml`](../.github/workflows/release.yml#L37-L149) 只构建和上传 macOS/Windows 包，不执行 fmt、check、Clippy、test、audit 或 deny。
- 审计工作区中 `cargo fmt --check` 失败，workspace 测试有 2 个稳定失败；由于审计基线包含未提交源码，这一结果不能被表述为远端 `main` 的独立运行结论。
- 根 lint 仅对 `unsafe_op_in_unsafe_fn`、`unwrap_used`、`todo` 发出 warning，见 [`Cargo.toml`](../Cargo.toml#L72-L78)。
- workflow 使用 `macos-latest`、`windows-latest`、Actions 浮动主版本和未指定版本的 `cargo-bundle`。

**影响**

标签发布可以在测试失败和安全缺陷已知的情况下成功。构建环境、打包器和 Actions 会随时间漂移，发布产物不可充分追溯。

**整改步骤**

1. 新建 PR/merge quality workflow，执行本文第 13 节命令。
2. release job 必须 `needs: quality`，且只从受保护标签和已验证 commit 构建。
3. 所有 cargo 命令使用 `--locked`；打包工具固定版本，Actions 固定完整 commit SHA。
4. 标签版本必须与 workspace/package 版本一致。
5. 先把告警基线降为 0，再启用 `-D warnings`；不能长期维护数百条 allow。

**验收标准**

- 在故意引入格式错误、Clippy 告警和失败测试时 PR 与 release 均被阻断。
- 同一 commit 的两次构建记录 Rust、runner image、cargo-bundle 和依赖 lock hash。

**责任与估算**：工程效率/发布负责人；3-5 人日，不含清理历史告警的 8-15 人日。

## 6. P1 高风险问题

### P1-01 下载续传、重试和删除语义不正确

**证据与影响**

- Range 请求只解析 Content-Range 总大小，不验证返回起点，见 [`parse_content_range`](../crates/qingqi-feature-download-manager/src/service.rs#L982-L990)。
- 服务端对续传返回 200 时文件会重新创建，但 `downloaded` 仍从旧进度累加，见 [`service.rs`](../crates/qingqi-feature-download-manager/src/service.rs#L879-L914)。最终数据库进度和文件大小错误。
- 没有持久化 ETag/Last-Modified、原文件实际长度和内容身份，远端文件变化时可拼接出损坏文件。
- “重试次数”分支只把状态短暂设为 Pending，外层随即写 Failed，且没有再次 dispatch，见 [`service.rs`](../crates/qingqi-feature-download-manager/src/service.rs#L854-L876) 和错误收尾逻辑。
- [`delete_task`](../crates/qingqi-feature-download-manager/src/service.rs#L519-L533) 只设置 cancel flag，随即删除文件和数据库；下载线程仍可能写入或重新更新记录。

**整改**

1. 建立明确状态机：Queued -> Running -> Pausing/Canceling -> Paused/Canceled/Failed/Completed。
2. 每个任务持有 join handle；删除必须先取消、等待 worker 退出，再删除 `.part` 和记录。
3. 始终下载到 `.part`，完成并校验后原子改名。
4. 续传验证本地长度、206、Content-Range 起点、ETag/If-Range；200 时把进度归零。
5. 重试保存 attempt count，采用带 jitter 的退避，并区分可重试网络错误与永久错误。

**验收**：本地可控 HTTP server 覆盖 200/206/416、错误 range、ETag 变化、断线、取消删除竞争和 3 次退避。

**责任与估算**：下载负责人；7-10 人日。

### P1-02 API 调试器的编码、取消、内存和秘密管理不可靠

**证据与影响**

- 查询参数和 query auth 直接 `format!("{k}={v}")` 拼接，见 [`service.rs`](../crates/qingqi-feature-api-debugger/src/service.rs#L1132-L1158)；表单同样不编码，见 [`build_form_urlencoded_body`](../crates/qingqi-feature-api-debugger/src/service.rs#L1459-L1466)。空格、`&`、`=`、Unicode 会改变请求语义。
- 取消只递增 generation 并丢弃结果，注释也确认不能中止请求，见 [`cancel_request`](../crates/qingqi-feature-api-debugger/src/service.rs#L216-L226)。UI 立即允许下一次发送，会继续创建 OS 线程。
- 每次发送使用 `thread::spawn` 和新的 blocking Client，见 [`send_request`](../crates/qingqi-feature-api-debugger/src/service.rs#L696-L745)；超时前不可回收。
- 二进制请求使用 `fs::read`，multipart 手工构建并把所有文件读入 `Vec<u8>`，见 [`service.rs`](../crates/qingqi-feature-api-debugger/src/service.rs#L1210-L1223) 和 [`build_multipart_body`](../crates/qingqi-feature-api-debugger/src/service.rs#L1482-L1516)。
- 响应使用 `resp.text()` 整体读入，见 [`service.rs`](../crates/qingqi-feature-api-debugger/src/service.rs#L1283-L1293)。
- request snapshot 明文包含 `auth_value`，环境变量、header 和完整响应历史也明文入库，见 [`model.rs`](../crates/qingqi-feature-api-debugger/src/model.rs#L463-L477) 和 [`data_source.rs`](../crates/qingqi-feature-api-debugger/src/data_source.rs#L62-L107)。

**整改**

1. 使用 `url::Url`/`query_pairs_mut` 和 `reqwest::RequestBuilder::query/form/multipart`。
2. 复用 async `reqwest::Client`；每个请求持有 `AbortHandle` 或 cancellation token，取消后等待任务结束。
3. 文件与响应流式传输，设置请求/响应展示上限、下载到文件选项和 backpressure。
4. 认证值引用统一 SecretStore；导出、代码生成、日志和 request dump 默认脱敏。
5. history 增加总量、总字节、保留天数和单响应上限。

**验收**：特殊字符编码测试、取消后 socket/任务收敛测试、1 GB 流式文件内存上限测试、数据库秘密扫描。

**责任与估算**：API 负责人；8-12 人日。

### P1-03 剪贴板默认全量留存，敏感标签不构成保护，图片文件泄漏

**证据与影响**

- 默认捕获文本、图片和文件列表，见 [`ClipboardConfig::default`](../crates/qingqi-feature-clipboard/src/history_store.rs#L147-L167)。
- `contains_sensitive` 只搜索少数英文关键词，且仅用于渲染“敏感”标签，见 [`history_store.rs`](../crates/qingqi-feature-clipboard/src/history_store.rs#L138-L145) 和 [`view/history.rs`](../crates/qingqi-feature-clipboard/src/view/history.rs#L660-L679)。正文及 FTS 仍然明文。
- 图片写到独立目录，见 [`capture_image`](../crates/qingqi-feature-clipboard/src/service.rs#L378-L403)。删除、清空和 5,000 条淘汰只删 SQLite/FTS，不删除对应文件，见 [`data_source.rs`](../crates/qingqi-feature-clipboard/src/data_source.rs#L339-L355) 和 [`prune_history`](../crates/qingqi-feature-clipboard/src/data_source.rs#L509-L522)。

**整改**

1. 首次启用明确征得同意，提供应用排除、密码管理器排除、敏感内容不入库和自动过期。
2. 使用来源应用 metadata；macOS/Windows 支持时识别 transient/concealed clipboard 类型。
3. 文本内容加密存储，FTS 需要在“不可搜索敏感内容”和受保护索引之间做明确产品取舍。
4. 引入 `BlobStore`，数据库行与图片 blob 事务关联；删除、清空、淘汰后做引用计数清理和孤儿扫描。
5. “清空”同时覆盖 WAL、临时文件和图片；文档明确 SQLite 安全删除的限制。

**验收**：密码管理器场景不留存；删除/淘汰后 blob 目录无孤儿；敏感样本不进入 FTS；升级不丢置顶记录。

**责任与估算**：剪贴板负责人 + 安全负责人；7-10 人日。

### P1-04 共享输入组件的状态、绘制和交互契约断裂

**证据与影响**

- `masked` 可以设置但 [`TextElement::paint`](../crates/qingqi-ui/src/components/input/element.rs#L108-L177) 始终绘制原文。
- `read_only` 只有字段和默认值，没有 setter 或任何编辑路径判断。
- [`Input::disabled`](../crates/qingqi-ui/src/components/input/input.rs#L97-L105) 只修改 builder 字段，没有同步 `InputState.disabled`；多数键盘动作也不检查 disabled。
- `show_clear_button` 和 `mask_toggle` 被计算/保存但从未渲染，见 [`input.rs`](../crates/qingqi-ui/src/components/input/input.rs#L115-L192)。
- 多行文本绘制时把换行替换成空格，见 [`element.rs`](../crates/qingqi-ui/src/components/input/element.rs#L142-L166)。
- 滚轮处理被注释，搜索面板只切换内部状态且未渲染，见 [`state.rs`](../crates/qingqi-ui/src/components/input/state.rs#L975-L1020)。
- redo 的结束位置写成 `start + old_len.min(text.len())`，当 `start > 0` 时仍可能越界，见 [`state.rs`](../crates/qingqi-ui/src/components/input/state.rs#L795-L803)。
- 整个 `qingqi-ui` 没有测试，输入目录也没有测试标注。

**整改**

1. 先冻结对外 API，定义单行、密码、多行三种明确组件，不把未实现的 LSP 编辑器能力暴露为可用 API。
2. 建立唯一真相：disabled/read-only/masked 属于 `InputState`，builder 只能在构造时配置或通过 action 更新 state。
3. 所有修改入口共用 `can_edit()`：键盘、IME、粘贴、剪切、undo/redo、清空、拖放。
4. masked 绘制按 grapheme 输出掩码，但 selection、cursor、IME 映射仍基于原文；复制策略必须明确。
5. 多行按真实 line layout 绘制，落实 viewport、滚动、软换行和 hit testing。
6. 删除无产品计划的 LSP/popover stub，或迁到独立 editor crate，不能让死代码占据公共 API。

**验收**：状态机单测、Unicode/IME/emoji/组合字符 property test、GPUI 交互测试和 macOS/Windows 截图测试；密码像素中无原文。

**责任与估算**：UI 基础设施负责人；10-15 人日。

### P1-05 后台任务监督器会增长并占用 GPUI executor

**证据与影响**

- [`BackgroundSupervisor`](../crates/qingqi-app/src/app/background.rs#L30-L35) 把任务放入 `Vec<Task<()>>`。
- 每个托盘事件处理后递归注册新任务并再次 push，完成任务从不移除，见 [`arm_tray_icon_event`](../crates/qingqi-app/src/app/background.rs#L242-L274) 和 [`arm_tray_menu_event`](../crates/qingqi-app/src/app/background.rs#L276-L302)。事件越多，向量越大。
- 快捷键、主题、电源、托盘等把同步 `recv()` 包在 async closure 中提交给 background executor，见 [`background.rs`](../crates/qingqi-app/src/app/background.rs#L70-L221)。每个长期等待会占据执行器线程。

**整改**

1. `BackgroundSupervisor` 改为按名称持有固定 task handle 和 cancellation token，而不是事件级 task 列表。
2. 每类事件使用一个长生命周期循环；同步 OS receiver 由专用 bridge thread 转成 async channel。
3. `Drop`/shutdown 时先 cancel，再有超时地 join；记录未退出任务。
4. 禁止重复任务直接 `detach()`；只允许进程级 fire-and-forget，并在代码评审中注明所有权。

**验收**：连续模拟 100 万托盘事件后 task 数保持常量；关闭应用后所有 bridge thread 和 channel 在限定时间内退出。

**责任与估算**：应用壳负责人；5-8 人日。

### P1-06 Windows 低级键盘钩子无法可靠停止并泄漏上下文

**证据与影响**

- hook thread 阻塞在 `GetMessageW`，见 [`low_level_hook.rs`](../crates/qingqi-platform/src/low_level_hook.rs#L76-L113)。
- `Drop` 在调用方线程执行 `PostQuitMessage(0)`，该 API 给当前线程队列发消息，不会唤醒 hook thread，见 [`low_level_hook.rs`](../crates/qingqi-platform/src/low_level_hook.rs#L137-L144)。
- `JoinHandle` 只被丢弃，没有 join；`HOOK_CTX` 保存 `Box::into_raw` 指针，成功路径没有 `Box::from_raw`，见 [`low_level_hook.rs`](../crates/qingqi-platform/src/low_level_hook.rs#L242-L284)。

**整改**

保存 hook thread ID，使用 `PostThreadMessageW(thread_id, WM_QUIT, ...)` 唤醒；`Drop`/显式 shutdown 发送退出、join、unhook、`swap(0)` 后回收 Box。安装也应使用 channel 同步 ready/error，避免 busy-yield。

**验收**：Windows 上重复安装/卸载 1,000 次，无残留线程、hook 或堆增长；应用退出不挂起。

**责任与估算**：Windows 平台负责人；3-5 人日。

### P1-07 防窥窗口关闭和自定义图片功能不完整

**证据与影响**

- `close_idle` 和 view `on_close` 只把 `active` 设为 false，不关闭全屏 overlay，见 [`plugin.rs`](../crates/qingqi-feature-anti-peeping/src/plugin.rs#L130-L149) 和 [`plugin.rs`](../crates/qingqi-feature-anti-peeping/src/plugin.rs#L248-L250)。只有 Esc 路径调用 `close_overlays`。
- 设置页显示 `draft_path`，但没有输入框或文件选择入口；“保存设置”只保存创建 view 时的旧值，见 [`plugin.rs`](../crates/qingqi-feature-anti-peeping/src/plugin.rs#L167-L244)。

**整改**

让 overlay session 成为 RAII owner，关闭插件、关闭设置窗、Esc、显示器变化和 app shutdown 都走同一个 idempotent `close()`。增加文件选择、清除、预览、无效路径回退；多显示器窗口打开失败时回滚已打开窗口。

**验收**：所有退出路径都在 500 ms 内关闭所有显示器 overlay；自定义图可选择、持久化、重启恢复和清除。

**责任与估算**：防窥功能负责人；3-4 人日。

### P1-08 数据库迁移分散且不具备原子性和可恢复性

**证据与影响**

- 快速启动通过检查列后逐条 `ALTER TABLE`，没有版本表或事务，见 [`store.rs`](../crates/qingqi-feature-quick-launch/src/store.rs#L320-L377)。
- API 调试器先执行整套 schema，再单独写版本，见 [`data_source.rs`](../crates/qingqi-feature-api-debugger/src/data_source.rs#L27-L137)。
- 抓包和下载也把 DDL 与版本更新分开，见 [`http-capture/store.rs`](../crates/qingqi-feature-http-capture/src/store.rs#L91-L100) 和 [`download/store.rs`](../crates/qingqi-feature-download-manager/src/store.rs#L23-L34)。
- SSH v2->v3 用 `filter_map(|r| r.ok())` 静默丢行；创建新记录和迁移日志不在同一事务，见 [`ssh/store.rs`](../crates/qingqi-feature-ssh/src/store.rs#L81-L127) 和 [`ssh/store.rs`](../crates/qingqi-feature-ssh/src/store.rs#L196-L214)。调用方又用 `unwrap_or(0)` 吞掉整体迁移失败，见 [`ssh/lib.rs`](../crates/qingqi-feature-ssh/src/lib.rs#L57-L70)。

**整改**

1. 在 `qingqi-plugin` 建立 `MigrationRegistry`：数据库 key、单调版本、up SQL/Rust fn、checksum、最小应用版本。
2. 每步 `BEGIN IMMEDIATE`，数据变换、校验和版本写入同一事务；失败 rollback。
3. 迁移前创建可恢复备份，成功后延迟清理；启动失败展示恢复入口，不能静默跳过。
4. 建立每个历史版本的 fixture，并测试 v0 -> latest、每一跳、重复执行、故障中断和坏数据策略。

**验收**：任意语句注入失败后 schema/version/data 均回到迁移前状态；坏行有明确错误和用户可见恢复路径。

**责任与估算**：数据基础设施负责人 + 各功能负责人；8-12 人日。

### P1-09 快速启动可直接执行破坏性动作，输出和历史无界，Windows 语义不成立

**证据与影响**

- 默认动作包含清空废纸篓、休眠等 macOS 命令，见 [`default_actions`](../crates/qingqi-feature-quick-launch/src/service.rs#L949-L1035)。
- 动作会作为启动器命令暴露；无参数时 [`handle_command`](../crates/qingqi-feature-quick-launch/src/plugin.rs#L72-L101) 直接执行，没有风险等级或确认。
- `wait_with_output()` 无界收集 stdout/stderr，见 [`service.rs`](../crates/qingqi-feature-quick-launch/src/service.rs#L684-L733)；完整输出永久写入 runs，schema 没有淘汰约束，见 [`store.rs`](../crates/qingqi-feature-quick-launch/src/store.rs#L50-L108)。
- 默认解释器固定 `/bin/zsh`，非 Unix 停止进程明确返回不支持，见 [`service.rs`](../crates/qingqi-feature-quick-launch/src/service.rs#L791-L923)，但产品发布 Windows 包。

**整改**

为动作加入 `RiskLevel`、平台条件和确认策略；清空、删除、关机类默认二次确认且不能由模糊搜索回车直接触发。stdout/stderr 使用 ring buffer 和磁盘上限，历史按条数/字节/日期淘汰。为 Windows 实现 PowerShell/cmd 和 Job Object 进程树终止，或明确在 Windows 隐藏不支持动作。

**验收**：破坏性默认动作需要明确确认；无限输出进程不会让内存/数据库无限增长；Windows 动作和停止行为有真实主机测试。

**责任与估算**：快速启动负责人 + Windows 负责人；7-10 人日。

### P1-10 日志 WorkerGuard 在主循环前被释放

**证据与影响**

`bootstrap` 正确把 guard 放入 `AppHost`，但 [`run`](../crates/qingqi-app/src/app/runtime.rs#L205-L216) 使用 `_log_guard: _` 解构，值在进入 `Application::run` 前被 drop。文件 non-blocking worker 因此不能覆盖应用生命周期，后续日志可能丢失，崩溃诊断不完整。

**整改**

绑定为 `_log_guard` 并让其活到 [`app.run`](../crates/qingqi-app/src/app/runtime.rs#L232-L368) 返回；退出时先停止生产日志，再 drop guard 完成 flush。增加启动、运行中、shutdown 三个 marker 的文件测试。

**验收**：运行期间和正常退出日志均落盘；模拟 panic 后已有队列尽可能 flush。

**责任与估算**：应用壳负责人；0.5-1 人日。

### P1-11 托盘网速存在不可停止任务和未告知的外部请求

**证据与影响**

- 启动后台服务立即调用 `refresh_ip_cache_background`，见 [`tray/service.rs`](../crates/qingqi-feature-tray/src/service.rs#L84-L97)。
- 公网 IP 请求发送到 `https://api.ipify.org`，见 [`tray/service.rs`](../crates/qingqi-feature-tray/src/service.rs#L302-L318)，用户没有显式选择。
- 定时采样递归创建并 detach 任务，没有停止 token；闭包持有 `Arc<Service>`，见 [`tray/service.rs`](../crates/qingqi-feature-tray/src/service.rs#L257-L285)。

**整改**

公网 IP 默认关闭并说明第三方服务、发送信息和缓存时长；允许自定义 endpoint。采样任务由 service 持有并在 shutdown 取消，设置不可见时可暂停。外部请求遵守全局代理、超时和隐私模式。

**验收**：默认启动抓包看不到 ipify 请求；关闭插件后采样计数停止且 service 可 drop。

**责任与估算**：托盘负责人；2-3 人日。

### P1-12 发布产物未签名、未公证且没有完整性材料

**证据与影响**

[`release.yml`](../.github/workflows/release.yml#L72-L90) 直接压缩 macOS `.app`，Windows 直接压缩 exe；没有 macOS codesign/notary/staple、Windows Authenticode、SHA-256、SBOM 或 provenance。用户无法可靠验证来源，系统也会给出高风险警告。

**整改**

使用短期 CI 凭据完成 macOS hardened runtime、Developer ID 签名、公证和 staple；Windows 使用受保护证书签名。生成 SHA-256、CycloneDX/SPDX SBOM 和 GitHub artifact attestation，发布页上传全部材料。

**验收**：`codesign --verify --deep --strict`、`spctl --assess`、notary history 和 Windows `Get-AuthenticodeSignature` 全部成功；checksum 与下载包一致。

**责任与估算**：发布负责人；4-7 人日，另需证书准备时间。

### P1-13 二维码历史默认保存完整敏感内容

**证据与影响**

二维码保存、复制和扫描都会把完整内容写入 `history.json`，见 [`QrHistoryStore::push`](../crates/qingqi-feature-qr-code/src/store.rs#L75-L92) 和 [`write_records`](../crates/qingqi-feature-qr-code/src/store.rs#L132-L150)。Wi-Fi 密码、登录 token、OTP provisioning URI 会被明文保留并可导出。

**整改**

默认不保存包含 `otpauth://`、Wi-Fi credential、token query 的内容；首次启用历史时明确授权。历史支持关闭、自动过期、加密和只保存摘要。导出前提示敏感内容并允许脱敏。

**验收**：敏感 URI 默认不落盘；关闭历史后 save/copy/scan 不写文件。

**责任与估算**：二维码负责人 + 安全负责人；2-4 人日。

## 7. P2/P3 工程治理问题

### P2-01 两套剪贴板存储实现重复且 schema 已漂移

`ClipboardHistoryStore` 与生产使用的 `ClipboardDataSource` 高度重复。语义引用检查显示旧 store 没有生产调用，仅测试借用新 data source；两者的 FTS schema 已出现 `contentless_delete` 差异。应先补行为契约测试，再删除旧实现，保留单一 repository。

**验收**：只有一个 schema 和 store；所有历史、FTS、置顶、删除、迁移测试针对生产实现。

**估算**：2-4 人日。

### P2-02 配置和历史 JSON 普遍直接覆盖写入

项目多处使用 `fs::write` 保存主题、快捷键、系统设置、二维码历史和 API workspace，例如 [`theme_store.rs`](../crates/qingqi-app/src/app/theme_store.rs#L190-L201)、[`qr-code/store.rs`](../crates/qingqi-feature-qr-code/src/store.rs#L146-L150) 和 [`api-debugger/store.rs`](../crates/qingqi-feature-api-debugger/src/store.rs#L41-L49)。崩溃或磁盘写满会留下截断文件。

建立统一 `AtomicFileStore`：同目录临时文件、权限、flush/sync、原子 replace、`.bak`、schema version 和恢复。

**估算**：3-5 人日。

### P2-03 生产代码警告、格式和错误恢复标准未建立

- `cargo fmt --check` 产生大范围差异。
- `qingqi-ui` 单 crate 137 条编译告警。
- 全源码静态扫描有 779 个 `unwrap()`/`expect()`，其中包含测试，但快速启动等生产路径也大量以 mutex poisoning 为 panic。
- 热键主路径保留多处 `println!("!!! ...")`，例如 [`background.rs`](../crates/qingqi-app/src/app/background.rs#L70-L103)。

整改顺序：先格式化独立 PR；再删除死 stub 和 unused import；逐 crate 清零 warnings；生产 `unwrap/expect` 仅允许经过注释证明的不可变式；统一 `lock_or_recover` 或显式错误传播；日志全部走 `tracing` 并禁止敏感字段。

### P2-04 `unsafe` 边界尚可，但安全说明和生命周期测试不足

静态扫描有 238 个 `unsafe` 关键字，只有约 32 个相邻 `SAFETY`/Safety 文档标记。数量受 cfg 和块结构影响，不能直接等同于 206 个无说明缺陷，但当前根 lint 只是 warning。

要求：`unsafe_op_in_unsafe_fn = deny`；每个 unsafe block 描述指针有效性、线程、所有权、对齐和释放条件；FFI 返回值不得忽略；平台资源使用 RAII；纯转换函数增加 property test，Windows/macOS 资源增加重复创建销毁测试。

### P2-05 依赖树过大且缺少许可证/CVE 策略

优先升级 `russh` 并验证 known-hosts API；用 `cargo tree -d` 逐项处理可控重复版本。配置 `cargo-deny` 的 advisories、licenses、bans、sources；生成 SBOM。不要为了数字盲目统一 GPUI 间接依赖，但应记录无法收敛的原因和升级窗口。

### P2-06 GPUI demo 被编入正式产品

[`gpui-demo` manifest](../crates/qingqi-feature-gpui-demo/src/manifest.rs#L10-L45) 明确是学习实验场，却在正式 registry 注册并进入二进制。应使用非默认 Cargo feature `dev-tools` 或独立 examples 应用，release profile 不包含 demo 和实验组件。

### P2-07 公共 API 和源码文档没有门禁

workspace 未启用 `missing_docs`，仅发现少量可执行 rustdoc 示例。`qingqi-plugin`、`qingqi-core`、`qingqi-platform` 的公共契约尤其需要稳定性、线程模型、错误、panic、取消和安全说明。具体规范见第 11 节。

### P3-01 产品文案、平台能力和错误状态需要统一

当前中英文错误、注释和日志混合；部分功能在 Windows 仍展示 macOS 动作；大量 I/O 错误只写日志或回退为空列表。应建立用户错误码、可恢复操作、平台 capability gate 和本地化资源，避免“功能看得见但不可用”。

### P3-02 缺少性能与资源预算

建议为启动时间、空闲 CPU、常驻内存、SQLite 大小、日志、剪贴板图片、HTTP body、API history、命令输出和后台线程数定义预算，并在 nightly/基准 workflow 中跟踪趋势。

## 8. 功能设计专项结论

| 功能 | 当前能力 | 主要缺口 | 产品整改决策 |
|---|---|---|---|
| 启动器/应用壳 | 命令检索、插件模式、窗口复用、快捷键 | 总锁、阻塞 receiver、调试打印、日志 guard | 先建设 task supervisor 和异步 command boundary |
| 剪贴板 | 文本/图片/文件、FTS、置顶、过滤 | 默认全量保存、敏感只标记、图片孤儿、双 store | 隐私优先，建立 BlobStore 和单 repository |
| SSH/FTP | profile、终端、文件传输、多认证 | 主机密钥全信任、凭据明文、密码明显、原始日志 | known_hosts + SecretStore 完成前阻止正式发布 |
| API 调试器 | collection、环境、变量、history、导入、代码生成 | URL 编码、真取消、流式 body、秘密明文 | 迁到 async client，统一 redaction policy |
| HTTP 抓包 | HTTP/HTTPS MITM、mock、过滤 | 开放代理、CA 权限、聚合 body、上下文相关性 | 默认 loopback，明确“远程代理”安全模式 |
| 下载管理 | 排队、暂停、续传、限速、分类 | 路径逃逸、续传校验、假重试、删除竞争 | 以 `.part` + 状态机 + 校验元数据重做 worker |
| 快速启动 | 自定义动作、参数、历史、停止 | 破坏性直达、macOS 强绑定、输出无界 | 加风险等级、平台 capability 和资源上限 |
| 图片压缩 | 导入、批量、格式压缩、覆盖 | 非原子覆盖、无像素上限、剪贴板临时文件无策略 | 临时文件原子替换，统一 ImageLimits/TempStore |
| 二维码 | 生成、复制、扫描、历史 | 敏感内容默认明文历史、解码无显式像素限额 | 历史 opt-in，复用 ImageLimits |
| 防窥 | 多显示器全屏遮盖 | 非 Esc 关闭失效、自定义图不可编辑 | session RAII 和完整设置交互 |
| 托盘网速 | 采样、托盘展示、IP 信息 | 外部请求未告知、detached 周期任务 | 公网 IP opt-in，任务可停止 |
| 系统设置/主题 | 模式持久化、系统主题同步 | 测试与 macOS 默认值漂移 | 明确平台默认策略并修复测试/迁移 |
| JSON 工具 | 核心解析逻辑未发现发布阻断项 | 深层/大 JSON 资源预算和 UI 回归不足 | 增加深度、大小限制和 property/fuzz test |
| About/GPUI demo | 基础展示和实验组件 | demo 进入正式包、零测试 | demo 移出 release，About 保持纯展示 |

## 9. GPUI 专项整改规范

### 9.1 UI 线程规则

1. GPUI `Entity` 和 view 状态只在 UI 线程修改。
2. `render`、事件回调和持有 `&mut App/Context` 的代码不得执行文件、SQLite、网络、进程等待或阻塞锁。
3. UI 回调只提交领域命令；后台任务返回不可变 DTO，再通过 `AsyncApp::update` 更新 entity。
4. 锁内不得调用插件、用户 callback、`cx.update` 或 I/O。

### 9.2 Task 所有权规则

1. 每个重复任务必须有 owner、取消 token、join 策略和最大退出时间。
2. 窗口级任务存入 view/entity，窗口销毁自动取消。
3. 插件级任务存入 service runtime，`Plugin::shutdown` 等待收敛。
4. 进程级任务存入 `BackgroundSupervisor`，应用退出统一停止。
5. `detach()` 需要代码注释说明为何允许任务超过 owner 生命周期，并接受结构化审查。

### 9.3 输入组件完成定义

- 单行、密码、多行分别有明确 API，不把无效 flag 暴露给调用者。
- grapheme、UTF-8、UTF-16、Rope offset 和 shaped glyph offset 的转换集中在一个模块并有 property test。
- disabled/read-only 对所有写路径一致；masked 不影响真实 selection，但不泄露绘制和 accessibility value。
- IME composition、undo/redo、粘贴、拖选、滚动、软换行和焦点都有 macOS/Windows 测试。
- 任何动态内容变化不得改变固定控件尺寸或造成点击区域漂移。

## 10. 目标架构

```text
Composition Root (qingqi)
  |
  +-- App Shell (GPUI main thread)
  |     WindowRouter / CommandPalette / TrayHost / ThemeHost
  |
  +-- Feature Catalog
  |     FeatureDescriptor (纯数据、版本化、无 I/O)
  |     FeatureViewFactory (仅创建 GPUI entity)
  |     FeatureCommandHandler (异步领域命令)
  |
  +-- Runtime Services
  |     TaskSupervisor / Cancellation / EventBus / Metrics
  |     SecretStore / AtomicFileStore / BlobStore
  |     DatabaseService / MigrationRegistry
  |
  +-- Platform Capabilities
        Clipboard / Hotkey / Tray / Credential / Process / Display
        每个能力有 cfg 实现和 Unsupported 明确返回
```

### 10.1 推荐边界

- `qingqi-plugin`：只保留稳定的 feature contract 和 DTO，不承载具体业务数据库逻辑。
- `qingqi-core`：命令索引、feature catalog、调度和策略；不直接持有长时间可变插件对象。
- `qingqi-app`：GPUI composition、窗口和生命周期；不实现功能业务规则。
- `qingqi-ui`：小而完整的 design-system 组件；代码编辑器能力另建 crate。
- `qingqi-platform`：所有 OS 资源的 RAII wrapper、能力检测和线程约束。
- `feature-*`：按 `model / repository / service / ui / tests` 组织，service 不依赖 GPUI，ui 不直接访问 SQLite。

### 10.2 不建议一次性重写

先通过 P0/P1 修复建立安全边界，再逐功能迁移。推荐顺序是下载和图片原子文件层、TaskSupervisor、MigrationRegistry、SecretStore、输入组件，最后才调整插件抽象。这样可以持续交付并降低大规模重构风险。

## 11. 代码与文档规范

### 11.1 Rust 代码规范

1. 所有提交必须通过 `cargo fmt --all -- --check`。
2. workspace 本地代码必须 Clippy 零告警；临时 allow 要附 issue、原因和删除日期。
3. 生产代码禁止裸 `unwrap`/`expect`，除非不可变式在同一位置有注释并有测试证明。
4. 错误使用有领域语义的 enum；边界处用 `anyhow::Context`，UI 展示稳定错误码和恢复动作。
5. 单文件超过 1,000 行必须给出保留理由；目标是 service/repository 模块小于 800 行、单 view 小于 600 行。
6. 不在日志中输出 secret、body、clipboard 原文和终端数据；敏感类型实现 redacted Debug。
7. 重复任务、OS thread、临时文件和 native handle 必须 RAII。
8. `unsafe_op_in_unsafe_fn` 提升为 deny；每个 unsafe block 有可验证 SAFETY 说明。
9. 时间、大小、条数、并发和重试均使用有界配置，禁止无界 `Vec`、channel 和 output collection。
10. 解析 URL、HTTP header、JSON、路径和 shell 参数时使用结构化库，不手工字符串拼接。

### 11.2 源码文档规范

本节是独立建议，没有参考已有文档。

1. 每个 crate 根使用 `//!` 说明职责、依赖方向、线程模型和非目标。
2. `qingqi-plugin`、`qingqi-core`、`qingqi-platform` 先启用 `#![warn(missing_docs)]`，清零后改为 deny；feature crate 至少对公共跨 crate API启用。
3. public API 必须写明：错误、panic、取消、线程、阻塞、资源所有权和安全约束。
4. `unsafe fn` 必须有 `# Safety`；涉及凭据、CA、代理、剪贴板的 API 必须写数据分级。
5. 示例应为可运行 doctest；平台专属示例使用正确 cfg，而不是全部 ignore。

### 11.3 工程文档最小集合

每个功能的设计说明至少包含：

- 目标、非目标和核心用户流程。
- 平台支持矩阵及 Unsupported 行为。
- 数据模型、数据分级、保存位置、保留时间和删除语义。
- 网络 endpoint、发送字段、权限和用户授权时机。
- 状态机、取消、重试、超时和崩溃恢复。
- schema 版本、迁移、备份和回滚。
- 安全威胁模型及滥用场景。
- 测试矩阵、性能预算和可观测指标。

需要优先形成的 ADR：

1. ADR-001 插件到底是内置 feature 还是外部扩展。
2. ADR-002 GPUI 任务所有权和取消模型。
3. ADR-003 SecretStore 与本地数据分级。
4. ADR-004 SQLite MigrationRegistry 和恢复策略。
5. ADR-005 Blob/临时文件生命周期。
6. ADR-006 跨平台 capability 与正式支持范围。

## 12. 测试整改方案

### 12.1 测试金字塔

| 层级 | 内容 | 目标 |
|---|---|---|
| 单元 | 解析、状态机、路径净化、编码、selection、迁移步骤 | 快、无网络、确定性 |
| 组件 | GPUI 输入、按钮状态、窗口关闭、主题切换 | 真实 entity/action/render |
| 集成 | SQLite fixture、本地 HTTP/SSH server、文件故障注入 | 覆盖边界协议 |
| E2E | macOS/Windows 启动器、托盘、快捷键、安装包 | 验证用户流程和平台行为 |
| 安全/性能 | fuzz、资源上限、权限、秘密扫描、长稳 | 防止 DoS 和数据泄露 |

### 12.2 必须新增的回归用例

1. 下载：恶意文件名、错误 Content-Range、ETag 变化、200 回退、取消与删除竞争。
2. SSH：known-hosts 首次确认、匹配、变更拒绝；数据库和日志秘密扫描。
3. 抓包：loopback 默认、CA 权限、header 脱敏、body 流式上限、并发关联。
4. 图片：编码/flush/rename 故障下源文件不变；超大像素和解压炸弹限制。
5. 输入：Unicode grapheme、UTF-16 IME、masked、read-only、disabled、undo/redo、拖选和滚动。
6. API：查询/表单特殊字符、真取消、大文件 streaming、history quota。
7. 剪贴板：敏感内容策略、删除/淘汰 blob、SQLite 升级、关闭捕获。
8. 防窥：Esc、关闭 view、关闭插件、app shutdown、多显示器部分失败。
9. Windows hook：重复安装/卸载、退出、进程结束和资源计数。

### 12.3 测试执行原则

- 把外网 TLS 测试改为本地 CA、本地 upstream 和本地 client，不依赖互联网或 `curl`。
- 历史版本数据库 fixture 入库，禁止测试运行时临时推测旧 schema。
- 对 URL、路径、Rope offset 和迁移使用 `proptest`；对 cURL/OpenAPI/JSON parser 使用 `cargo-fuzz` 或 `arbitrary`。
- 初期目标 workspace 行覆盖 70%，安全关键模块分支覆盖 85%；覆盖率不能替代上述场景测试。
- 所有测试临时目录由 RAII fixture 清理，测试不能写用户真实配置目录。

## 13. CI/CD 与供应链整改

### 13.1 PR 必过命令

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked --no-fail-fast
cargo doc --workspace --all-features --no-deps --locked
cargo audit --deny warnings
cargo deny check advisories bans licenses sources
```

注：如果 GPUI 在 Linux runner 缺少系统依赖，fmt/audit/deny 放 Ubuntu，主 check/clippy/test 放 macOS；Windows 至少执行 `cargo check --workspace --all-targets --locked` 和平台单测。不得因 runner 配置困难直接删除门禁。

### 13.2 Workflow 结构

```text
quality
  fmt
  audit + deny
  macos-check-clippy-test
  windows-check-test

release (仅 tag)
  needs: quality
  verify-version
  build --locked
  sign + notarize
  smoke-test packaged app
  checksum + SBOM + provenance
  publish
```

### 13.3 供应链政策

- Rust、cargo-bundle、cargo-audit、cargo-deny 和 runner image 都记录明确版本。
- GitHub Actions 使用完整 SHA，并由 Dependabot/Renovate 更新。
- 禁止未知 git dependency；新增 source/许可证必须通过 review。
- 每周定时 audit，每月依赖升级，安全补丁按 SLA 处理。
- `russh` 升级独立 PR，包含连接、SFTP、known-hosts、key auth 和跨平台回归。

## 14. 分阶段路线图

### 阶段 0：0-2 周，恢复发布可信度

| 工作 | 交付物 | 退出标准 |
|---|---|---|
| 暂停正式发布 | 分支保护和 release freeze | P0 未清零不能打标签 |
| 修复下载路径 | 净化器、containment、并发创建测试 | 恶意 corpus 全通过 |
| 修复图片覆盖 | 原子替换和故障注入 | 源文件在所有失败场景不变 |
| 收紧抓包默认 | loopback、CA 0600、基础脱敏 | 非本机不可连接，私钥权限合格 |
| SSH 最小安全闭环 | known-hosts、密码遮罩、停用原始日志 | 主机变更被拒绝，UI 无明文 |
| 建立 quality CI | fmt/check/clippy/test 基础 job | 当前主线全部绿色 |
| 修复主题测试 | 明确默认主题契约 | workspace 测试 0 失败 |
| 保持日志 guard | 生命周期测试 | 运行期日志持续落盘 |

预计 22-32 人日，可由 3-4 名工程师并行。

### 阶段 1：2-6 周，补齐可靠性和隐私

| 工作流 | 内容 | 退出标准 |
|---|---|---|
| TaskSupervisor | 托盘、快捷键、主题、电源和网络采样迁移 | task/thread 数稳定，可完整 shutdown |
| 下载状态机 | `.part`、续传身份、重试、join 后删除 | 本地 server 故障矩阵通过 |
| API async 化 | URL 编码、streaming、abort、quota | 取消真实生效，大 body 有界 |
| SecretStore | SSH/API/代理秘密引用与迁移 | DB/日志秘密扫描通过 |
| 剪贴板隐私 | opt-in、敏感排除、BlobStore | 删除无孤儿，敏感默认不留存 |
| 输入组件 | 完成单行/密码基础，隔离多行 editor | UI 组件测试覆盖关键动作 |
| Windows hook | 正确退出和资源回收 | Windows 重复生命周期测试通过 |
| 发布签名 | macOS/Windows 签名、checksum、SBOM | 安装包系统验证通过 |

预计 45-65 人日。

### 阶段 2：6-12 周，架构收敛

| 工作流 | 内容 | 退出标准 |
|---|---|---|
| MigrationRegistry | 版本、事务、checksum、fixture、恢复 | 所有功能迁移统一注册 |
| 插件边界 ADR | 内置 feature 或隔离外部插件决策 | 删除假接口或形成版本化 IPC |
| 超大模块拆分 | API、快速启动、SSH、图片、launcher | 业务服务可无 GPUI 单测 |
| 依赖治理 | russh 升级、deny、重复版本评估 | audit/deny 定时绿色 |
| 文档门禁 | rustdoc、ADR、feature spec 模板 | 核心 crate missing_docs 清零 |
| 性能预算 | 启动、内存、CPU、存储、线程基线 | nightly 趋势可见且有阈值 |
| demo 隔离 | `dev-tools` feature/examples app | release 二进制不含实验场 |

预计 35-50 人日。

## 15. 可直接建立的整改 Backlog

本节是面向负责人估算和立项的主题级 backlog，不是低级模型执行工单。`SEC-*`、`UI-*` 等条目范围较大，必须先映射到第 0 节已有 `FIX-*`，或在高级设计完成后继续拆分；不得整行复制给模型要求一次实现。

| ID | Issue 标题 | 优先级 | 依赖 | 估算 |
|---|---|---:|---|---:|
| SEC-001 | SSH known_hosts 严格校验和首次指纹确认 | P0 | 无 | 4-6d |
| SEC-002 | SSH/API 凭据迁入系统 SecretStore | P0/P1 | ADR-003 | 8-12d |
| SEC-003 | HTTP proxy 默认 loopback 与远程认证模式 | P0 | 无 | 3-5d |
| SEC-004 | CA key 权限、轮换和清理 | P0 | SecretStore | 3-5d |
| DL-001 | 下载文件名净化与目录 containment | P0 | 无 | 3-5d |
| IMG-001 | 覆盖原图改为原子替换 | P0 | AtomicFileStore | 4-6d |
| CI-001 | PR quality workflow 和分支保护 | P0 | 告警清理 | 3-5d |
| UI-001 | Input masked/disabled/read-only 契约 | P1 | 无 | 5-7d |
| UI-002 | Input Unicode/IME/undo/redo 组件测试 | P1 | UI-001 | 5-8d |
| APP-001 | TaskSupervisor 和 async receiver bridge | P1 | ADR-002 | 5-8d |
| WIN-001 | LowLevelHook 正确 shutdown 和回收 | P1 | APP-001 可并行 | 3-5d |
| DL-002 | 下载状态机、续传校验和真实重试 | P1 | DL-001 | 7-10d |
| API-001 | async HTTP、结构化编码和真取消 | P1 | APP-001 | 8-12d |
| CLIP-001 | Clipboard BlobStore 和敏感捕获策略 | P1 | SecretStore | 7-10d |
| DB-001 | MigrationRegistry 与历史 fixture | P1/P2 | ADR-004 | 8-12d |
| QL-001 | 动作风险等级、平台 gate、输出 quota | P1 | APP-001 | 7-10d |
| REL-001 | 双平台签名、公证、SBOM 和 provenance | P1 | CI-001 | 4-7d |
| ARCH-001 | 明确插件定位并调整 contract | P2 | ADR-001 | 8-15d |
| DOC-001 | 核心 crate rustdoc 与文档门禁 | P2 | CI-001 | 5-8d |

## 16. 完成定义

一次整改只有同时满足以下条件才算完成：

1. 行为修复、错误状态和恢复路径均实现，不只是隐藏按钮或吞掉错误。
2. 原问题有自动化回归测试，测试在修复前可稳定复现。
3. macOS 和 Windows 相关路径均编译；宣称支持的平台有真实运行证据。
4. 安全变更包含数据迁移、旧数据处理、日志脱敏和威胁模型更新。
5. 任务类变更包含取消、join、drop 和应用 shutdown 测试。
6. 数据类变更包含升级、回滚、崩溃中断和备份恢复测试。
7. `fmt`、`check`、Clippy、test、doc、audit、deny 全绿。
8. 用户可见功能有明确的成功、进行中、取消、失败和重试状态。

## 17. 最终结论

项目当前最需要的不是继续增加 feature crate，而是把已有能力变成可信赖的桌面产品。现有分层足以支持渐进整改，不建议推倒重来。应先封住 SSH、代理、下载路径和原图覆盖四个高危边界，以 CI 阻止回归；随后统一任务、秘密、迁移和 blob 生命周期；最后再收敛插件抽象和超大模块。

按本方案执行后，12 周内可以把项目从“功能广但安全与生命周期不稳定”提升到“具备双平台持续发布基础”。如果跳过 P0/P1 而继续扩展功能，缺陷会通过共享输入、总锁、分散迁移和无界后台任务继续向所有功能传播，后续整改成本会明显上升。
