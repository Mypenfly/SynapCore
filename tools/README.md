# Tools Crate — 工具系统

> **crate名**: `tools`  
> **定位**: 为 LLM agent 提供可调用的内部/外部工具集  
> **配置**: `~/.config/synapcore/tools/tools.toml`

---

## 架构总览

```
tools/src
├── lib.rs                     # Tools 主结构体：配置加载、工具调度、定义收集
├── define_call/               # 工具调用/定义的数据结构
│   ├── mod.rs
│   ├── tool_call.rs           # ToolCall, Function  — LLM 返回的调用请求
│   └── tool_define.rs         # ToolDefinition, FunctionDefinition, Tool trait
├── tool_response.rs           # ToolResponse 枚举 — 工具执行结果
├── error.rs                   # ToolErr — 工具错误类型
├── search_tools/              # tools_manager — 动态工具搜索/添加
├── outer/                     # 外部工具（通过 执行外部命令 调用第三方 CLI工具，本质是shell exector）
├── web_search/                # Inner 工具示例
├── files_extract/
├── files_write/
├── files_system/
├── fetch_url/
├── note_book/
├── bash/
├── executer/
└── todo_list/                 # Inner 工具示例（含 error.rs 子模块）
```

---

## 创建 Inner 工具的规范

### 1. 创建模块目录

在 `tools/src/` 下创建 `your_tool/mod.rs`，可选 `your_tool/error.rs`。

### 2. 实现 Tool trait

所有工具必须实现 `Tool` trait（定义在 `define_call/tool_define.rs`）：

```rust
use crate::{
    define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition},
    define_call::tool_call::Function,
    tool_response::ToolResponse,
};

pub struct YourTool;

impl Tool for YourTool {
    fn definition(&self) -> ToolDefinition {
        let parameters = serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "操作说明"
                }
            },
            "required": ["action"]
        });

        let function = FunctionDefinition {
            name: "your_tool".to_string(),
            description: "工具描述".to_string(),
            parameters,
        };

        ToolDefinition {
            tool_type: "function".to_string(),
            function,
        }
    }

    async fn execute(&self, function: &Function) -> ToolResponse {
        let arguments = match &function.arguments {
            Some(s) => s,
            None => return ToolResponse::Error("lack arguments".to_string()),
        };

        // 解析参数 → 执行逻辑 → 返回 ToolResponse
        ToolResponse::Error("not implemented".to_string())
    }
}
```

**关键约束**：
- `definition()` 中的 `name` 必须与 `Tools::call()` 中的 match 分支名一致
- `parameters` 用 `serde_json::json!` 构建，遵循 OpenAI function calling 格式
- `execute()` 入参是 `&Function`（含 `name` 和 `arguments`），返回 `ToolResponse`

### 3. 定义参数结构体（推荐）

```rust
#[derive(Debug, Serialize, Deserialize)]
struct Args {
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_field: Option<String>,
}
```

在 `execute()` 中解析：`let args: Args = serde_json::from_str(arguments).unwrap_or_default();`

### 4. 定义错误类型（推荐）

在 `your_tool/error.rs` 中：

```rust
#[derive(Debug, thiserror::Error)]
pub(super) enum YourToolErr {
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("序列化错误: {0}")]
    Serde(#[from] serde_json::Error),
}
```

### 5. 在 ToolResponse 中添加变体

在 `tool_response.rs` 中添加：

```rust
pub enum ToolResponse {
    // ... 已有变体
    YourTool {
        action: String,
        content: String,
    },
}
```

并在 `impl Display for ToolResponse` 的 match 中添加对应分支。

### 6. 在 lib.rs 中注册（共 4 处）

```rust
// 1. 声明模块
mod your_tool;

// 2. use 引入
use your_tool::YourTool;

// 3. Default 中添加 Inner 条目
let your_inner = Inner {
    name: "your_tool".to_string(),
    enable: true,
    params: None,
};
inner.push(your_inner);

// 4. Tools::call() 中添加 match 分支
"your_tool" => {
    let tool = YourTool;
    tool.execute(&tool.function).await
}

// 5. get_enabled_inner() 中添加解析
if list.contains(&"your_tool") {
    let tool = YourTool;
    let def = tool.definition();
    enabled_list.push(def);
}
```

### 7. 需要外部参数的工具

参考 `web_search`：在 `Inner.params` 中配置键值对，在 `Tools::call()` 中从 `self.inner` 提取传入：

```rust
"web_search" => {
    let params = self.inner.iter()
        .find(|i| i.name == "web_search")
        .and_then(|w| w.params.clone())
        .unwrap_or_default();
    let search = web_search::WebSearch { params };
    search.execute(&tool.function).await
}
```

---

## 工具调度流程

```
LLM 返回 ToolCall
    │
    ▼
Tools::call(tool_call)
    │ match tool_call.function.name
    ├── "files_extract"   → ExtractTool.execute()
    ├── "web_search"      → WebSearch.execute()
    ├── "todo_list"       → TodoList.execute()
    ├── ...               → ...
    └── _                 → OuterTools.execute()  (外部工具)
    │
    ▼
ToolResponse → Display → 作为 Role::Tool 消息注入 session
```

---

## tools.toml 配置格式

```toml
sandbox_path = "/path/to/sandbox"
sandbox_dyn = true

[[inner]]
name = "files_extract"
enable = true

[[inner]]
name = "web_search"
enable = true

[inner.params]
base_url = "https://..."
api_key = "YOUR_KEY"

[[inner]]
name = "timer"
enable = true
```

`enable = false` 的工具不会生成 `ToolDefinition`，LLM 无法调用。
