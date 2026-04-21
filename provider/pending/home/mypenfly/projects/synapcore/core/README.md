# SynapCore Core — 核心库技术文档

> **crate名**: `synapcore_core`  
> **版本**: 0.1.0  
> **定位**: SynapCore系统的核心库，提供LLM对话管理、记忆系统、工具调用、会话持久化等完整能力  
> **依赖**: `tools` crate（同workspace下的兄弟crate）

---

## 一、架构总览

```
synapcore_core
├── Core                    # 核心结构体，对外主入口
├── CoreEvent               # 内部事件枚举（状态机驱动）
├── BotResponse             # 对外输出枚举（调用方消费）
├── SendMode                # 发送模式枚举（Task/Chat）
├── UserMessage             # 对外输入结构体（含工厂方法）
├── config/                 # 配置系统
│   ├── CoreConfig          # 根配置
│   ├── NormalConfig        # 通用配置（路径、存储阈值等）
│   ├── AgentConfig         # Agent配置（leader/subagents/embed）
│   ├── RoleConfig          # 单个角色配置
│   └── MemConfig           # 记忆参数配置
├── assistant/              # 助手（LLM客户端封装）
│   └── Assistant           # 单个LLM会话实例
├── conversation/           # 对话缓存
│   ├── Conversation        # 单轮对话记录
│   └── TempData            # 临时数据（用户输入+文件）
├── memory/                 # 向量记忆系统
│   ├── MemoryStore         # SQLite+sqlite-vec存储
│   ├── EmbeddingClient     # Embedding API客户端
│   └── MemoryConfig        # 记忆检索参数
├── read_config/            # API配置解析
│   ├── JsonConfig          # api.json解析与查询
│   ├── Provider/Model      # 供应商与模型定义
│   └── LLMConfig           # 提取后的LLM连接配置
└── request_body/           # LLM请求层
    ├── LLMClient           # HTTP客户端+流式解析
    ├── PostBody            # 请求体构建
    ├── Session             # 会话管理（消息队列）
    ├── Messenge            # 单条消息（支持多模态）
    ├── LLMResponse         # 流式响应枚举
    ├── ToolCall/ToolDefinition  # 工具调用结构
    └── Agent               # 模型标识
```

---

## 二、Core 对外 API 详解

### 2.1 `Core::init()` — 初始化

```rust
pub fn init() -> CoreResult<Self>
```

**功能**: 从配置文件初始化Core实例，是使用Core的第一步。

**执行流程**:
1. 调用 `CoreConfig::init()` 读取 `~/.config/synapcore/synapcore.toml`，若不存在则创建默认配置
2. 读取 `api.json`（API密钥和模型配置），若不存在则创建默认模板
3. 初始化对话缓存：为leader和每个subagent创建/加载 `~/.cache/synapcore_cache/{character}.jsonl`
4. 返回完整的 `Core` 实例

**使用示例**:
```rust
let mut core = Core::init()?;
```

**错误**: 返回 `CoreErr::InitError`，可能原因：
- TOML配置文件格式错误
- api.json格式错误或无法创建

---

### 2.2 `Core::task()` — 任务派发（Leader模式）

```rust
pub async fn task(
    &mut self,
    message: &UserMessage,
) -> CoreResult<tokio::sync::mpsc::Receiver<BotResponse>>
```

**功能**: 向leader角色发送任务消息，启动完整的事件循环（含工具调用、记忆存储等）。

**参数**:
- `message: &UserMessage` — 用户消息，包含文本、文件、发送模式、是否启用工具、是否保存

**返回**: `mpsc::Receiver<BotResponse>` — 异步通道，调用方通过 `.recv().await` 逐条消费响应

**行为特点**:
- 自动使用配置中的 `leader` 角色（忽略 `message.character`）
- 支持工具调用（当 `enable_tools = true`）
- 支持记忆存储（当对话轮数达到 `store_num` 阈值时自动触发）
- 支持对话缓存和持久化

**使用示例**:
```rust
let message = UserMessage::task("帮我分析这段代码");

let mut rx = core.task(&message).await?;

while let Some(response) = rx.recv().await {
    match response {
        BotResponse::Content { chunk } => print!("{}", chunk),
        BotResponse::Reasoning { chunk } => { /* 处理思考内容 */ },
        BotResponse::ToolCall { character, name, arguments } => { /* 工具调用通知 */ },
        BotResponse::Save { character } => { /* 对话已保存 */ },
        BotResponse::Store { character } => { /* 记忆已存储 */ },
        BotResponse::Error { character, error } => eprintln!("Error: {}", error),
    }
}
```

---

### 2.3 `Core::chat()` — 一般交流（指定角色）

```rust
pub async fn chat(
    &mut self,
    character: &str,
    message: &UserMessage,
) -> CoreResult<tokio::sync::mpsc::Receiver<BotResponse>>
```

**功能**: 向指定角色发送消息，用于与subagent对话。

**参数**:
- `character: &str` — 角色名，必须在配置的 `subagents` 中定义
- `message: &UserMessage` — 同 `task()`

**返回**: 同 `task()`

**与 `task()` 的区别**:
- `task()` 固定使用leader，`chat()` 可指定任意角色
- `task()` 是"主入口"，`chat()` 是"子对话"

**使用示例**:
```rust
let message = UserMessage::chat("translator");
// 然后可以修改字段
// message.text = "翻译这段文字".to_string();

let mut rx = core.chat("translator", &message).await?;
```

---

### 2.4 `SendMode` — 发送模式枚举

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendMode {
    Task,   // 使用 leader 角色，调用 Core::task()
    Chat,   // 使用指定 subagent 角色，调用 Core::chat()
}
```

**说明**: Provider层根据此枚举决定调用 `task()` 还是 `chat()`，Core自身不使用此字段。

---

### 2.5 `UserMessage` — 用户输入结构体

```rust
#[derive(Debug, Clone)]
pub struct UserMessage {
    pub text: String,           // 用户文本消息
    pub files: Vec<String>,     // 附件文件路径列表
    pub enable_tools: bool,     // 是否启用工具调用
    pub is_save: bool,          // 是否保存对话（影响持久化和记忆存储）
    pub mode: SendMode,         // 发送模式（Task/Chat）
    pub character: String,      // 角色名（mode=Chat时使用）
}
```

**字段说明**:
| 字段 | 类型 | 说明 |
|------|------|------|
| `text` | `String` | 用户的文本输入，Core会自动附加当前时间戳 |
| `files` | `Vec<String>` | 文件路径列表，图片会base64编码，其他文件会提取文本内容 |
| `enable_tools` | `bool` | `true` 时加载工具定义并发送给LLM，LLM可发起工具调用 |
| `is_save` | `bool` | `true` 时对话会被持久化到磁盘，且达到阈值时触发记忆存储 |
| `mode` | `SendMode` | 决定Provider层调用 `task()` 还是 `chat()` |
| `character` | `String` | `mode=Chat` 时指定角色名；`mode=Task` 时可为空 |

**工厂方法**:

```rust
// 创建Task模式消息（默认启用工具、保存对话）
let msg = UserMessage::task("帮我分析代码");

// 创建Chat模式消息（默认不启用工具、保存对话）
let msg = UserMessage::chat("Yore");
```

`UserMessage::task(text)` 默认值:
- `enable_tools: true`, `is_save: true`, `mode: SendMode::Task`, `character: ""`

`UserMessage::chat(character)` 默认值:
- `text: ""`, `enable_tools: false`, `is_save: true`, `mode: SendMode::Chat`

**手动构造示例**:
```rust
let message = UserMessage {
    text: "读取这个文件".to_string(),
    files: vec!["./config.toml".to_string()],
    enable_tools: true,
    is_save: true,
    mode: SendMode::Task,
    character: String::new(),
};
```

---

### 2.6 `BotResponse` — 输出响应枚举

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum BotResponse {
    Reasoning { chunk: String },          // 思考过程（如DeepSeek的reasoning输出）
    Content { chunk: String },            // 正文内容（流式chunk）
    ToolCall { character: String, name: String, arguments: String },  // 工具调用通知
    Save { character: String },           // 对话已保存通知
    Store { character: String },          // 记忆已存储通知
    Error { character: String, error: String },  // 错误通知
}
```

**消费模式**: 调用方通过 `rx.recv().await` 循环消费，典型的响应序列为：

```
Reasoning → Reasoning → ... → Content → Content → ... → [ToolCall → Content → ...] → Save → [Store]
```

**实现 `Display`**: 可直接 `print!("{}", response)` 输出可读格式。

---

### 2.7 `CoreErr` / `CoreResult` — 错误处理

```rust
pub enum CoreErr {
    InitError(String),                              // 初始化失败
    AssistantError { model: String, error: String }, // 助手操作失败
    ToolError(ToolErr),                             // 工具调用失败
}

pub type CoreResult<T> = Result<T, CoreErr>;
```

已通过 `pub use error::{CoreErr, CoreResult};` 导出。

---

## 三、内部核心流程

### 3.1 事件驱动架构

Core内部使用 `CoreEvent` 枚举驱动状态机：

```rust
enum CoreEvent {
    Streaming { chunk },       // LLM输出文本chunk
    Reasoning { chunk },       // LLM思考过程chunk
    Completed { character, content, is_save },  // LLM完成一轮输出
    Tools { raw_content, character, tools, is_save },  // LLM请求工具调用
    Store { character, raw_content },  // 记忆存储完成
    Error { character, error },        // 错误
    Finshed,                    // 结束信号
}
```

**流程图**:
```
UserMessage
    │
    ▼
Core::task/chat
    │
    ├── get_bot() → 创建Assistant实例
    ├── bot() → 发起LLM请求，产生LLMResponse流
    │       │
    │       ▼ (tokio::spawn)
    │   LLMResponse → CoreEvent 转换
    │       │
    │       ▼ (通过event_tx发送)
    └── event_loop() → CoreEvent → BotResponse 转换
            │
            ├── Streaming → BotResponse::Content
            ├── Reasoning → BotResponse::Reasoning
            ├── Completed → 保存对话 → 可能触发记忆存储
            ├── Tools → 执行工具 → 重新调用bot()
            ├── Store → 执行记忆保存
            └── Error → BotResponse::Error
```

### 3.2 工具调用循环

当LLM返回 `tool_calls` 时：
1. 将assistant消息（含tool_call信息）加入session
2. 逐个执行工具调用：`self.tool(&mut bot, tool).await`
3. 将工具返回结果以 `Role::Tool` 消息加入session
4. 重新调用 `self.bot()` 发起下一轮LLM请求
5. 循环直到LLM不再请求工具调用

### 3.3 记忆存储流程

当对话轮数达到 `store_num`（默认50）时自动触发：
1. 创建一个新的Assistant实例（不启用工具）
2. 构造总结提示词，让LLM按 `memoryFormat.md` 格式总结对话
3. 从LLM输出中提取 `<memory>...</memory>` 标签内容
4. 对每条记忆调用 `MemoryStore::store()` 进行向量嵌入和存储
5. 压缩session：保留最近2条，用记忆摘要替代中间对话
6. 保存压缩后的session到磁盘

---

## 四、配置系统

### 4.1 配置文件位置

| 文件 | 路径 | 格式 |
|------|------|------|
| 主配置 | `~/.config/synapcore/synapcore.toml` | TOML |
| API配置 | `~/.config/synapcore/api.json` | JSON |
| 提示词 | `~/.config/synapcore/prompts/{character}.md` | Markdown |
| 记忆提示词 | `~/.config/synapcore/prompts/memory.md` | Markdown |
| 会话数据 | `~/.config/synapcore/data/{character}.json` | JSON |
| 记忆数据库 | `~/.config/synapcore/memory/{character}.db` | SQLite |
| 工具配置 | `~/.config/synapcore/tools/tools.toml` | TOML |
| 对话缓存 | `~/.cache/synapcore_cache/{character}.jsonl` | JSONL |
| 定时任务 | `~/.cache/synapcore_cache/timer.json` | JSON |

### 4.2 synapcore.toml 结构

```toml
[normal]
sc_root = "~/.config/synapcore"       # 配置根目录
api_path = "~/.config/synapcore/api.json"  # API配置路径
store_num = 50                          # 触发记忆存储的对话轮数
mem_prompt = "~/.config/synapcore/prompts/memory.md"  # 记忆提示词路径
cache_num = 50                          # 对话缓存上限

[agent]
[agent.leader]
character = "Yore"       # 角色名（需与prompts/下的文件名一致）
agent = "deepseek"       # 模型名（需与api.json中的name一致）
provider = "siliconflow" # 供应商名（需与api.json中的provider name一致）

[[agent.subagents]]
character = "coder"
agent = "gpt4o"
provider = "openai"

[agent.embed]
character = "Yore"       # 嵌入模型角色名（用于记忆系统）
agent = "qwen_embed"     # 嵌入模型名
provider = "siliconflow" # 嵌入模型供应商

[memory]
min_score = 0.05    # 记忆最低分数（低于此值将被删除）
max_score = 9.0     # 高分注入阈值
boost = 0.02        # 命中记忆的分数增长率
penalty = 0.01      # 未命中记忆的分数衰减率
high_limit = 2      # 高分记忆注入数量
top_k = 3           # 检索返回的记忆数量
```

### 4.3 api.json 结构

```json
{
  "providers": [
    {
      "name": "siliconflow",
      "base_url": "https://api.siliconflow.cn/v1",
      "api_key": "YOUR_API_KEY",
      "models": [
        { "name": "deepseek", "model_id": "deepseek-ai/DeepSeek-V3" },
        { "name": "qwen_embed", "model_id": "BAAI/bge-large-zh-v1.5" }
      ]
    },
    {
      "name": "openai",
      "base_url": "https://api.openai.com/v1",
      "api_key": "YOUR_API_KEY",
      "models": [
        { "name": "gpt4o", "model_id": "gpt-4o" }
      ]
    }
  ],
  "streaming": true,
  "params": {
    "temperature": 0.7,
    "max_tokens": 4096,
    "top_p": 0.9,
    "enable_thinking": true
  },
  "metadata": {}
}
```

**`JsonConfig::get_config(provider, model)` 方法**:
- 根据 `provider` 名找到对应的 `Provider`
- 在该 Provider 的 `models` 中根据 `model` 名找到 `model_id`
- 返回 `LLMConfig { provider, model_id, api_key }`

---

## 五、核心模块详解

### 5.1 Assistant — LLM会话实例

```rust
pub struct Assistant {
    pub llm: LLMClient,           // HTTP客户端
    pub character: String,         // 角色名
    pub store: Option<MemoryStore>, // 记忆存储（可选）
    pub path: String,              // 配置根路径
    pub is_leader: bool,           // 是否为leader
    pub stop_ok: bool,             // 停机判断标志
}
```

**关键方法**:

| 方法 | 说明 |
|------|------|
| `Assistant::new(json, description, character)` | 从配置创建Assistant实例 |
| `open_store(json, embed_description)` | 启用向量记忆存储 |
| `chat(data, mem_config)` | 发起对话，返回broadcast通道 |
| `tool(content)` | 添加工具返回结果到session |
| `note_into(note)` | 将最新笔记注入session（位置1，即系统提示词之后） |

**`chat()` 方法详细流程**:
1. 检查最后一条消息是否为Tool返回（避免重复添加用户消息）
2. 构造用户消息（附加时间戳）
3. 如有文件，调用 `messenge.add_files()` 处理（图片base64编码，其他文件提取文本）
4. 如启用记忆，调用 `messenge.add_mem()` 检索相关记忆并注入
5. 将消息加入session
6. 克隆LLMClient，在tokio::spawn中调用 `llm.send()` 发起流式请求
7. 返回 `(broadcast::Sender, broadcast::Receiver)` 对

### 5.2 LLMClient — HTTP请求与流式解析

```rust
pub struct LLMClient {
    pub client: Client,        // reqwest客户端
    pub postbody: PostBody,    // 请求体
    pub character: String,     // 角色名
}
```

**关键方法**:

| 方法 | 说明 |
|------|------|
| `new(model, provider, session, tools, params)` | 创建客户端 |
| `send(tx)` | 发送请求并通过broadcast通道流式输出 |
| `enable_mem(root, config)` | 初始化记忆存储 |
| `load_session(root)` | 从磁盘加载会话 |
| `save_session(root)` | 保存会话到磁盘 |
| `remove_content(content, tag)` | 从文本中提取XML标签内容（用于记忆提取） |

**流式解析 (`stream_out`)**:
- 解析SSE格式的流式响应
- 处理 `reasoning_content`（思考过程）和 `content`（正文）的切换
- 累积 `tool_calls` 的增量数据（id、name、arguments分多个chunk到达）
- 每20ms刷新一次缓冲区到broadcast通道
- `finish_reason: "tool_calls"` 时收集所有工具调用并发出 `LLMResponse::Tool`

**`rebuild_body()` 方法**:
构建符合OpenAI API格式的请求体JSON，包含 `model`、`messages`、`tools`、`stream`、`temperature`、`max_tokens`、`top_p`、`enable_thinking` 等字段。

### 5.3 Session — 会话管理

```rust
pub struct Session {
    pub id: String,                // 会话UUID
    pub agent: Agent,              // 模型标识
    pub provider: String,          // 供应商名
    pub messenge: VecDeque<Messenge>,  // 消息队列
}
```

**关键方法**:

| 方法 | 说明 |
|------|------|
| `new(model, provider)` | 创建新session |
| `add_messenge(messenge)` | 尾部添加消息 |
| `add_into(messenge, position)` | 指定位置替换消息（用于笔记注入） |
| `compression(from, to)` | 压缩对话，drain指定范围的消息 |
| `format_api()` | 转化为API请求格式的JSON数组 |
| `save_to_file(path)` / `load_from_file(path)` | 持久化 |

### 5.4 Messenge — 消息结构

```rust
pub struct Messenge {
    pub role: Role,                        // System/User/Assistant/Tool
    pub content: Vec<Content>,             // 内容列表（支持多模态）
    pub tool_call: Option<Vec<ToolCall>>,  // 工具调用信息
    pub tool_call_id: Option<String>,      // 工具调用ID
}

pub struct Content {
    pub content_type: String,              // "text" 或 "image_url"
    pub text: Option<String>,
    pub image_url: Option<HashMap<String, String>>,
}
```

**工厂方法**:
- `Messenge::user(txt)` — 创建用户消息
- `Messenge::assistant(txt)` — 创建助手消息
- `Messenge::system(txt)` — 创建系统消息
- `Messenge::tool(id, txt)` — 创建工具返回消息

**关键方法**:
- `add_files(files)` — 处理文件附件：图片base64编码，其他文件用 `tools::files_extract::extract` 提取文本
- `add_mem(store, config, text)` — 检索相关记忆并注入到消息内容中
- `format_api()` — 转化为API请求格式

### 5.5 MemoryStore — 向量记忆系统

```rust
pub struct MemoryStore {
    pool: Pool<SqliteConnectionManager>,  // r2d2连接池
    pub embedding_client: EmbeddingClient, // 嵌入客户端
}
```

**数据库表结构**:
- `memories` 表：id(TEXT PK), content(TEXT), score(REAL), created_time(INTEGER)
- `vec_memories` 表（sqlite-vec虚拟表）：id(TEXT PK), embedding(FLOAT[1024])

**关键方法**:

| 方法 | 说明 |
|------|------|
| `open(path, config)` | 打开/创建记忆数据库，注册sqlite-vec扩展 |
| `store(input)` | 存储一条记忆：嵌入→插入memories表→插入vec_memories表 |
| `search(query, config)` | 核心检索算法（详见下文） |

**检索算法 (`search`)**:
1. 将查询文本向量化
2. 在 `vec_memories` 中做向量近邻搜索（KNN），JOIN `memories` 获取元数据
3. 过滤 `score >= min_score` 的结果
4. 计算 `final_score = similarity × score`（向量相似度 × 历史分数）
5. **Boost**: 命中的记忆分数增长 `score *= (1 + boost)`
6. **Decay**: 未命中的记忆分数衰减 `score *= (1 - penalty)`
7. 按 `final_score` 降序排列，截取 `top_k` 条
8. **高分注入**: 额外注入 `score >= threshold` 的高分记忆（最多 `high_limit` 条）
9. **低分清除**: 删除 `score < min_score` 的记忆

### 5.6 EmbeddingClient — 嵌入API客户端

```rust
pub struct EmbeddingClient {
    base_url: String,
    api_key: String,
    model: String,
    client: Client,
}
```

**方法**: `embed(text) -> Result<Vec<f32>>`  
调用 `{base_url}/embeddings` API，返回1024维浮点向量。

### 5.7 Conversation — 对话缓存

```rust
pub struct Conversation {
    pub user: TempData,     // 用户输入（文本+文件）
    pub agent: String,      // 助手回复
}
```

**用途**: 轻量级对话记录，存储在 `~/.cache/synapcore_cache/` 下，用于快速恢复最近对话上下文。与Session的持久化不同，Conversation是辅助缓存。

**缓存策略**: 当缓存条目数超过 `cache_num` 时，删除最早的条目（保留最近20条）。

---

## 六、错误处理

### 6.1 CoreErr（已公开导出）

```rust
pub enum CoreErr {
    InitError(String),                          // 初始化失败
    AssistantError { model: String, error: String },  // 助手操作失败
    ToolError(ToolErr),                         // 工具调用失败
}

pub type CoreResult<T> = Result<T, CoreErr>;
```

### 6.2 APIErr（内部）

```rust
pub enum APIErr {
    Network(String),                              // 网络错误
    Api { code: usize, message: String },         // API返回错误
    Streaming(reqwest::Error),                    // 流式解析错误
    Json { chunk: String, e: serde_json::Error }, // JSON解析错误
    SendError(...),                               // 通道发送错误
    SessionError(String),                         // 会话加载错误
    StoreOpenError(MemoryErr),                    // 记忆存储初始化错误
    FileError(std::io::Error),                    // 文件操作错误
}
```

### 6.3 MemoryErr

```rust
pub enum MemoryErr {
    Init(r2d2::Error),           // 连接池初始化失败
    Database(rusqlite::Error),   // 数据库操作失败
    Embedding(EmbeddingErr),     // 嵌入API调用失败
    Json(serde_json::Error),     // JSON序列化失败
}
```

---

## 七、Tools Crate 依赖说明

Core通过 `tools` crate 使用以下功能：

### 7.1 直接使用的类型

| 类型 | 路径 | 用途 |
|------|------|------|
| `Tools` | `tools::Tools` | 工具管理器，初始化/调用/获取定义 |
| `ToolCall` | `tools::define_call::tool_call::ToolCall` | LLM返回的工具调用结构 |
| `ToolDefinition` | `tools::define_call::tool_define::ToolDefinition` | 工具定义（发送给LLM） |
| `Function` | `tools::define_call::tool_call::Function` | 工具调用的函数名和参数 |
| `ToolErr` | `tools::error::ToolErr` | 工具错误类型 |
| `files_extract::extract` | `tools::files_extract::extract` | 文件内容提取（用于Messenge::add_files） |

### 7.2 Tools 初始化流程

```rust
// Core::get_bot() 中
let tools = Tools::init(&self.config.normal.sc_root, character)?;
```

1. 读取 `{sc_root}/tools/tools.toml`
2. 解析内部工具列表（files_extract, files_write, web_search, files_system, fetch_url, note_book等）
3. 解析外部工具配置
4. 生成 `active_tools` 列表（ToolDefinition），发送给LLM
5. `note_book` 工具始终保留在active_tools中（让模型习惯使用）

### 7.3 工具调用流程

```
LLM返回ToolCall → CoreEvent::Tools → Core::tool(bot, tool_call)
    │
    ▼
Tools::call(tool_call) → 匹配工具名 → 执行 → ToolResponse
    │
    ▼
Assistant::tool(content) → 添加Role::Tool消息到session
    │
    ▼
Core::bot() → 重新发起LLM请求
```

---

## 八、Provider Crate 集成参考

### 8.1 Provider 当前实现

Provider crate 已实现以下模块：

```
synapcore_provider
├── Provider                # 主入口，持有Core + TimerLoop
├── timer/                  # 定时任务系统（已实现）
│   ├── Timer               # 单条定时任务
│   ├── TimerStore          # timer.json 读写
│   ├── TimerLoop           # tokio::select! 轮询循环
│   └── TimerErr            # 错误枚举
├── notify/                 # 系统通知（已实现）
│   └── SystemNotify        # notify-rust 封装
└── (待实现: exit/, send/)
```

### 8.2 Provider::send() — 统一消息发送

```rust
pub async fn send(
    &mut self,
    message: &UserMessage,
) -> CoreResult<mpsc::Receiver<BotResponse>>
```

根据 `message.mode` 自动路由：
- `SendMode::Task` → `self.core.task(message)`
- `SendMode::Chat` → `self.core.chat(&message.character, message)`

### 8.3 Provider::run() — 主循环

```rust
pub async fn run(&mut self) -> CoreResult<()>
```

在独立tokio任务中启动TimerLoop，主循环通过 `tokio::select!` 同时监听：
- Timer通知 → 转发为系统桌面通知
- 退出信号

### 8.4 TimerLoop 关键设计

- **独立Core实例**: `TimerLoop::new()` 调用 `Core::init()` 创建专属实例
- **轮询间隔**: 30秒
- **触发流程**: reload → 筛选is_due → fire(构造prompt → chat → 提取`<timer>`标签 → 截断50字) → mark_done → 发送TimerNotification
- **退出**: 通过 `watch::Receiver<bool>` 接收shutdown信号

### 8.5 关键注意事项

1. **Core不是线程安全的**: Core内部包含 `mpsc::Sender` 等类型，TimerLoop使用独立Core实例避免冲突
2. **事件循环是异步的**: `task()/chat()` 会spawn异步任务，Provider需要正确处理生命周期
3. **配置热重载**: Core的提示词支持热重载（`load_prompt`在每次创建Assistant时调用）
4. **记忆系统依赖Embedding**: 记忆功能需要配置嵌入模型（`agent.embed`），否则 `store` 为 `None`
5. **工具系统独立**: Tools crate 是独立模块，扩展工具通过 `tools.toml` 配置
6. **Timer路径**: 使用 `dirs::cache_dir().join("synapcore_cache/timer.json")`

---

## 九、依赖关系图

```
┌─────────────┐
│    tui       │ (bin crate, TUI界面)
│  (ratatui)   │
└──────┬───────┘
       │ (待接入)
       ▼
┌─────────────┐     ┌─────────────┐
│   provider   │────▶│    core     │
│  (扩展封装)  │     │ (核心库)    │
│  timer/notify│     └──────┬───────┘
└─────────────┘            │ 依赖
                           ▼
                    ┌─────────────┐
                    │    tools    │
                    │ (工具系统)  │
                    └─────────────┘

┌─────────────┐
│ src-flutter  │ (bin crate, Flutter集成, 待开发)
└─────────────┘
```

---

## 十、快速开始

### 10.1 直接使用Core

```rust
use synapcore_core::{Core, UserMessage, BotResponse, SendMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut core = Core::init()?;
    
    // 方式1: 使用工厂方法
    let message = UserMessage::task("你好");
    
    // 方式2: 手动构造
    let message = UserMessage {
        text: "你好".to_string(),
        files: vec![],
        enable_tools: false,
        is_save: false,
        mode: SendMode::Task,
        character: String::new(),
    };
    
    let mut rx = core.task(&message).await?;
    
    while let Some(response) = rx.recv().await {
        print!("{}", response);
    }
    
    Ok(())
}
```

### 10.2 使用Provider

```rust
use synapcore_provider::{Provider, SystemNotify, Timer, TimerStore};
use synapcore_core::{UserMessage, SendMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = Provider::new()?;
    
    // 发送消息
    let message = UserMessage::task("帮我分析代码");
    let mut rx = provider.send(&message).await?;
    while let Some(resp) = rx.recv().await {
        print!("{}", resp);
    }
    
    // 添加定时任务
    let mut store = TimerStore::load(&synapcore_provider::timer::default_timer_path())?;
    let timer = Timer::new(
        "2026-04-22-09:00".to_string(),
        "Yore".to_string(),
        "提醒我开会".to_string(),
    )?;
    store.add(timer)?;
    
    // 启动Provider主循环（含TimerLoop）
    provider.run().await?;
    
    Ok(())
}
```

### 10.3 带工具调用的示例

```rust
let message = UserMessage {
    text: "读取./config.toml文件的内容".to_string(),
    files: vec![],
    enable_tools: true,
    is_save: true,
    mode: SendMode::Task,
    character: String::new(),
};

let mut rx = core.task(&message).await?;

while let Some(response) = rx.recv().await {
    match response {
        BotResponse::Content { chunk } => print!("{}", chunk),
        BotResponse::ToolCall { name, arguments, .. } => {
            println!("[Tool: {}]", name);
        }
        BotResponse::Save { character } => {
            println!("[Saved by {}]", character);
        }
        _ => {}
    }
}
```

---

**文档版本**: 2.0  
**最后更新**: 2026-04-21  
**基于代码版本**: synapcore_core 0.1.0 (edition 2024)  
**变更记录**: v2.0 — 新增 SendMode/UserMessage.mode/character 字段说明、Provider集成参考、工厂方法
