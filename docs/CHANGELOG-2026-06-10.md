# 文档清理与规范更新 - 变更总结

> 日期：2026-06-10  
> 目标：清理过期文档，更新核心规范，让项目健康推进

## 变更内容

### 1. 文档清理

#### 归档的文档（移至 `docs/archive/`）

以下文档已完成历史使命，归档保留作参考：

- ✅ `gpt-5.4-workspace-split-execution-plan.md` - workspace拆分执行计划（已完成）
- ✅ `codebase-deep-audit-report.md` - 代码深度审计报告（2026-06-06）
- ✅ `codebase-optimization-report.md` - 代码优化审计报告（2026-06-06）
- ✅ `codebase-remediation-blueprint.md` - 代码整改蓝图（2026-06-06）
- ✅ `plugin-ui-optimization-plan.md` - 插件UI优化方案（2026-05-30）

#### 保留的核心文档

- ✅ `docs/workspace-split-guide.md` - 架构设计权威文档
- ✅ `docs/conventions.md` - 编码规范（详细版）
- ✅ `docs/gpui-component-guide.md` - GPUI组件使用指南

### 2. 核心文档更新

#### `AGENT.md` - 全面重写

**主要变更**：
- ❌ 删除所有 "pre-split" 和 `src/*` 旧路径引用
- ✅ 更新为当前 workspace 结构（19个crate）
- ✅ 明确 crate 职责边界表
- ✅ 添加架构不变量（4条必须守住的规则）
- ✅ 添加开发规范速查（分层、命名、UI性能、异步）
- ✅ 添加依赖边界验证命令
- ✅ 添加常见反模式与正确示例
- ✅ 更新文档权威说明

**新增内容**：
- Crate 职责边界表（7个核心crate）
- 插件内部分层结构（plugin/service/store/model/view/manifest）
- 高性能 UI 铁律（5条）
- 异步规则（generation guard、去抖、锁纪律）
- 命名约定表（XxxPlugin/XxxView/XxxViewModel等）
- UI组件选型优先级（gpui-component > wrapper > div）
- 错误处理规范（禁止unwrap）
- 常见反模式对比（❌ vs ✅）

#### `README.md` - 结构性重写

**主要变更**：
- ❌ 删除过时的 `crates/qingqi/src/*` 路径描述
- ✅ 更新为 workspace 多包架构说明
- ✅ 添加完整的 crate 目录树
- ✅ 添加架构设计原则（4条）
- ✅ 添加 crate 职责表
- ✅ 添加依赖边界验证命令

**新增内容**：
- 项目架构可视化目录树
- 架构设计原则（插件化、依赖单向、核心独立、SDK稳定）
- Crate 职责与稳定性说明表
- 开发环境配置说明
- 依赖边界验证脚本

### 3. 架构验证

所有架构不变量验证通过 ✅：

```bash
# I1 - 核心零插件可运行
cargo tree -p qingqi-core | rg "qingqi-feature"   # ✅ 无输出
cargo tree -p qingqi-app | rg "qingqi-feature"    # ✅ 无输出

# I2 - SDK 不泄漏宿主
cargo tree -p qingqi-plugin | rg "qingqi-(core|app|platform|feature)"  # ✅ 无输出

# I3 - 代码中无非法引用
rg -n "\bqingqi_feature_" crates/qingqi-core crates/qingqi-app  # ✅ 无输出

# 编译通过
cargo check --workspace  # ✅ 通过（仅有6个未使用函数warning）
```

## 文档体系现状

### 当前有效文档（按重要性排序）

1. **架构与设计**
   - `docs/workspace-split-guide.md` ⭐⭐⭐⭐⭐ 架构权威文档
   - `AGENT.md` ⭐⭐⭐⭐⭐ AI助手与开发者指南
   - `README.md` ⭐⭐⭐⭐ 项目概述与快速开始

2. **编码规范**
   - `docs/conventions.md` ⭐⭐⭐⭐⭐ 详细编码规范
   - `docs/gpui-component-guide.md` ⭐⭐⭐⭐ GPUI组件使用

3. **历史参考**
   - `docs/archive/` 已完成的拆分计划、审计报告等

### 文档权威等级

| 文档 | 适用范围 | 权威性 | 更新频率 |
|------|----------|--------|----------|
| `workspace-split-guide.md` | crate边界、依赖方向、SDK契约 | **最高** | 架构变更时 |
| `conventions.md` | 编码风格、命名、性能规则 | **最高** | 规范演进时 |
| `AGENT.md` | 日常开发指导、速查 | 高 | 随架构文档同步 |
| `README.md` | 新人入门、项目概览 | 中 | 主要特性变更时 |
| `gpui-component-guide.md` | GPUI组件使用细节 | 中 | 组件库升级时 |

## 后续建议

### 立即可开展的工作

现在文档已同步，可以安全推进以下工作：

1. **P1 - 代码质量收敛**
   - 消除 unwrap（677个clippy警告）
   - 清除硬编码颜色（20+处）
   - 修复锁unwrap（可能导致连锁panic）

2. **P2 - 架构优化**
   - 拆分超大文件（api-debugger/view.rs 4,955行）
   - 提取共享UI组件到 qingqi-ui

3. **P3 - 持续改进**
   - 补充service/store层测试
   - 平台层unsafe审计

### 开发流程

```
修改前 → 查阅文档（workspace-split-guide / conventions）
   ↓
编码 → 遵循规范（AGENT.md 速查）
   ↓
提交前 → cargo fmt && cargo check && cargo clippy
   ↓
架构变更 → 运行依赖边界验证命令
   ↓
重大变更 → 先更新文档，再改代码
```

## 验证清单

- [x] 文档清理完成（5个文档归档）
- [x] AGENT.md 更新完成（删除pre-split描述）
- [x] README.md 更新完成（反映workspace结构）
- [x] 架构不变量验证通过（I1, I2, I3）
- [x] 项目编译通过（cargo check --workspace）
- [x] 代码格式化完成（cargo fmt --all）
- [x] 文档权威等级明确

## 影响评估

### 正面影响

✅ **文档同步**：核心文档准确反映当前架构  
✅ **降低认知负担**：新人和AI助手不再被过时信息误导  
✅ **清晰的规范**：AGENT.md 提供可执行的开发指导  
✅ **可验证性**：提供具体的依赖边界验证命令  
✅ **历史可追溯**：归档文档保留演进历史  

### 风险

⚠️ **文档数量减少**：从8个减到5个（3个归档）
   - 缓解：归档文档仍可访问，核心文档质量提升
   
⚠️ **开发者需要重新学习**：AGENT.md 结构变化较大
   - 缓解：新版更清晰、有速查表和示例

## 总结

本次更新完成了 **P0 级文档同步任务**，消除了架构与文档的严重不一致问题。项目现在具备：

1. ✅ 准确的架构描述
2. ✅ 清晰的开发规范
3. ✅ 可验证的质量门禁
4. ✅ 完整的文档体系

项目已经可以健康推进后续的代码质量收敛和架构优化工作。
