# Qingqi 代码质量改进执行计划

> 日期：2026-06-10  
> 目标：渐进式提升代码质量，每个任务独立可执行，风险可控  
> 执行者：低级模型（Haiku/Sonnet）或人工

## 总览

当前技术债：
- 677个 clippy 警告（主要是 unwrap）
- 20+ 处硬编码颜色
- 19处锁 unwrap
- 6个未使用函数
- 部分超大文件（4000+ 行）

改进策略：**从低风险到高风险，从小范围到大范围**

---

## Phase 1: 代码清理（低风险，1-2天）

### Task 1.1: 删除未使用函数 ⭐ 优先级：P0

**目标**：修复 cargo check 的 6 个 warning

**位置**：`crates/qingqi-feature-ftp-sftp-ssh-client/src/protocols.rs`

**待删除函数**：
1. `SshConnection::open_sftp` (line ~411)
2. `FtpConnection::quit` (line ~895)
3. `connect_sftp` (line ~1044)
4. `ftp_quit` (line ~1497)
5. 另外2个（运行 cargo check 确认行号）

**执行步骤**：
```bash
# 1. 确认未使用函数列表
cargo check --package qingqi-feature-ftp-sftp-ssh-client 2>&1 | grep "never used"

# 2. 对每个函数：
#    - 打开 protocols.rs
#    - 搜索函数名（如 "fn open_sftp"）
#    - 删除整个函数（包括文档注释）
#    - 保存

# 3. 验证
cargo check --package qingqi-feature-ftp-sftp-ssh-client
# 应该没有 "never used" 警告

# 4. 测试
cargo test --package qingqi-feature-ftp-sftp-ssh-client

# 5. 提交
git add crates/qingqi-feature-ftp-sftp-ssh-client/src/protocols.rs
git commit -m "cleanup: 删除 ftp-sftp-ssh-client 中未使用的函数

删除了 6 个未使用的函数：
- SshConnection::open_sftp
- FtpConnection::quit
- connect_sftp
- ftp_quit
- （列出其他函数）

Co-Authored-By: Claude <noreply@anthropic.com>"
```

**预期结果**：
- ✅ 6 个 warning 消失
- ✅ cargo check 通过
- ✅ 测试通过

**风险**：极低（已确认未使用）

---

### Task 1.2: 清理简单的 clippy 警告 ⭐ 优先级：P1

**目标**：修复非 unwrap 类的简单警告

**执行步骤**：
```bash
# 1. 生成 clippy 报告
cargo clippy --workspace --all-targets 2>&1 | tee clippy-report.txt

# 2. 过滤简单警告（低风险）
grep -E "needless_return|redundant_clone|unnecessary_mut|single_match" clippy-report.txt

# 3. 按提示逐个修复
#    - needless_return: 删除多余的 return
#    - redundant_clone: 删除不必要的 .clone()
#    - unnecessary_mut: 删除不需要的 mut
#    - single_match: 改为 if let

# 4. 验证每个文件
cargo clippy --package <affected-crate>

# 5. 分批提交（每种警告类型一个commit）
git commit -m "clippy: 修复 needless_return 警告

Co-Authored-By: Claude <noreply@anthropic.com>"
```

**预期结果**：
- ✅ 减少 50-100 个简单警告
- ✅ 代码更简洁

**风险**：极低（clippy 自动修复建议）

---

## Phase 2: 样式规范化（中风险，2-3天）

### Task 2.1: 清除硬编码颜色 ⭐ 优先级：P1

**目标**：消除硬编码颜色，统一使用语义 token

**范围**：
- `crates/qingqi-feature-json-parser/src/view.rs` (1处)
- `crates/qingqi-feature-ftp-sftp-ssh-client/src/terminal.rs` (19处)

**执行步骤**：

#### 2.1.1 json-parser（简单，先做）

```bash
# 1. 打开文件
# crates/qingqi-feature-json-parser/src/view.rs

# 2. 查找硬编码颜色
rg "rgb\(0x" crates/qingqi-feature-json-parser/src/view.rs

# 3. 替换
# 找到：let bool_null_color = gpui::rgb(0x8B5CF6);
# 改为：
use qingqi_ui::ui;
let bool_null_color = ui::accent_color(qingqi_plugin::PluginAccent::Violet);

# 4. 验证
cargo check --package qingqi-feature-json-parser
cargo run  # 目视检查颜色是否正确

# 5. 提交
git commit -m "style(json-parser): 使用语义token替换硬编码颜色

将 rgb(0x8B5CF6) 改为 ui::accent_color(Violet)
遵循 .claude/CLAUDE.md 样式规范

Co-Authored-By: Claude <noreply@anthropic.com>"
```

#### 2.1.2 ftp-ssh-client 终端配色（复杂，需设计）

```bash
# 1. 分析现状
# crates/qingqi-feature-ftp-sftp-ssh-client/src/terminal.rs
# 19个硬编码颜色，这是终端ANSI配色方案

# 2. 设计方案（需要决策）
# 选项A：在 qingqi-ui/src/theme.rs 添加终端配色方案
# 选项B：在插件内部定义配色常量（可接受，因为是终端特定）
# 选项C：使用外部配置文件

# 推荐：选项B（最简单，风险最低）

# 3. 执行（选项B）
# 在 terminal.rs 顶部添加：
const TERMINAL_COLORS: TerminalColorScheme = TerminalColorScheme {
    black: rgb(0x1d, 0x1f, 0x21),
    red: rgb(0xcc, 0x66, 0x66),
    // ... 其他颜色
};

fn map_named_color(color: NamedColor) -> Rgba {
    match color {
        NamedColor::Black => TERMINAL_COLORS.black,
        NamedColor::Red => TERMINAL_COLORS.red,
        // ...
    }
}

# 4. 添加注释说明这是终端ANSI标准配色
# 5. 验证终端显示正确
# 6. 提交
```

**预期结果**：
- ✅ 消除 20 处硬编码颜色违规
- ✅ 样式规范化

**风险**：中等（需要视觉验证）

---

### Task 2.2: 统一命名（XxxPanel → XxxView）⭐ 优先级：P2

**目标**：将旧命名统一为规范命名

**执行步骤**：
```bash
# 1. 搜索 Panel 命名
rg "struct \w+Panel" crates/ --type rust

# 2. 对每个找到的 Panel：
#    - 重命名结构体：XxxPanel → XxxView
#    - 更新所有引用（IDE 重构工具）
#    - 更新文档注释

# 3. 搜索 Element 命名
rg "struct \w+Element" crates/ --type rust

# 4. 同样重命名

# 5. 逐个 crate 提交
git commit -m "refactor(clipboard): 重命名 ClipboardPanel → ClipboardView

统一使用 XxxView 命名约定
遵循 .claude/CLAUDE.md 命名规范

Co-Authored-By: Claude <noreply@anthropic.com>"
```

**预期结果**：
- ✅ 命名一致性
- ✅ 符合规范

**风险**：低（纯重命名，IDE可自动完成）

---

## Phase 3: 错误处理改进（高风险，3-5天）

### Task 3.1: 修复锁 unwrap（选择1个小 crate）⭐ 优先级：P1

**目标**：消除锁 unwrap，防止连锁 panic

**范围选择**：从最少的开始
- `qingqi-feature-http-capture/src/mock_engine.rs` (1处) ← 先做这个
- `qingqi-feature-http-capture/src/engine.rs` (1处)
- `qingqi-feature-download-manager/src/service.rs` (4处)
- `qingqi-feature-image-compress/src/view.rs` (15处) ← 最后做

**执行步骤（以 mock_engine.rs 为例）**：

```bash
# 1. 找到锁 unwrap
rg "lock\(\)\.unwrap\(\)" crates/qingqi-feature-http-capture/src/mock_engine.rs

# 2. 分析上下文
#    - 这个锁保护什么数据？
#    - 在哪个线程/函数中使用？
#    - 失败时应该如何处理？

# 3. 替换策略
# 选项A：使用 lock_or_recover（推荐）
use qingqi_plugin::lock_or_recover;
let guard = lock_or_recover(&self.state, "mock_engine")?;

# 选项B：日志+降级
let guard = self.state.lock().unwrap_or_else(|e| {
    tracing::error!("mock engine lock poisoned: {}", e);
    e.into_inner()  // 恢复锁，继续使用（但数据可能不一致）
});

# 选项C：返回错误
let guard = self.state.lock()
    .map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;

# 4. 根据上下文选择合适的策略

# 5. 修改代码

# 6. 验证
cargo check --package qingqi-feature-http-capture
cargo test --package qingqi-feature-http-capture

# 7. 提交
git commit -m "fix(http-capture): 修复 mock_engine 中的锁 unwrap

使用 lock_or_recover 替代 lock().unwrap()
防止锁中毒导致的连锁 panic

Co-Authored-By: Claude <noreply@anthropic.com>"
```

**预期结果**：
- ✅ 消除 1-2 个锁 unwrap
- ✅ 提高健壮性

**风险**：中高（需要理解业务逻辑）

**注意**：
- 每次只改1个文件
- 充分测试
- 如果不确定，先询问维护者

---

### Task 3.2: 消除部分 unwrap（选择1个小 feature）⭐ 优先级：P2

**目标**：消除 Result/Option 的 unwrap

**范围选择**：从最少的开始
- `qingqi-feature-json-parser` (3个 unwrap) ← 先做这个
- `qingqi-feature-about` (可能很少)
- `qingqi-feature-system-settings` (0个 unwrap，已符合规范)

**执行步骤（以 json-parser 为例）**：

```bash
# 1. 找到所有 unwrap
rg "\.unwrap\(\)" crates/qingqi-feature-json-parser/src/ --line-number

# 2. 对每个 unwrap 分析：
#    - 这是什么类型？Result 还是 Option？
#    - 什么情况下会 None/Err？
#    - 失败时应该如何处理？

# 3. 替换策略
# A. 如果确实是不变量（99.9%不会失败）
let value = result.expect("invariant: json must be valid here");
# 并添加注释说明为什么这是安全的

# B. 如果可以传播错误
let value = result?;

# C. 如果可以提供默认值
let value = option.unwrap_or_default();
let value = option.unwrap_or_else(|| compute_default());

# D. 如果需要降级处理
let value = match result {
    Ok(v) => v,
    Err(e) => {
        tracing::warn!("failed to parse: {}", e);
        return;  // 或其他降级逻辑
    }
};

# 4. 逐个修改

# 5. 验证
cargo check --package qingqi-feature-json-parser
cargo test --package qingqi-feature-json-parser

# 6. 提交
git commit -m "fix(json-parser): 消除 unwrap，改进错误处理

- 使用 ? 传播错误
- 使用 unwrap_or_default 提供默认值
- 添加 expect 说明不变量

Co-Authored-By: Claude <noreply@anthropic.com>"
```

**预期结果**：
- ✅ 消除 3 个 unwrap
- ✅ 更好的错误处理

**风险**：中等（需要理解代码逻辑）

---

## Phase 4: 架构重构（高风险，5-10天）

### Task 4.1: 拆分 api-debugger/view.rs（4955行）⭐ 优先级：P2

**目标**：将超大文件拆分为模块化结构

**执行步骤**：

```bash
# 1. 分析现有结构
# 打开 crates/qingqi-feature-api-debugger/src/view.rs
# 识别主要部分：
#   - 主视图结构
#   - 请求列表区
#   - 请求详情区
#   - 编辑器区
#   - 工具栏
#   - 辅助函数

# 2. 设计目标结构
mkdir -p crates/qingqi-feature-api-debugger/src/view
# 目标：
#   view/mod.rs          - 主视图入口（200行）
#   view/request_list.rs - 请求列表（500行）
#   view/detail.rs       - 请求详情（800行）
#   view/editor.rs       - 编辑器区（600行）
#   view/toolbar.rs      - 工具栏（300行）
#   view/shared.rs       - 共享组件（400行）
#   view/vm.rs           - ViewModel（500行）

# 3. 执行拆分（逐步，每步验证）
# 步骤1: 创建 view/mod.rs，移动主结构
# 步骤2: 创建 view/vm.rs，移动 ViewModel
# 步骤3-N: 逐个模块拆分

# 4. 每个模块拆分后立即验证
cargo check --package qingqi-feature-api-debugger

# 5. 完整测试
cargo test --package qingqi-feature-api-debugger
cargo run  # 手动测试功能

# 6. 提交（分多个commit）
git commit -m "refactor(api-debugger): 拆分view模块 - 阶段1: 创建模块骨架"
git commit -m "refactor(api-debugger): 拆分view模块 - 阶段2: 提取ViewModel"
# ...
```

**预期结果**：
- ✅ 4955行 → 7个文件，每个500-800行
- ✅ 职责清晰
- ✅ 可维护性提升

**风险**：高（大规模重构）

**建议**：
- 分多个小步骤
- 每步都验证编译和测试
- 考虑先征求维护者意见

---

### Task 4.2: 拆分 ftp-ssh-client/view/mod.rs（3861行）⭐ 优先级：P2

**目标**：按职责拆分为多个模块

**设计目标结构**：
```
view/
  mod.rs           - 主视图入口
  connection.rs    - 连接管理
  file_browser.rs  - 文件浏览器
  terminal.rs      - 终端视图（已独立？）
  editor.rs        - 编辑器
  toolbar.rs       - 工具栏
  vm.rs            - ViewModel
```

**执行步骤**：类似 Task 4.1

**风险**：高

---

## Phase 5: 测试补充（中风险，持续进行）

### Task 5.1: 为 qingqi-core 添加测试 ⭐ 优先级：P2

**目标**：提升核心模块测试覆盖率

**范围**：
- `PluginManager` 基本操作
- 命令排序逻辑
- 使用统计

**执行步骤**：

```bash
# 1. 查看现有测试
ls crates/qingqi-core/src/*test* crates/qingqi-core/tests/

# 2. 识别未测试的关键路径

# 3. 编写测试
# crates/qingqi-core/src/plugin_manager.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registration() {
        // 测试插件注册流程
    }

    #[test]
    fn test_command_sorting() {
        // 测试命令排序
    }
}

# 4. 运行测试
cargo test --package qingqi-core

# 5. 提交
git commit -m "test(core): 添加 PluginManager 单元测试

测试插件注册、命令排序等核心功能

Co-Authored-By: Claude <noreply@anthropic.com>"
```

**预期结果**：
- ✅ 核心逻辑有测试保护
- ✅ 重构更安全

**风险**：低

---

## 执行优先级总结

### 立即可做（P0，1-2天）
1. ✅ Task 1.1: 删除未使用函数（极低风险）
2. ✅ Task 1.2: 修复简单 clippy 警告（极低风险）

### 近期推荐（P1，2-5天）
3. ✅ Task 2.1.1: json-parser 硬编码颜色（低风险）
4. ✅ Task 3.1: 修复 1-2 个锁 unwrap（中风险）
5. ✅ Task 3.2: 消除 json-parser 的 unwrap（中风险）

### 中期规划（P2，1-2周）
6. ⚠️ Task 2.1.2: ftp-ssh-client 终端配色（需设计）
7. ⚠️ Task 2.2: 统一命名（低风险但工作量大）
8. ⚠️ Task 5.1: 补充核心测试（持续进行）

### 长期规划（P2-P3，2-4周）
9. ⚠️ Task 4.1: 拆分 api-debugger（高风险，需谨慎）
10. ⚠️ Task 4.2: 拆分 ftp-ssh-client（高风险，需谨慎）

---

## 执行建议

### 对低级模型说：

#### 执行 Task 1.1（最简单）
```
请执行 Task 1.1：删除 qingqi-feature-ftp-sftp-ssh-client 中的未使用函数。

步骤：
1. 运行 `cargo check --package qingqi-feature-ftp-sftp-ssh-client 2>&1 | grep "never used"` 查看未使用函数列表
2. 打开 crates/qingqi-feature-ftp-sftp-ssh-client/src/protocols.rs
3. 删除列表中的所有未使用函数（包括文档注释）
4. 运行 `cargo check --package qingqi-feature-ftp-sftp-ssh-client` 验证
5. 运行 `cargo test --package qingqi-feature-ftp-sftp-ssh-client` 测试
6. 提交代码，commit message 按计划中的格式

请开始执行。
```

#### 执行 Task 2.1.1（简单）
```
请执行 Task 2.1.1：清除 json-parser 的硬编码颜色。

步骤：
1. 打开 crates/qingqi-feature-json-parser/src/view.rs
2. 查找 `rgb(0x8B5CF6)`
3. 替换为使用 qingqi_ui::ui 的语义 token
4. 验证编译和运行
5. 提交

参考计划中的详细步骤。
```

### 风险控制

**每个任务必须**：
1. ✅ 先在独立分支执行
2. ✅ 编译通过
3. ✅ 测试通过
4. ✅ 手动验证（如果是UI相关）
5. ✅ Code review（重大修改）

**如果遇到问题**：
- 立即停止
- 回滚到上一个稳定状态
- 记录问题
- 寻求高级模型或人工帮助

---

## 进度跟踪

创建一个 `docs/quality-improvement-progress.md` 跟踪进度：

```markdown
# 代码质量改进进度

## Phase 1: 代码清理
- [ ] Task 1.1: 删除未使用函数（开始日期：____，完成日期：____）
- [ ] Task 1.2: 简单 clippy 警告（开始日期：____，完成日期：____）

## Phase 2: 样式规范化
- [ ] Task 2.1.1: json-parser 硬编码颜色
- [ ] Task 2.1.2: ftp-ssh-client 终端配色
- [ ] Task 2.2: 统一命名

## Phase 3: 错误处理改进
- [ ] Task 3.1: 修复锁 unwrap
- [ ] Task 3.2: 消除 json-parser unwrap

## Phase 4: 架构重构
- [ ] Task 4.1: 拆分 api-debugger
- [ ] Task 4.2: 拆分 ftp-ssh-client

## Phase 5: 测试补充
- [ ] Task 5.1: qingqi-core 测试
```

---

## 总结

这个计划：
- ✅ 从低风险到高风险
- ✅ 从小范围到大范围
- ✅ 每个任务独立可执行
- ✅ 详细的步骤说明
- ✅ 明确的验证标准
- ✅ 适合低级模型执行

**建议执行顺序**：Task 1.1 → Task 1.2 → Task 2.1.1 → Task 3.1 → Task 3.2

每完成一个任务，更新进度文档并评估下一步。
