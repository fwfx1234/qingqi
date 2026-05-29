# 迁移执行手册 · M1：Manifest owned 化 + serde + IconRef

> 本文是**给执行模型照做的逐步操作手册**，对应 [architecture.md](architecture.md) §13.2 的 **M1**。
> 目标：把核心契约里的 `PluginManifest`（当前 `Copy + &'static str`）改成 **owned（`Arc<str>` + serde）**，
> 全链路去 `&'static str`，并引入 **`IconRef`** 图标类型。
> 改完后行为**零变化**，只是类型变 owned。
>
> ⚠️ 本手册要求**严格按 Step 顺序执行，逐字照抄 before/after**。不要自由发挥、不要"顺手优化"、
> 不要改本手册没列出的东西。每个 Step 的代码块就是答案。

---

## 0. 执行模型工作守则（必读，每次开工前重读）

1. **只做本手册写的改动。** 不改命名（除非本手册明确要求）、不改逻辑、不删功能、不动 UI 样式/颜色/字体。
2. **照抄 before/after。** "BEFORE" 是文件里现在的样子，"AFTER" 是要替换成的样子。找到 BEFORE，替换为 AFTER。
3. **遇到不在手册里的情况就停下来**，把卡住的文件名+行号+报错记下来，不要猜。
4. **每做完一个大 Step，运行一次编译**（见 §3 命令）。能编译过再继续下一步。**不要一口气改完才编译**——除非某 Step 明确说"本步无法单独编译，需连同下一步"。
5. **遇到编译错误，先查 §12「编译报错对照表」**，里面有标准修法。
6. 类型替换是"全有或全无"：`PluginManifest` 的字段类型一改（Step 3），所有用到它的文件都会报错，必须全部改完（Step 4–11）才能再次编译通过。所以 Step 3–11 是**一个不可分割的大批次**，中途编译报错是正常的，全改完再编译。
7. 本手册涉及的字符串**字面量内容一律原样保留**（包括中文、重复词、拼写）。你只是给它们"穿一层 `.into()`"，不改内容。

---

## 1. 本次范围（M1）

**做：**
- `PluginManifest` 的字符串字段 `&'static str` → owned（`Arc<str>` / `Vec<Arc<str>>`）。
- `PluginVisualSpec.icon: &'static str` → `IconRef`；`PluginStats` 三个字段 → `Arc<str>`。
- 新增 `core/icon.rs` 定义 `IconRef`。
- 给跨边界数据类型加 `#[derive(Serialize, Deserialize)]`。
- `PluginManager` 内部 key `&'static str` → `Arc<str>`。
- 修所有调用点 + 受影响的 3 个测试文件。

**不做（留给后续 M2–M6，见 §13）：**
- ❌ **不**把类型名 `PluginManifest` 改成 `Manifest`（改名留作后续独立机械步骤，本步保留原名以缩小改动面）。
- ❌ **不**动 `PluginSession` / `PluginRuntime` trait 的结构（视图枚举是 M2）。
- ❌ **不**碰 `Rc<RefCell>` / `XxxElement` 包装壳 / 视图渲染（M2）。
- ❌ **不**改颜色 / 字体 / token / 图标资源文件本身（那是 conventions §8/§9，独立任务）。
- ❌ **不**改 `CommandItem`→`Command` 重命名、不加 `Activation`/`Action`（后续独立步骤）。
- ❌ **不**给 `CommandMatch` 加 serde（它有 `reason: &'static str`，是 serde 障碍，本步跳过它）。

---

## 2. 关键约定（贯穿全篇）

- **owned 字符串统一用 `Arc<str>`**（不是 `String`）。这是 architecture §3.2 的要求（将来 IPC 线格式 + clone 便宜）。
- 引入类型别名 **`pub type PluginId = Arc<str>;`**，`id` 字段用它。
- 数组字段 `&'static [&'static str]` → **`Vec<Arc<str>>`**。
- `IconRef` 内部是 `Arc<str>`，**不是 Copy**。因此 `PluginVisualSpec`、`PluginStats`、`PluginManifest` 都会**失去 `Copy`**，只保留 `Clone`。这会连带几处 `.copied()` 要改成 `.cloned()`、几处按值传递要改成借用——本手册都列出来了。
- `manifest()` 方法**仍返回 owned `PluginManifest`（by value）**，签名不变（只是返回的值变 owned，clone 便宜）。

---

## 3. 编译 / 测试命令（每次验证用这一组）

```bash
cd /Users/fwfx1234/develop/qingqi
export PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH
cargo check          # 快速看能否编译
cargo clippy --all-targets
cargo test
cargo fmt            # 最后格式化
```

> 提示：Step 3–11 期间允许 `cargo check` 报错（类型还没全改完）。全部改完后必须 `cargo check` 干净。

---

## 速查表（核心！重复改动全靠它）

执行 Step 5–11 时，遇到下列模式就按本表替换。**先吃透这张表，后面很多步只是反复套用它。**

### A. 字段定义（出现在 struct 定义里）
| BEFORE | AFTER |
|---|---|
| `pub id: &'static str,` | `pub id: PluginId,` |
| `pub name: &'static str,` | `pub name: Arc<str>,` |
| `pub description: &'static str,` | `pub description: Arc<str>,` |
| `pub command_hint: &'static str,` | `pub command_hint: Arc<str>,` |
| `pub keywords: &'static [&'static str],` | `pub keywords: Vec<Arc<str>>,` |
| `pub command_prefixes: &'static [&'static str],` | `pub command_prefixes: Vec<Arc<str>>,` |
| `pub icon: &'static str,` | `pub icon: IconRef,` |
| `pub primary: &'static str,`（PluginStats） | `pub primary: Arc<str>,` |

### B. 构造字面量（出现在 `PluginManifest { ... }` / `PluginVisualSpec { ... }` / `PluginStats { ... }` 里）
| BEFORE | AFTER |
|---|---|
| `id: PLUGIN_ID,` | `id: PLUGIN_ID.into(),` |
| `id: "system-settings",`（内联，无常量） | `id: "system-settings".into(),` |
| `name: "JSON 解析",` | `name: "JSON 解析".into(),` |
| `description: "...",` | `description: "...".into(),` |
| `command_hint: "...",` | `command_hint: "...".into(),` |
| `keywords: &["json", "格式化"],` | `keywords: ["json", "格式化"].into_iter().map(Into::into).collect(),` |
| `command_prefixes: &["json", "jq"],` | `command_prefixes: ["json", "jq"].into_iter().map(Into::into).collect(),` |
| `icon: "icons/json.svg",` | `icon: IconRef::asset("icons/json.svg"),` |
| `primary: "格式化",`（PluginStats） | `primary: "格式化".into(),` |

### C. 读取字段并传给 `CommandItem::plugin_open(...)` / `plugin_action(...)`
这些函数的字符串参数是 `impl Into<String>`，数组参数是 `impl IntoIterator<Item = impl Into<String>>`。owned 后要把 `Arc<str>` 借成 `&str`：
| BEFORE | AFTER |
|---|---|
| `manifest.id`（作 `impl Into<String>` 实参） | `manifest.id.as_ref()` |
| `manifest.name` | `manifest.name.as_ref()` |
| `manifest.description` | `manifest.description.as_ref()` |
| `manifest.keywords.iter().copied()` | `manifest.keywords.iter().map(|s| s.as_ref())` |
| `manifest.command_prefixes.iter().copied()` | `manifest.command_prefixes.iter().map(|s| s.as_ref())` |
| `manifest.visual.icon` | `manifest.visual.icon.as_str()` |
| `m.id` / `m.keywords.iter().copied()` 等（变量名是 `m`） | 同理：`m.id.as_ref()` / `m.keywords.iter().map(\|s\| s.as_ref())` |

### D. 比较 / Copy
| BEFORE | AFTER |
|---|---|
| `manifest.id == "clipboard"` | `manifest.id.as_ref() == "clipboard"` |
| `manifest.id == plugin_id`（`plugin_id: String`） | `manifest.id.as_ref() == plugin_id.as_str()` |
| `manifest.id == plugin_id`（`plugin_id: &str`） | `manifest.id.as_ref() == plugin_id` |
| `.get(key).copied()`（值是 `PluginVisualSpec`） | `.get(key).cloned()` |
| `a.name.cmp(b.name)` | `a.name.cmp(&b.name)` |

### E. 测试断言
| BEFORE | AFTER |
|---|---|
| `assert_eq!(manifest.id, "json-parser");` | `assert_eq!(manifest.id.as_ref(), "json-parser");` |
| `assert_eq!(manifest.name, "JSON 解析");` | `assert_eq!(manifest.name.as_ref(), "JSON 解析");` |
| `manifest.command_prefixes.contains(&"json")` | `manifest.command_prefixes.iter().any(\|p\| p.as_ref() == "json")` |
| `assert_eq!(m.id, "about");`（变量名 `m`） | `assert_eq!(m.id.as_ref(), "about");` |
| `assert!(!m.command_prefixes.is_empty())` | 不变（`Vec::is_empty` 仍可用） |

---

## Step 0 — Cargo.toml：给 serde 加 `rc` feature（必须，否则后面 serde 编译失败）

> **为什么**：`Arc<str>` 的 `Serialize`/`Deserialize` 实现被 serde 关在 `rc` feature 后面。
> 不加这一项，Step 2/3 给含 `Arc<str>` 字段的结构体 `#[derive(Serialize, Deserialize)]` 会报
> "the trait `Serialize` is not implemented for `Arc<str>`"。

**文件**：`Cargo.toml`

**BEFORE**（约第 22 行）：
```toml
serde = { version = "1.0", features = ["derive"] }
```
**AFTER**：
```toml
serde = { version = "1.0", features = ["derive", "rc"] }
```

改完单独验证一下依赖能解析：`cargo check`（此时应仍能编译，因为还没用到）。

---

## Step 1 — 新建 `src/core/icon.rs` 并在 mod.rs 导出

**新建文件** `src/core/icon.rs`，内容**完整如下**：
```rust
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// 图标引用：当前形态是指向 `assets/` 下资源的相对路径（如 `"icons/json.svg"`）。
/// owned + serde，便于将来作为第三方插件的线格式；clone 便宜（内部 `Arc<str>`）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconRef(Arc<str>);

impl IconRef {
    /// 由资源相对路径构造，如 `IconRef::asset("icons/json.svg")`。
    pub fn asset(path: impl Into<Arc<str>>) -> Self {
        Self(path.into())
    }

    /// 取底层路径字符串，用于传给渲染层（`ui::icon_element` 收 `&str`）。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

**编辑** `src/core/mod.rs`，加一行模块声明（按字母序插在 `dict_store` 和 `job` 之间，或 `database` 之后均可）：

**BEFORE**：
```rust
pub mod database;
pub mod dict_store;
pub mod job;
```
**AFTER**：
```rust
pub mod database;
pub mod dict_store;
pub mod icon;
pub mod job;
```

---

## Step 2 — `src/core/plugin_spec.rs`：owned + serde

**文件**：`src/core/plugin_spec.rs`

### 2.1 顶部 import
**BEFORE**（第 1 行）：
```rust
use gpui::SharedString;
```
**AFTER**：
```rust
use std::sync::Arc;

use gpui::SharedString;
use serde::{Deserialize, Serialize};

use crate::core::icon::IconRef;
```

### 2.2 给所有"纯数据"枚举/结构加 serde derive
对下列每个类型，在其 `#[derive(...)]` 里**追加** `Serialize, Deserialize`（保留原有的 derive）：

- `PluginCategory`（第 3 行）：`#[derive(Clone, Copy, Debug, PartialEq, Eq)]` → `#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]`
- `PluginStatus`（第 20 行）：同上加 `Serialize, Deserialize`
- `PluginAccent`（第 37 行）：同上
- `PluginWindowMode`（第 48 行）：同上
- `WindowSize`（第 65 行）：`#[derive(Clone, Copy, Debug, PartialEq)]` → 加 `Serialize, Deserialize`
- `WindowSpec`（第 71 行）：同上加 `Serialize, Deserialize`

> 这些都是 Copy 的小枚举/数值结构，**保留 `Copy`**，只追加 serde。`WindowSpec` 的 `const fn` 构造器不动。

### 2.3 `PluginVisualSpec`：去 Copy + icon 改 IconRef + serde
**BEFORE**（第 100–108 行）：
```rust
#[derive(Clone, Copy, Debug)]
pub struct PluginVisualSpec {
    pub icon: &'static str,
    pub accent: PluginAccent,
    pub category: PluginCategory,
    pub status: PluginStatus,
    pub mode: PluginWindowMode,
    pub window: WindowSpec,
}
```
**AFTER**：
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginVisualSpec {
    pub icon: IconRef,
    pub accent: PluginAccent,
    pub category: PluginCategory,
    pub status: PluginStatus,
    pub mode: PluginWindowMode,
    pub window: WindowSpec,
}
```
（注意：删掉了 `Copy`，`icon` 变 `IconRef`。）

### 2.4 `PluginStats`：去 Copy + 字段 Arc<str> + serde
**BEFORE**（第 110–115 行）：
```rust
#[derive(Clone, Copy, Debug)]
pub struct PluginStats {
    pub primary: &'static str,
    pub secondary: &'static str,
    pub tertiary: &'static str,
}
```
**AFTER**：
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginStats {
    pub primary: Arc<str>,
    pub secondary: Arc<str>,
    pub tertiary: Arc<str>,
}
```

> `PluginOverviewSection`（第 117 行起，含 `SharedString`）**不动**，与本迁移无关。

---

## Step 3 — `src/core/plugin.rs`：核心契约 owned 化

**文件**：`src/core/plugin.rs`

### 3.1 顶部 import：加 `Arc`、`serde`、`IconRef`
在文件已有 `use` 区追加（`Arc` 若已在 `std::sync` import 里则合并）：
```rust
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::core::icon::IconRef;
```
（`IconRef` 在本文件 Step 3.4 默认 commands 里不直接用名字，但 `plugin_spec` 已 re-export 经字段使用；若 `cargo check` 提示未使用可去掉该 import。`Arc`、serde 必用。）

### 3.2 加 `PluginId` 类型别名
在 `PluginManifest` 定义**正上方**插入：
```rust
/// 插件稳定标识。owned（`Arc<str>`），可作 IPC 线格式；clone 便宜。
pub type PluginId = Arc<str>;
```

### 3.3 `PluginManifest`：字段 owned + serde（**保留类型名**）
**BEFORE**（第 133–144 行）：
```rust
#[derive(Clone, Copy, Debug)]
pub struct PluginManifest {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
    pub background: bool,
    pub visual: PluginVisualSpec,
    pub stats: PluginStats,
    pub command_hint: &'static str,
    pub command_prefixes: &'static [&'static str],
}
```
**AFTER**：
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: PluginId,
    pub name: Arc<str>,
    pub description: Arc<str>,
    pub keywords: Vec<Arc<str>>,
    pub background: bool,
    pub visual: PluginVisualSpec,
    pub stats: PluginStats,
    pub command_hint: Arc<str>,
    pub command_prefixes: Vec<Arc<str>>,
}
```
（删 `Copy`，字符串/数组 owned，加 serde。`background: bool`、`visual`、`stats` 字段名不变。）

### 3.4 默认 `commands()`（trait 默认实现，第 82–92 行附近）
**BEFORE**：
```rust
    fn commands(&self) -> Vec<CommandItem> {
        let manifest = self.manifest();
        vec![CommandItem::plugin_open(
            manifest.id,
            manifest.name,
            manifest.description,
            manifest.keywords.iter().copied(),
            manifest.command_prefixes.iter().copied(),
            manifest.visual.icon,
        )]
    }
```
**AFTER**：
```rust
    fn commands(&self) -> Vec<CommandItem> {
        let manifest = self.manifest();
        vec![CommandItem::plugin_open(
            manifest.id.as_ref(),
            manifest.name.as_ref(),
            manifest.description.as_ref(),
            manifest.keywords.iter().map(|s| s.as_ref()),
            manifest.command_prefixes.iter().map(|s| s.as_ref()),
            manifest.visual.icon.as_str(),
        )]
    }
```

### 3.5 `default_plugin_commands`（第 265 行附近）
函数体与上面 3.4 内的 `vec![...]` 完全一样的写法，按相同方式把 6 个实参套上 `.as_ref()` / `.iter().map(|s| s.as_ref())` / `.visual.icon.as_str()`。
（参数签名 `manifest: PluginManifest` 保持不变。）

### 3.6 `recommended_plugin_command`（第 276 行附近）
同 3.5：函数体里那段 `CommandItem::plugin_open(manifest.id, ... )` 的 6 个实参按速查表 C 套 `.as_ref()` 等。
参数签名 `manifest: PluginManifest` 不变。

### 3.7 `PluginManager` 容器 key：`&'static str` → `Arc<str>`
**BEFORE**（第 293–301 行附近，结构体字段）：
```rust
pub struct PluginManager {
    runtimes: HashMap<&'static str, Box<dyn PluginRuntime>>,
    runtime_order: Vec<&'static str>,
    command_cache: Vec<CommandItem>,
    ...
}
```
**AFTER**：
```rust
pub struct PluginManager {
    runtimes: HashMap<Arc<str>, Box<dyn PluginRuntime>>,
    runtime_order: Vec<Arc<str>>,
    command_cache: Vec<CommandItem>,
    ...
}
```
（其余字段不动。）

### 3.8 `register()`（第 316–333 行附近）
**BEFORE**：
```rust
        let id = manifest.id;
        if !self.runtimes.contains_key(id) {
            self.runtime_order.push(id);
        }
        self.runtimes.insert(id, runtime);
```
**AFTER**：
```rust
        let id: Arc<str> = manifest.id.clone();
        if !self.runtimes.contains_key(&id) {
            self.runtime_order.push(id.clone());
        }
        self.runtimes.insert(id, runtime);
```

### 3.9 `manifests()` 排序（第 637 行附近）
**BEFORE**：
```rust
        manifests.sort_by(|a, b| a.name.cmp(b.name));
```
**AFTER**：
```rust
        manifests.sort_by(|a, b| a.name.cmp(&b.name));
```

### 3.10 tracing 字段里读 plugin_id（多处，第 396/419/483 等）
`PluginManager` 里多处迭代 `self.runtimes.iter()` 得到 `(plugin_id, runtime)`，此时 `plugin_id: &Arc<str>`。
凡 tracing 宏里写 `plugin_id = *plugin_id` 的，**改成** `plugin_id = plugin_id.as_ref()`。
（搜索本文件所有 `plugin_id = *plugin_id`，逐个替换为 `plugin_id = plugin_id.as_ref()`。）

> `open_session(&mut self, plugin_id: &str, ...)`、`handle_command`、`close_idle(&self, plugin_id: &str)`、
> `set_shortcut(..., plugin_id: &str, ...)` 这些**参数仍是 `&str`，不用改**——因为 `HashMap<Arc<str>, _>`
> 的 `.get(&str)` / `.get_mut(&str)` 依赖 `Arc<str>: Borrow<str>`，天然成立。

---

## Step 4 —（可选）`src/core/command.rs`：加 serde derive

> M1 给"跨界数据"加 serde。command.rs 里**除 `CommandMatch` 外**的类型都可安全加。
> ⚠️ **`CommandMatch` 不要动**（它有 `reason: &'static str`，加 serde 会编译失败）。

对下列类型在 `#[derive(...)]` 追加 `Serialize, Deserialize`：
- `CommandTarget`（第 3 行）
- `CommandKind`（第 23 行）
- `CommandItem`（第 38 行）
- `ContextMatcher`（第 59 行）
- `ContextSource`（第 84 行）
- `ContextKind`（第 90 行）

顶部若无 serde import，加 `use serde::{Deserialize, Serialize};`。
**`CommandMatch`（第 53 行）保持原样不加。**

> 如果你不确定，本 Step 可整体跳过——它不影响 M1 编译通过，只是少留了一点 serde。但建议做。

---

## Step 5 — 12 个 `manifest.rs`：套速查表 A/B

这 12 个文件结构完全一样，改法完全一样：**只是给 `manifest()` 函数体里的每个字段穿 `.into()` / 包 `IconRef`**。
字面量内容**原样保留**。

### 5.1 每个 manifest.rs 的统一改法
1. 顶部 import 区，把 `PluginVisualSpec, ...` 那一行所在的 `use` 块加上 `IconRef`：
   在 `use crate::core::{ plugin::PluginManifest, plugin_spec::{...} };` 里，给 `plugin_spec::{...}` 加 `... , ` 不够——`IconRef` 在 `crate::core::icon`。所以**新增一行**：
   ```rust
   use crate::core::icon::IconRef;
   ```
2. `manifest()` 函数体内每个字段按**速查表 B** 替换。

### 5.2 完整范例：`src/features/json_parser/manifest.rs`
**BEFORE**（`manifest()` 体，第 11–34 行）：
```rust
pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "JSON 解析",
        description: "JSON 格式化、验证与 JSONPath 查询",
        keywords: &["json", "格式化", "format", "parse", "解析", "query"],
        background: false,
        visual: PluginVisualSpec {
            icon: "icons/json.svg",
            accent: PluginAccent::Green,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.82, 0.84),
        },
        stats: PluginStats {
            primary: "格式化",
            secondary: "JSONPath",
            tertiary: "serde_json",
        },
        command_hint: "双栏输入/输出与 JSONPath 查询",
        command_prefixes: &["json", "jq"],
    }
}
```
**AFTER**：
```rust
pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID.into(),
        name: "JSON 解析".into(),
        description: "JSON 格式化、验证与 JSONPath 查询".into(),
        keywords: ["json", "格式化", "format", "parse", "解析", "query"]
            .into_iter()
            .map(Into::into)
            .collect(),
        background: false,
        visual: PluginVisualSpec {
            icon: IconRef::asset("icons/json.svg"),
            accent: PluginAccent::Green,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.82, 0.84),
        },
        stats: PluginStats {
            primary: "格式化".into(),
            secondary: "JSONPath".into(),
            tertiary: "serde_json".into(),
        },
        command_hint: "双栏输入/输出与 JSONPath 查询".into(),
        command_prefixes: ["json", "jq"].into_iter().map(Into::into).collect(),
    }
}
```
注意三点：① `&["..."]` 去掉前导 `&`，改 `[...].into_iter().map(Into::into).collect()`；
② `icon: "..."` → `icon: IconRef::asset("...")`；③ 其余字符串字段尾部加 `.into()`；
④ `accent/category/status/mode/window` 这几个枚举/WindowSpec 字段**完全不动**；`background` 不动。

### 5.3 其余 11 个 manifest.rs：同样套用，icon 实参对照表
对以下每个文件，按 5.1+5.2 的方式改。各字段字面量照抄原文，**唯一需要对照的是 `icon` 的参数**（其余字符串直接尾加 `.into()`，数组统一 `.into_iter().map(Into::into).collect()`）：

| 文件 | `icon:` 改成 |
|---|---|
| `features/http_capture/manifest.rs` | `IconRef::asset("icons/capture.svg")` |
| `features/about/manifest.rs` | `IconRef::asset("icons/about.svg")` |
| `features/app_launcher/manifest.rs` | `IconRef::asset("icons/rocket.svg")` |
| `features/api_debugger/manifest.rs` | `IconRef::asset("qta/mdi6.api.png")` |
| `features/ftp_sftp_ssh_client/manifest.rs` | `IconRef::asset("qta/mdi6.folder-network-outline.png")` |
| `features/quick_launch/manifest.rs` | `IconRef::asset("qta/fa5s.bolt.png")` |
| `features/clipboard/manifest.rs` | `IconRef::asset("qta/mdi6.clipboard-text-outline.png")` |
| `features/download_manager/manifest.rs` | `IconRef::asset("qta/mdi6.download.png")` |
| `features/image_compress/manifest.rs` | `IconRef::asset("qta/mdi6.image-size-select-large.png")` |
| `features/qr_code/manifest.rs` | `IconRef::asset("qta/mdi6.qrcode.png")` |
| `features/anti_peeping/manifest.rs` | `IconRef::asset("qta/mdi6.shield-eye-outline.png")` |

> 这些字段值（name/keywords/window/accent 等）各不相同，但**改法完全一致**——照抄原文字面量，只按速查表穿衣服。不要改任何字面量内容（包括 quick_launch 里重复出现的 `"命令"`、anti_peeping 里的中文前缀 `"防窥"`/`"遮盖"`）。

---

## Step 6 — 2 个内联 manifest（无 manifest.rs）

这两个插件的 manifest 直接写在各自 `plugin.rs` 的 `fn manifest(&self)` 里。

### 6.1 `src/features/system_settings/plugin.rs`（第 55–78 行附近）
该方法体内是一个 `PluginManifest { id: "system-settings", name: "系统设置", ... }` 字面量。
按**速查表 B** 改每个字段（注意：`id` 这里是字面量 `"system-settings"` 而非常量，改成 `"system-settings".into()`）。
顶部 import 加 `use crate::core::icon::IconRef;`。`icon` 改 `IconRef::asset("qta/mdi6.cog-outline.png")`。

### 6.2 `src/features/gpui_demo/plugin.rs`（第 35–58 行附近）
同 6.1。`id: "gpui-demo"` → `id: "gpui-demo".into()`。
顶部加 `use crate::core::icon::IconRef;`。`icon` 改 `IconRef::asset("qta/mdi6.school-outline.png")`。

---

## Step 7 — A 类插件 `commands(manifest: PluginManifest)`（4 个）

这 4 个插件用 `ConfiguredPluginRuntime`，各有一个 `fn commands(manifest: PluginManifest) -> Vec<CommandItem>`，
内部调 `recommended_plugin_command(manifest, [...])`。

| 文件 | 位置 |
|---|---|
| `features/json_parser/plugin.rs` | `fn commands` 第 25 行 |
| `features/qr_code/plugin.rs` | `fn commands` 第 26 行 |
| `features/image_compress/plugin.rs` | `fn commands` 第 26 行 |
| （about 无 commands fn，跳过） | — |

**改动**：这些函数体只是 `recommended_plugin_command(manifest, [ ... ])`，`manifest` 整体按值传进去，
`recommended_plugin_command` 内部（Step 3.6 已改）会自己处理 owned 字段。**所以这 3 个 `commands` 函数体通常无需改**——
签名 `manifest: PluginManifest` 保持不变（现在是 owned，by value move 一次即可）。

> ✅ 验证点：如果 `cargo check` 对这几行没报错，就不用动。若报 "use of moved value"，说明函数体内 `manifest` 被用了两次——按报错位置把第一次用法改成借用。但这 3 个插件目前都只用一次，预期无需改。

---

## Step 8 — B 类插件 override 的 `commands()`（套速查表 C）

下列插件**自己 `impl PluginRuntime`** 且 override 了 `commands()`，函数体里有一段
`CommandItem::plugin_open(manifest.id, manifest.name, ..., manifest.keywords.iter().copied(), ..., manifest.visual.icon)`。
对每个文件，把那段的 6 个字段实参按**速查表 C**替换（`.as_ref()` / `.iter().map(|s| s.as_ref())` / `.visual.icon.as_str()`）。

| 文件 | override commands 位置 | 备注 |
|---|---|---|
| `features/http_capture/plugin.rs` | 第 46–59 行（读 50–55） | 标准一段 |
| `features/api_debugger/plugin.rs` | 第 86–102 行（读 90–95） | 标准一段 |
| `features/download_manager/plugin.rs` | 第 82–98 行（读 86–91） | 标准一段 |
| `features/clipboard/plugin.rs` | 第 42–55 行（读 46–51） | 另见 Step 9.1 |
| `features/anti_peeping/plugin.rs` | 第 110–120 行（变量名是 `m`，读 113/116/117/118） | title/subtitle 是写死字面量，**不动**；只改 `m.id`/`m.keywords...`/`m.command_prefixes...`/`m.visual.icon` |
| `features/quick_launch/plugin.rs` | 第 57–82 行 | 另见 Step 9.2 |
| `features/app_launcher/plugin.rs` | 第 88–112 行 | 另见 Step 9.3 |

> `ftp_sftp_ssh_client` **不 override commands**（用默认实现，Step 3.4 已覆盖），本步跳过它。

标准一段的 BEFORE/AFTER 示意（以 http_capture 为模板，其余同形）：
**BEFORE**：
```rust
        vec![CommandItem::plugin_open(
            manifest.id,
            manifest.name,
            manifest.description,
            manifest.keywords.iter().copied(),
            manifest.command_prefixes.iter().copied(),
            manifest.visual.icon,
        )
        .with_recommend_matchers([...])]
```
**AFTER**：
```rust
        vec![CommandItem::plugin_open(
            manifest.id.as_ref(),
            manifest.name.as_ref(),
            manifest.description.as_ref(),
            manifest.keywords.iter().map(|s| s.as_ref()),
            manifest.command_prefixes.iter().map(|s| s.as_ref()),
            manifest.visual.icon.as_str(),
        )
        .with_recommend_matchers([...])]
```
（`.with_recommend_matchers([...])` 里的内容不动。）

---

## Step 9 — 三个特殊插件的额外改动

### 9.1 clipboard：`shortcuts()` 读 `manifest.id`（第 92、97 行附近）
**BEFORE**（约）：
```rust
        let manifest = self.manifest();
        ...
            plugin_id: manifest.id.to_string(),
```
`manifest.id` 现在是 `Arc<str>`，`.to_string()` **仍然可用**（`Arc<str>` 经 Deref 到 `str` 有 `to_string`）。
→ **预期无需改**。若报错，改成 `manifest.id.as_ref().to_string()`。
其余 commands 段见 Step 8。

### 9.2 quick_launch：动态 actions 读 `manifest.id` / `manifest.visual.icon`（第 68–80 行）
该段为每个 action 构造 `CommandItem::plugin_action(manifest.id, ..., manifest.visual.icon, ...)`，
循环里多次读 `manifest.id`（第 70 行）和 `manifest.visual.icon`（第 76 行）。
- `manifest.id`：作 `impl Into<String>` 实参 → 改 `manifest.id.as_ref()`。
- `manifest.visual.icon`：→ 改 `manifest.visual.icon.as_str()`。
- 因为在循环里多次借用同一个 `manifest`，**用 `.as_ref()`/`.as_str()`（借用）正好不会 move，循环安全**。
- 第 57–65 行那段开头的 `plugin_open(...)` 同 Step 8 标准替换。

### 9.3 app_launcher：`app_command(manifest: &PluginManifest, ...)`（第 186 行）+ commands/commands_for_query
- `fn app_command(manifest: &crate::core::plugin::PluginManifest, app: AppEntry)`：**参数已是借用 `&PluginManifest`**，签名不动。
  函数体内第 194 行读 `manifest.id`（作 `impl Into<String>`）→ 改 `manifest.id.as_ref()`。
- `commands()`（第 88–112 行）末尾的 `plugin_open(...)`（读 103–110）按 Step 8 标准替换。
- `commands_for_query()`（第 114–137 行）内 `app_command(&manifest, app)` 不变（已传引用）。

---

## Step 10 — `src/features/stub_plugin.rs`（无调用方，但要能编译）

> 这是个**模板文件，全仓没有任何地方实例化它**，但仍参与编译。它重度依赖 `Copy`（把 manifest 复制进 4 个结构体），
> 且 `plugin_id()/title()` 直接 `return self.manifest.id/name`（owned 后返回 `&'static str` 会失败）。
> 因为没有调用方，**可以放心改 `new` 的签名**。

### 10.1 给两个 session/page 结构补 `&'static str` 的 id/title
`StubPluginSession`（第 54 行附近）和 `StubPluginPage`（第 79 行附近）目前各存 `manifest: PluginManifest`。
`plugin_id()`（第 61 行）`title()`（第 64 行）原本返回 `self.manifest.id` / `self.manifest.name`。

**最小改法**：给 `StubPluginRuntime`/`StubPluginSession`/`StubPluginPage` 各加两个字段
`id: &'static str` 和 `title: &'static str`，由 `new` 接收并透传；`plugin_id()/title()` 改为返回这两个字段。

- `StubPluginRuntime`（第 11–15 行）结构体加：
  ```rust
      id: &'static str,
      title: &'static str,
  ```
- `StubPluginRuntime::new`（第 17–29 行）签名改为：
  ```rust
  pub fn new(
      id: &'static str,
      title: &'static str,
      manifest: PluginManifest,
      hero: &'static str,
      sections: &'static [(&'static str, &'static str)],
  ) -> Self {
      Self { id, title, manifest, hero, sections }
  }
  ```
- `manifest(&self)`（第 32 行）：`self.manifest` 现在不是 Copy，改成 `self.manifest.clone()`：
  ```rust
  fn manifest(&self) -> PluginManifest {
      self.manifest.clone()
  }
  ```
- `open_session`（第 42 行附近）里 `manifest: self.manifest` → `manifest: self.manifest.clone()`，并把 `id`/`title` 透传进 `StubPluginSession`。
- `StubPluginSession::plugin_id()`（第 61 行）→ `self.id`；`title()`（第 64 行）→ `self.title`。
- `render`（第 70 行）里 `manifest: self.manifest` → `manifest: self.manifest.clone()`，并透传 `id`/`title` 给 `StubPluginPage`。
- `StubPluginPage` 渲染体里读 `self.manifest.name`（第 129/194 行）、`.description`（130）、`.command_hint`（194）、`.visual`（94）、`.stats`（95）：
  这些是传给 GPUI 元素的文本/数据。`Arc<str>` 经 Deref 多数能直接用；若某处报类型不符，按报错加 `.as_ref()` 或 `.to_string()`。`let visual = self.manifest.visual;`（94）改 `let visual = self.manifest.visual.clone();`。

> 若改 stub 太繁琐且你不确定，**可临时给整个 stub_plugin.rs 顶部加 `#![allow(dead_code)]` 不行——它仍需编译通过**。
> 必须改到编译通过。stub 无测试、无调用方，改错也不影响运行，但必须能 `cargo check`。

---

## Step 11 — app 层：`launcher.rs` 与 `window_controller.rs`

### 11.1 `src/app/launcher.rs`
**(a) 第 123–126 行** 构建 `plugin_visuals`：
**BEFORE**：
```rust
        .map(|manifest| (manifest.id.to_string(), manifest.visual))
```
`manifests()` 返回 owned `Vec<PluginManifest>`，`into_iter()` 后可 move 出 `manifest.visual`（`PluginVisualSpec` 现在是 Clone-only，但从 owned 值里 move 字段没问题）。`manifest.id.to_string()` 对 `Arc<str>` 仍可用。
→ **预期无需改**。若报 "cannot move out"（因为同时用了 id 和 visual），改成：
```rust
        .map(|manifest| (manifest.id.to_string(), manifest.visual.clone()))
```

**(b) 第 619 行** `.copied()`：
**BEFORE**：
```rust
        let visual = self.plugin_visuals.get(plugin_id).copied();
```
**AFTER**：
```rust
        let visual = self.plugin_visuals.get(plugin_id).cloned();
```
（`PluginVisualSpec` 失去 Copy，`.copied()` → `.cloned()`。）

**(c)** 第 75/285/296 行的类型注解 `HashMap<String, PluginVisualSpec>` **不用改**（类型名没变）。
第 299–303、621–624 行只读 `visual.status`/`visual.mode`（Copy 枚举），**不用改**。
第 1381/1399/1670/1690 行用的是 `CommandItem.icon`（已是 `String`），**不用改**。

### 11.2 `src/app/window_controller.rs`（本步最容易出错，仔细看）
这里原来靠 `PluginManifest: Copy` 把同一个 `Option<PluginManifest>` 按值传了 3 次。owned 后必须改成借用。

**(a) 三个 helper 改成接收引用。**

- `plugin_reopens_in_active_space`（第 440–442 行）：
  **BEFORE**：
  ```rust
  fn plugin_reopens_in_active_space(manifest: Option<crate::core::plugin::PluginManifest>) -> bool {
      manifest.is_some_and(|manifest| manifest.visual.window.always_on_top)
  }
  ```
  **AFTER**：
  ```rust
  fn plugin_reopens_in_active_space(manifest: Option<&crate::core::plugin::PluginManifest>) -> bool {
      manifest.is_some_and(|manifest| manifest.visual.window.always_on_top)
  }
  ```

- `plugin_window_options`（第 489–530 行，签名第 491 行）：把参数
  `manifest: Option<crate::core::plugin::PluginManifest>` 改成 `Option<&crate::core::plugin::PluginManifest>`。
  函数体内第 495 行 `let Some(manifest) = manifest else {...}` 不变（现在 `manifest: &PluginManifest`）。
  第 508 行 `manifest.id == "clipboard"` → `manifest.id.as_ref() == "clipboard"`。
  第 507 行读 `manifest.visual.window.always_on_top`（bool）不变。

- `plugin_bounds`（第 532–558 行，签名第 533 行）：参数改 `Option<&crate::core::plugin::PluginManifest>`。
  第 539 行 `let Some(manifest) = manifest else {...}` 不变。第 542 行 `match manifest.visual.window.size {...}` 不变
  （`WindowSize` 是 Copy，从 `&` 读 Copy 字段 OK）。

**(b) 调用点（第 198–239 行）改成传引用。**
**BEFORE**（约）：
```rust
        let manifest = plugin_manager
            .borrow()
            .manifests()
            .into_iter()
            .find(|manifest| manifest.id == plugin_id);
        ...
        if plugin_reopens_in_active_space(manifest) {            // 204
        ...
        let (display, bounds) = plugin_bounds(manifest, cx);     // 238
        let options = plugin_window_options(title, manifest, display, bounds);  // 239
```
**AFTER**：
```rust
        let manifest = plugin_manager
            .borrow()
            .manifests()
            .into_iter()
            .find(|manifest| manifest.id.as_ref() == plugin_id);   // 见下注
        ...
        if plugin_reopens_in_active_space(manifest.as_ref()) {                    // 204
        ...
        let (display, bounds) = plugin_bounds(manifest.as_ref(), cx);            // 238
        let options = plugin_window_options(title, manifest.as_ref(), display, bounds);  // 239
```
- `manifest` 是 `Option<PluginManifest>`（owned），`manifest.as_ref()` 得到 `Option<&PluginManifest>`，正好喂给改成借用的 helper。
- find 闭包里 `manifest.id == plugin_id`：`plugin_id` 类型决定写法——若 `plugin_id: String` 用 `manifest.id.as_ref() == plugin_id.as_str()`；若 `&str` 用 `manifest.id.as_ref() == plugin_id`。按编译器提示二选一（见速查表 D）。

> 其余 window_controller.rs 与 `PluginSession`/`open_session` 相关的行（19/223/562/567）**不读 manifest 字段，不用改**。

---

## Step 12 — 修测试（3 个文件）

只有这 3 个 manifest.rs 有测试，且断言用了 `manifest.id == "..."` 等。按**速查表 E**改。

### 12.1 `src/features/json_parser/manifest.rs`（tests 模块，变量名 `manifest`）
- `assert_eq!(manifest.id, "json-parser");` → `assert_eq!(manifest.id.as_ref(), "json-parser");`
- `assert_eq!(manifest.name, "JSON 解析");` → `... manifest.name.as_ref() ...`
- `manifest.command_prefixes.contains(&"json")` → `manifest.command_prefixes.iter().any(|p| p.as_ref() == "json")`
- `manifest.command_prefixes.contains(&"jq")` → 同上换 `"jq"`
- `manifest.visual.accent` / `manifest.visual.category` 断言（与枚举比较）**不变**。
- `PLUGIN_ID == "json-parser"` 断言（`PLUGIN_ID` 仍是 `&str` 常量）**不变**。

### 12.2 `src/features/http_capture/manifest.rs`（变量名 `manifest`）
- `manifest.id` / `manifest.name` 断言加 `.as_ref()`。
- `manifest.command_prefixes.contains(&"cap"/"capture"/"httpcap")` → `.iter().any(|p| p.as_ref() == "...")`。
- `assert!(manifest.background)`、`manifest.visual.accent` 断言**不变**。

### 12.3 `src/features/about/manifest.rs`（**变量名是 `m`，不是 `manifest`**）
- `assert_eq!(m.id, "about");` → `assert_eq!(m.id.as_ref(), "about");`
- `assert_eq!(m.name, "关于");` → `... m.name.as_ref() ...`
- `m.command_prefixes.contains(&"about")` → `m.command_prefixes.iter().any(|p| p.as_ref() == "about")`
- `assert!(!m.command_prefixes.is_empty())` **不变**。
- `m.visual.accent` / `m.visual.category` 断言**不变**。
- `PLUGIN_ID == "about"` **不变**。

> 其他插件的 plugin.rs 测试（clipboard/quick_launch/app_launcher）调的是 `runtime.commands()` 等，
> 不直接断言 manifest 字段；若 `cargo test` 在那里报错，按报错位置套速查表处理（通常无需改）。

---

## Step 13 — 全量验证

```bash
cd /Users/fwfx1234/develop/qingqi
export PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH
cargo check
cargo clippy --all-targets
cargo test
cargo fmt
```
全部通过 = M1 完成。

### 自检 grep（应全部为空 / 符合预期）
```bash
# 1) 核心 manifest 字段不应再有 &'static str（应为 0）
grep -n "icon: &'static str" src/core/plugin_spec.rs        # 期望: 无输出
grep -n "id: &'static str"   src/core/plugin.rs             # 期望: 无输出

# 2) 不应再有 .visual.icon 直接当 &str 用而漏改（人工核对，应都带 .as_str()）
grep -rn "\.visual\.icon" src                               # 每条后面应是 .as_str() 或在 IconRef 上下文

# 3) PluginManager key 已 owned
grep -n "HashMap<&'static str" src/core/plugin.rs           # 期望: 无输出

# 4) serde rc 已开
grep -n 'features = \["derive", "rc"\]' Cargo.toml          # 期望: 命中
```

### 行为回归（可选手测）
启动应用、唤起启动器、搜索并打开任意插件、关闭窗口——应与改动前完全一致（M1 是纯类型改动）。

---

## 12. 编译报错对照表（卡住先查这里）

| 报错信息（关键片段） | 原因 | 修法 |
|---|---|---|
| `the trait bound \`Arc<str>: Serialize\` is not satisfied` | 没开 serde 的 `rc` feature | 做 Step 0（Cargo.toml 加 `"rc"`） |
| `cannot find type \`IconRef\`` | 忘了 import | 文件顶部加 `use crate::core::icon::IconRef;` |
| `mismatched types: expected \`IconRef\`, found \`&str\`` | manifest 构造处 icon 没包 | `icon: "x"` → `icon: IconRef::asset("x")` |
| `the trait \`From<&[&str; N]>\`...` / `expected \`Vec<Arc<str>>\`` | 数组字段没转 | `&["a","b"]` → `["a","b"].into_iter().map(Into::into).collect()` |
| `expected \`Arc<str>\`, found \`&str\`` | 文本字段没加 `.into()` | 字面量尾部加 `.into()` |
| `\`PluginManifest\` does not implement \`Copy\`` / `use of moved value: \`manifest\`` | owned 后不能多次按值用 | 改成借用：`&manifest` / `manifest.as_ref()`（Option）/ `.clone()` |
| `method \`copied\` not found` / `expected ... found &PluginVisualSpec` | 值失去 Copy | `.copied()` → `.cloned()` |
| `can't compare \`Arc<str>\` with \`&str\`` / `with \`String\`` | == 两边类型不符 | 左边 `.as_ref()`，右边 String 用 `.as_str()`（见速查表 D） |
| `expected \`impl Into<String>\`... found \`Arc<str>\`` | 传给 plugin_open 没借 | `manifest.id` → `manifest.id.as_ref()` |
| `expected \`impl Into<String>\`... in iterator` | 数组 `.copied()` 没改 | `.iter().copied()` → `.iter().map(\|s\| s.as_ref())` |
| `cannot return value referencing ... \`self.manifest\`` | session 从 owned manifest 返回 &'static | 见 Step 10（stub：改存 `&'static str` 字段） |
| tracing 宏 `\`Arc<str>\` cannot be ...` 或 `*plugin_id` 类型错 | 迭代得到 `&Arc<str>` | `plugin_id = *plugin_id` → `plugin_id = plugin_id.as_ref()` |

---

## 13. 后续路线（M2–M6，**非本次执行**，仅供了解全局）

本手册只覆盖 **M1**。完成并验证 M1 后，按 architecture.md §13.2 继续，每个 M 单独写执行手册：

- **M2** 视图枚举：引入 `PluginView` + `InlineView`/`ListView`/`WindowView` 窄 trait，拆掉宽 `PluginSession`；
  顺带消灭 `Rc<RefCell<XxxPanel>>` + `XxxElement` 包装壳，视图统一 `Entity<T>`（conventions §3.2/§6/§10）。
- **M3** 注册表 + DI：`FeatureRegistry`/`PluginDescriptor`/`BuildCx`，先注册 DB 再构造，去 clipboard 特例。
- **M4** App 内建：`AppCatalog`（提升 `app_launcher` 的索引服务），删 `features/app_launcher` 外壳。
- **M5** push 失效替轮询：`notify_commands_changed` + `dynamic_commands`，抽 `RevisionedService<T>`。
- **M6** 留口：`ViewModel` 占位、`PluginSource::External`、隔离边界收敛到一处。

期间穿插的 conventions 规约（§8 token / §9 图标 SVG 化 / §11 错误日志 / 命名 `Panel`→`View`）在各自 M 步顺带落地，
或在 M1–M6 主线跑通后作为独立清理任务。

> **写后续手册的方法**：完成 M1 后，对目标 M 步重复本手册的套路——先用只读盘点把"现状代码 + 全部调用点"摸清，
> 再产出"速查表 + 逐文件 before/after + 编译报错对照表"。低级模型只认精确的 before/after，不要给开放式指令。
