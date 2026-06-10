# 快速执行指令 - 给低级模型

> 这是简化的执行指令，可以直接复制给 Haiku 或其他低级模型

## 指令 #1：删除未使用函数（最简单）

```
任务：删除 qingqi-feature-ftp-sftp-ssh-client 中的未使用函数

步骤：
1. 执行：cargo check --package qingqi-feature-ftp-sftp-ssh-client 2>&1 | grep "never used"
2. 记录所有未使用的函数名和行号
3. 打开文件：crates/qingqi-feature-ftp-sftp-ssh-client/src/protocols.rs
4. 删除这些函数（包括上方的文档注释）：
   - SshConnection::open_sftp (约 line 411)
   - FtpConnection::quit (约 line 895)
   - connect_sftp (约 line 1044)
   - ftp_quit (约 line 1497)
   - 其他2个（根据第1步的输出）
5. 验证：cargo check --package qingqi-feature-ftp-sftp-ssh-client
   应该没有 "never used" 警告
6. 测试：cargo test --package qingqi-feature-ftp-sftp-ssh-client
7. 提交：
   git add crates/qingqi-feature-ftp-sftp-ssh-client/src/protocols.rs
   git commit -m "cleanup: 删除 ftp-sftp-ssh-client 中未使用的函数

删除了 6 个未使用的函数以消除编译警告

Co-Authored-By: Claude <noreply@anthropic.com>"

预期结果：6个warning消失，编译通过，测试通过
风险：极低
耗时：10-15分钟
```

---

## 指令 #2：修复 json-parser 硬编码颜色

```
任务：将 json-parser 中的硬编码颜色改为语义token

步骤：
1. 打开：crates/qingqi-feature-json-parser/src/view.rs
2. 查找：let bool_null_color = gpui::rgb(0x8B5CF6);
3. 在文件顶部添加导入：
   use qingqi_ui::ui;
4. 替换为：
   let bool_null_color = ui::accent_color(qingqi_plugin::PluginAccent::Violet);
5. 验证编译：cargo check --package qingqi-feature-json-parser
6. 运行程序：cargo run
   打开 JSON 解析器插件，检查颜色是否正常
7. 提交：
   git commit -m "style(json-parser): 使用语义token替换硬编码颜色

将 rgb(0x8B5CF6) 改为 ui::accent_color(Violet)
遵循 .claude/CLAUDE.md 样式规范

Co-Authored-By: Claude <noreply@anthropic.com>"

预期结果：消除1处硬编码颜色，颜色显示正常
风险：低
耗时：5-10分钟
```

---

## 指令 #3：修复简单的 clippy 警告

```
任务：修复 needless_return 类型的 clippy 警告

步骤：
1. 生成报告：
   cargo clippy --workspace --all-targets 2>&1 | grep "needless_return" > needless_return.txt
2. 查看文件：cat needless_return.txt
3. 对每个警告：
   - 打开对应文件
   - 找到函数
   - 删除不必要的 return 关键字
   例如：
   // 修改前
   fn foo() -> i32 {
       return 42;
   }
   // 修改后
   fn foo() -> i32 {
       42
   }
4. 每修改5-10个文件后验证：cargo check
5. 全部修改后测试：cargo test --workspace
6. 提交：
   git commit -m "clippy: 修复 needless_return 警告

删除不必要的 return 关键字，使代码更简洁

Co-Authored-By: Claude <noreply@anthropic.com>"

预期结果：减少20-50个警告
风险：极低
耗时：30-60分钟
```

---

## 指令 #4：修复 http-capture 锁 unwrap

```
任务：修复 http-capture 中的锁 unwrap

步骤：
1. 查找：
   rg "lock\(\)\.unwrap\(\)" crates/qingqi-feature-http-capture/src/

2. 对于 mock_engine.rs：
   - 打开文件
   - 找到 lock().unwrap()
   - 在文件顶部添加：use qingqi_plugin::lock_or_recover;
   - 替换为：
     let guard = lock_or_recover(&self.state, "mock_engine")?;
   - 确保所在函数返回 Result

3. 对于 engine.rs：
   - 同样处理

4. 验证：cargo check --package qingqi-feature-http-capture
5. 测试：cargo test --package qingqi-feature-http-capture
6. 手动测试：运行程序，测试HTTP抓包功能
7. 提交：
   git commit -m "fix(http-capture): 修复锁 unwrap，防止连锁 panic

使用 lock_or_recover 替代 lock().unwrap()
提高程序健壮性

Co-Authored-By: Claude <noreply@anthropic.com>"

预期结果：消除2个锁unwrap
风险：中
耗时：20-30分钟
注意：需要理解业务逻辑，如不确定请咨询
```

---

## 指令 #5：消除 json-parser unwrap

```
任务：消除 json-parser 中的 unwrap

步骤：
1. 查找：
   rg "\.unwrap\(\)" crates/qingqi-feature-json-parser/src/ --line-number

2. 对每个 unwrap：
   - 打开文件，跳到对应行
   - 分析：这是 Result 还是 Option？
   - 判断：能否用 ? 传播？能否用 unwrap_or？
   - 选择合适的替代方案：
     A. result? （如果函数返回 Result）
     B. option.unwrap_or_default()
     C. option.unwrap_or_else(|| ...)
     D. result.expect("说明为什么这是安全的")

3. 示例：
   // 如果是 serde_json::from_str(...).unwrap()
   // 改为
   let value = serde_json::from_str(...)
       .with_context(|| "parsing JSON")?;

4. 验证：cargo check --package qingqi-feature-json-parser
5. 测试：cargo test --package qingqi-feature-json-parser
6. 提交：
   git commit -m "fix(json-parser): 消除 unwrap，改进错误处理

使用 ? 传播错误，使用 unwrap_or 提供默认值

Co-Authored-By: Claude <noreply@anthropic.com>"

预期结果：消除3个unwrap
风险：中
耗时：15-20分钟
```

---

## 使用说明

### 给低级模型说：

**选项1（最简单）**：
```
请执行"指令 #1：删除未使用函数"。
严格按照步骤执行，每步完成后告诉我结果。
如果遇到问题立即停止并报告。
```

**选项2（简单）**：
```
请执行"指令 #2：修复 json-parser 硬编码颜色"。
按步骤执行，运行后截图给我看颜色是否正常。
```

**选项3（中等）**：
```
请执行"指令 #3：修复简单的 clippy 警告"。
先修复10个，验证通过后继续。
```

### 验证检查清单

每个指令执行后必须检查：
- [ ] 代码编译通过（cargo check）
- [ ] 测试通过（cargo test）
- [ ] 手动测试（如果是UI相关）
- [ ] git commit 成功
- [ ] 更新 quality-improvement-progress.md

### 遇到问题时

1. **编译失败**：回滚修改，报告错误信息
2. **测试失败**：回滚修改，报告失败的测试
3. **不确定如何修改**：停止，寻求帮助
4. **功能异常**：回滚修改，报告异常行为

---

## 快速参考

### 验证命令
```bash
# 检查单个包
cargo check --package qingqi-feature-xxx

# 检查整个workspace
cargo check --workspace

# 运行单个包测试
cargo test --package qingqi-feature-xxx

# 运行所有测试
cargo test --workspace -j 1 --quiet

# clippy检查
cargo clippy --workspace --all-targets
```

### 回滚命令
```bash
# 回滚未提交的修改
git checkout -- <file>

# 回滚最后一次提交
git reset --soft HEAD~1
```

### 提交模板
```bash
git commit -m "<type>(<scope>): <简短描述>

<详细说明>

Co-Authored-By: Claude <noreply@anthropic.com>"
```

type: fix, feat, refactor, style, test, docs, chore, cleanup
scope: 功能模块名称（如 json-parser, http-capture）
