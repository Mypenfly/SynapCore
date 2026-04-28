# SynapCore TUI 终端界面 - 实施计划

> **项目**: `synapcore_tui`  
> **定位**: SynapCore 系统的终端用户界面  
> **状态**: 基础框架已实现，核心功能待开发  
> **当前版本**: 0.1.0 (基础框架)
> **依赖**: `tokio`, `ratatui`, `crossterm`

**重要**: 当前代码仅为基础框架，需按照以下计划逐步实现完整功能。

## 一、当前架构与扩展计划

### 1.1 当前简单架构
```
synapcore_tui/
├── Cargo.toml                    # 项目配置
└── src/
    ├── main.rs                   # 应用入口 (简单封装)
    ├── lib.rs                    # 模块导出
    └── app/                      # 应用核心
        ├── mod.rs               # App 主结构 (约100行基础实现)
        ├── state.rs             # 状态枚举定义 (2个枚举)
        ├── draw.rs              # 绘制工作者 (简单页面分发)
        └── ui/                  # UI组件
            ├── mod.rs           # UI工具函数 (输入框渲染)
            ├── start_page.rs    # 启动页面 (Logo显示)
            ├── task_page.rs     # 任务页面 (仅基础布局框架)
            └── logo.txt         # ASCII艺术Logo
```

### 1.2 扩展目标架构
```
synapcore_tui/
├── Cargo.toml                    # 增加provider/core/tui-markdown依赖
└── src/
    ├── main.rs                   # 应用入口
    ├── lib.rs                    # 模块导出
    └── app/                      # 应用核心
        ├── mod.rs               # App 主结构重构 (支持复杂状态)
        ├── state.rs             # 状态枚举定义 (扩展)
        ├── draw.rs              # 绘制工作者重构 (集成新UI)
        ├── task_store.rs        # 新增: 任务页面数据管理
        ├── provider_client.rs   # 新增: Provider通信客户端
        ├── event.rs             # 新增: 统一事件处理系统
        ├── theme/               # 新增: 主题系统
        │   ├── mod.rs
        │   ├── everyforest.rs
        │   └── one_dark.rs
        └── ui/                  # UI组件
            ├── mod.rs           # 重构: 支持Markdown和主题
            ├── start_page.rs    # 启动页面
            ├── task_page.rs     # 重构: 完整任务页面实现
            └── components/      # 新增: 可复用组件
                ├── input_bar.rs
                ├── top_bar.rs
                └── message_list.rs
```

## 二、当前实现现状

### 2.1 当前App结构 (`app/mod.rs`)
```rust
pub struct App {
    state: AppState,               // 应用状态 (Running/Stopped)
    page: AppPage,                 // 当前页面 (StartPage/TaskPage/ChatPage)
    input: String,                 // 用户输入缓冲区
    draw_worker: DrawWorker,       // 简单绘制工作者
}

// 当前状态:
pub enum AppState {
    Running,  // 运行中
    Stopped,  // 已停止
}

pub enum AppPage {
    StartPage,  // 启动页面 (显示Logo)
    TaskPage,   // 任务页面 (仅占位符)
    ChatPage,   // 聊天页面 (未实现)
}
```

### 2.2 当前事件循环
```rust
async fn run(&mut self, terminal: &mut DefaultTerminal) -> AppResult<()> {
    let (key_tx, mut key_rx) = mpsc::channel::<KeyCode>(2);
    // 简单的键盘监听
    tokio::spawn(async move {
        loop {
            let event = tokio::task::spawn_blocking(crossterm::event::read).await;
            match event {
                Ok(Ok(Event::Key(key))) => {
                    if key_tx.send(key.code).await.is_err() { break; }
                }
                _ => break,
            }
        }
    });

    while self.state != AppState::Stopped {
        tokio::select! {
            Some(key_code) = key_rx.recv() => {
                self.handle_key(key_code).await;
            }
            _ = sleep(Duration::from_millis(16)) => {
                terminal.draw(|frame| self.draw_ui(frame))?;
            }
        }
    }
    Ok(())
}
```

**目前仅支持的基本交互**:
- `Esc`: 退出应用
- `Char(c)`: 输入字符
- `Enter`: 清空输入，跳转到 `TaskPage`
- `Backspace`: 删除最后一个字符

### 2.3 当前UI组件现状

#### 2.4.1 输入框渲染 (`app/ui/mod.rs`)
```rust
pub fn render_input(frame: &mut Frame, area: Rect, text: String)
```
实际：简单输入框，有圆角边框和标题`ask for agents`

#### 2.4.2 启动页面 (`app/ui/start_page.rs`)
```rust
pub fn render_start(frame: &mut Frame, area: Rect)
```
实际：显示`logo.txt`中的ASCII艺术Logo

#### 2.4.3 任务页面 (`app/ui/task_page.rs`)
```rust
pub fn render_task(frame: &mut Frame, area: Rect, messenge: &str, task: &str)
```
**问题**: 仅有简单95/5布局，无实际功能，与`task_page.md`设计完全不符

---

## 三、当前功能状态

### ✅ 已实现的核心功能
1. **基础框架**: 异步事件循环、状态管理、模块化架构
2. **核心数据模型**: 实现完整的`TaskPageStore`、`TaskPageChunk`数据结构
3. **Provider集成**: 创建`ProviderClient`模块，建立与`synapcore_provider`的连接
4. **TaskPage实现**: 完整的三栏布局（顶栏、消息区、输入栏）
5. **BotResponse处理**: 支持所有`BotResponse`变体的处理和显示
6. **事件系统**: 统一的`AppEvent`事件处理系统
7. **输入栏增强**: 实现生成动画、提示文本、自动换行
8. **滚动功能**: 实现消息滚动和自动滚动逻辑

### ⚠️ 待完善的次要功能
1. **主题系统**: 已创建基础`Theme`结构，需实现everyforest和one_dark完整主题
2. **Markdown渲染**: 已添加依赖，需完全集成到消息显示中
3. **错误处理**: 已创建错误类型，需完善网络错误和Provider错误的处理
4. **ChatPage**: 页面框架已定义，功能待实现
5. **配置系统**: 主题配置和快捷键配置需要外部化

## 四、实施计划 - 分阶段开发

### Phase 1: 核心数据结构与依赖集成 (1-2天)
- [ ] **依赖更新**: 添加`synapcore_provider`、`synapcore_core`、`tui-markdown`依赖
- [ ] **任务数据模型**: 实现`TaskPageStore`、`TaskPageChunk`、`ToolCallInfo`等
- [ ] **Provider客户端**: 实现`ProviderClient`模块，提供异步通信
- [ ] **基础事件系统**: 创建`AppEvent`枚举和统一事件处理器

### Phase 2: TaskPage完整实现 (2-3天)
- [ ] **TaskPage渲染重构**: 根据`src/app/ui/task_page.md`设计完全重写
- [ ] **Markdown集成**: 集成`tui-markdown`渲染BotResponse内容
- [ ] **滚动逻辑**: 实现消息滚动、自动滚动控制
- [ ] **输入栏增强**: 实现换行、提示文本、生成动画
- [ ] **BotResponse处理**: 实现所有`BotResponse`变体渲染逻辑

### Phase 3: 主题系统与UI优化 (1-2天)
- [ ] **主题模块**: 实现`theme/`目录，包含everyforest和one_dark
- [ ] **组件化**: 抽取`input_bar`、`top_bar`、`message_list`可复用组件
- [ ] **UI改进**: 改进布局、样式、交互反馈

### Phase 4: 错误处理与用户体验 (1天)
- [ ] **错误处理**: Provider连接错误、网络错误、渲染错误
- [ ] **加载状态**: Provider连接中、消息生成中状态显示
- [ ] **文档完善**: 补充代码文档和使用说明

---

## 五、核心数据模型设计

### 5.1 TaskPageStore 结构
```rust
pub struct TaskPageStore {
    pub chunks: Vec<TaskPageChunk>,       // 对话块列表
    pub current_chunk_idx: usize,         // 当前活跃块索引
    pub scroll_offset: usize,             // 滚动偏移量
    pub auto_scrolling: bool,             // 是否自动滚动
    pub generating: bool,                 // 是否正在生成
    pub input_buffer: String,             // 输入缓冲区
}

pub struct TaskPageChunk {
    pub input: String,                    // 用户输入
    pub reasoning: String,                // 思考过程
    pub content: String,                  // 回复内容
    pub tool_preparing: Option<ToolPreparing>,  // 工具准备
    pub tool_calls: Vec<ToolCallInfo>,    // 工具调用列表
    pub saved: bool,                      // 是否已保存
    pub stored: bool,                     // 是否已存储到记忆
    pub usage: Option<Usage>,             // token使用量
    pub error: Option<String>,            // 错误信息
}
```

### 5.2 Provider客户端设计
```rust
pub struct ProviderClient {
    cmd_tx: mpsc::Sender<ProviderCommand>,
    resp_rx: mpsc::Receiver<ProviderResponse>,
}

impl ProviderClient {
    pub async fn connect() -> Result<(Self, JoinHandle<()>)> {
        // 创建Provider实例并启动后台任务
    }
    
    pub async fn send_message(&mut self, text: &str) -> Result<()> {
        // 发送消息给Provider
    }
}
```

### 5.3 统一事件系统
```rust
pub enum AppEvent {
    Key(KeyCode),                         // 键盘事件
    ProviderResponse(ProviderResponse),   // Provider响应
    InputSubmitted(String),               // 用户提交输入
    Scroll(i32),                          // 滚动事件 (+上/-下)
    DetailToggle(usize),                  // 详情显示切换
    Generating(bool),                     // 生成状态变化
    Exit,                                 // 退出应用
}
```

## 六、BotResponse处理流程

### 6.1 BotResponse类型映射
根据`../core/src/lib.rs`分析，`BotResponse`包含：
```rust
Reasoning { chunk }       // 思考过程
Content { chunk }         // 正文内容流
ToolPreparing { charater, name }  // 工具准备
ToolCall { character, name, arguments }  // 工具调用
Save { character }         // 对话保存
Store { character }        // 记忆存储  
Usage { usage }           // token用量
Error { character, error } // 错误信息
```

### 6.2 处理逻辑
```rust
fn handle_provider_response(&mut self, response: ProviderResponse) {
    match response {
        ProviderResponse::Response(bot_resp) => {
            match bot_resp {
                BotResponse::Reasoning { chunk } => 
                    self.task_store.add_reasoning(chunk),
                BotResponse::Content { chunk } =>
                    self.task_store.add_content(chunk),
                BotResponse::ToolPreparing { charater, name } =>
                    self.task_store.add_tool_preparing(charater, name),
                BotResponse::ToolCall { character, name, arguments } =>
                    self.task_store.add_tool_call(character, name, arguments),
                BotResponse::Save { character } =>
                    self.task_store.mark_saved(character),
                BotResponse::Store { character } =>
                    self.task_store.mark_stored(character),
                BotResponse::Usage { usage } =>
                    self.task_store.set_usage(usage),
                BotResponse::Error { character, error } =>
                    self.task_store.set_error(character, error),
            }
        }
        ProviderResponse::Error(err) => {
            self.task_store.set_error("Provider", err);
        }
    }
}
```

## 七、更新Cargo.toml依赖

```toml
[dependencies]
tokio = { version = "1.35", features = ["full"] }
ratatui = { version = "0.30.0", features = ["serde"] }
crossterm = "0.29"

# 新增依赖
synapcore_provider = { path = "../provider" }
synapcore_core = { path = "../core" }
tui-markdown = "0.3"  # Markdown渲染
```

## 八、下一步操作

### 立即开始重构mod.rs
基于此文档立即开始重构`src/app/mod.rs`，按以下顺序：
1. 首先添加必要的use语句引入Provider、Core类型
2. 重构App结构体，添加task_store和provider_client字段
3. 扩展状态类型支持生成状态
4. 重构事件循环，集成Provider响应监听

### 参考文件
- `src/app/ui/task_page.md`: TaskPage详细设计文档
- `src/app/README.md`: UI交互逻辑和主题设计
- `../provider/README.md`: Provider接口文档
- `../core/src/lib.rs`: Core核心类型定义

---

**文档版本**: 2.0 (实施计划版本)  
**更新时间**: 2026-04-28  
**当前代码状态**: synapcore_tui 0.1.0 (基础框架)  
**实施目标**: 完全实现task_page.md设计，集成Provider