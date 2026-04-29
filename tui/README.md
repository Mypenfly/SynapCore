# SynapCore TUI - 终端用户界面

## 概述

SynapCore TUI是基于ratatui框架的终端用户界面，为SynapCore多智能体AI编排系统提供直观的终端交互体验。TUI模块与Provider深度集成，支持实时对话、工具调用状态显示、滚动浏览和历史对话管理。

## 目录

1. [架构设计](#架构设计)
2. [核心组件](#核心组件)
3. [用户交互](#用户交互)
4. [界面布局](#界面布局)
5. [主题系统](#主题系统)
6. [集成说明](#集成说明)
7. [开发指南](#开发指南)

## 正文

### 1. 架构设计

#### 概述
TUI采用异步事件驱动架构，通过通道与Provider通信，实现响应式的用户界面。

#### 引用文件
- `/home/mypenfly/projects/synapcore/tui/src/main.rs` - 主程序入口
- `/home/mypenfly/projects/synapcore/tui/src/lib.rs` - 库模块声明
- `/home/mypenfly/projects/synapcore/tui/Cargo.toml` - 依赖配置

#### 正文

**整体架构**：
```
TUI Application
├── App主循环 (tokio::select!)
│   ├── 键盘/鼠标事件监听
│   ├── Provider响应处理
│   └── 定时重绘界面 (60fps)
├── ProviderClient
│   ├── 连接Provider服务
│   ├── 发送用户消息
│   └── 接收响应流
└── TaskPageStore
    ├── 对话块管理
    ├── 滚动状态维护
    └── 输入缓冲区处理
```

**核心设计原则**：
- **异步优先**：所有I/O操作均为异步，避免阻塞UI线程
- **事件驱动**：用户输入、Provider响应都转换为事件处理
- **状态集中**：所有UI状态集中在`TaskPageStore`中管理
- **响应式UI**：状态变化自动触发界面重绘

#### 总结/建议
- 保持异步架构的一致性
- 事件处理应简洁高效，避免复杂逻辑
- 状态管理应支持撤销/重做功能（未来扩展）

### 2. 核心组件

#### 概述
详细介绍TUI模块的各个核心组件及其职责。

#### 引用文件
- `/home/mypenfly/projects/synapcore/tui/src/app/mod.rs` - 应用主结构体
- `/home/mypenfly/projects/synapcore/tui/src/app/provider_client.rs` - Provider客户端
- `/home/mypenfly/projects/synapcore/tui/src/app/task_store.rs` - 任务存储管理

#### 正文

**2.1 App主结构体**
```rust
pub struct App {
    pub state: AppState,           // 应用状态 (Running/Stopped)
    pub page: AppPage,             // 当前页面 (StartPage/TaskPage/ChatPage)
    pub task_store: TaskPageStore, // 任务页面数据存储
    pub provider_client: ProviderClient, // Provider客户端
    event_tx: Option<mpsc::Sender<AppEvent>>, // 事件发送通道
    draw_worker: DrawWorker,       // 绘制工作者
}
```

**关键方法**：
- `App::new()` - 初始化应用，连接Provider
- `App::run()` - 主事件循环，处理用户输入和Provider响应
- `App::handle_event()` - 事件分发处理
- `App::draw()` - 界面绘制

**2.2 ProviderClient**
```rust
pub struct ProviderClient {
    cmd_tx: mpsc::Sender<ProviderCommand>,   // 命令发送通道
    resp_rx: mpsc::Receiver<ProviderResponse>, // 响应接收通道
}
```

**功能**：
- `connect()` - 连接到Provider服务，启动后台任务
- `send_message()` - 发送用户消息给Provider
- `receive_response()` - 接收Provider响应
- `exit()` - 安全退出Provider

**2.3 TaskPageStore**
```rust
pub struct TaskPageStore {
    pub chunks: Vec<TaskPageChunk>,     // 对话块列表
    pub current_chunk_idx: usize,       // 当前活动chunk索引
    pub scroll_offset: usize,           // 滚动偏移量
    pub auto_scrolling: bool,           // 是否自动滚动到底部
    pub generating: bool,               // 是否正在生成响应
    pub input_buffer: String,           // 输入缓冲区
    pub cursor_position: usize,         // 光标位置
    pub theme: Rc<RefCell<Theme>>,     // 共享主题
}
```

**对话块结构**：
```rust
pub struct TaskPageChunk {
    pub input: String,                 // 用户输入
    pub reasoning: String,             // 思考过程
    pub content: String,               // 正文内容
    pub tool_preparing: Option<ToolPreparing>, // 工具准备信息
    pub tool_calls: Vec<ToolCallInfo>, // 工具调用列表
    pub saved: bool,                   // 是否已保存对话
    pub stored: bool,                  // 是否已存储到记忆
    pub has_usage: bool,               // 是否有Token使用量
    pub error: Option<String>,         // 错误信息
    pub created_at: DateTime<Utc>,     // 创建时间
    pub details_expanded: bool,        // 详情是否展开
}
```

#### 总结/建议
- App结构体应保持轻量，逻辑分散到各组件
- ProviderClient需要完善的错误处理和重连机制
- TaskPageStore应考虑性能优化，支持大量历史记录

### 3. 用户交互

#### 概述
TUI支持完整的键盘和鼠标交互，提供流畅的用户体验。

#### 引用文件
- `/home/mypenfly/projects/synapcore/tui/src/app/mod.rs` - 键盘事件处理
- `/home/mypenfly/projects/synapcore/tui/src/app/task_store.rs` - 输入处理

#### 正文

**3.1 键盘快捷键**

| 按键 | 功能 |
|------|------|
| **Esc** | 退出应用 |
| **Enter** | 提交输入 |
| **Backspace** | 删除前一个字符 |
| **Delete** | 删除后一个字符 |
| **Left/Right** | 移动光标 |
| **Home/End** | 移动到行首/行尾 |
| **Up/Down** | 滚动浏览 |
| **PageUp/PageDown** | 快速滚动 |

**3.2 鼠标交互**
- **滚轮滚动**：向上/向下滚动对话历史
- **点击展开**：点击对话块展开/收起详情

**3.3 输入处理流程**
```
用户输入 → TaskPageStore.input_buffer → Enter键提交
    ↓
AppEvent::InputSubmitted → on_input_submitted()
    ↓
task_store.add_new_chunk() → 创建新对话块
    ↓
provider_client.send_message() → 发送到Provider
    ↓
等待Provider响应 → handle_provider_response()
```

**3.4 响应处理**
TUI实时显示Provider的流式响应：
- **思考过程**：显示在对话块中
- **正文内容**：实时追加显示
- **工具调用**：显示工具名称和参数
- **状态通知**：保存/存储/错误状态

#### 总结/建议
- 快捷键应保持一致性和可发现性
- 鼠标交互应提供视觉反馈
- 输入处理需要支持多行输入（未来扩展）

### 4. 界面布局

#### 概述
TUI采用分层布局设计，清晰展示对话历史和当前状态。

#### 引用文件
- `/home/mypenfly/projects/synapcore/tui/src/app/draw.rs` - 绘制逻辑
- `/home/mypenfly/projects/synapcore/tui/src/app/ui/` - UI组件

#### 正文

**4.1 页面类型**

```rust
pub enum AppPage {
    StartPage,   // 启动页面
    TaskPage,    // 任务对话页面（主界面）
    ChatPage,    // 聊天页面（待实现）
}
```

**4.2 TaskPage布局结构**
```
┌─────────────────────────────────────────────────────┐
│ 顶部状态栏                                           │
│ [SynapCore TUI] 当前输入预览                        │
├─────────────────────────────────────────────────────┤
│                                                     │
│  对话历史区域（可滚动）                              │
│  ┌─────────────────────────────────────────────┐   │
│  │ [用户] 输入内容                             │   │
│  │ [AI] 思考过程...                           │   │
│  │ [AI] 响应内容...                           │   │
│  │ [工具] 正在调用: tool_name                 │   │
│  │ [状态] 对话已保存 ✓                         │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
├─────────────────────────────────────────────────────┤
│ 底部输入行                                          │
│ > 用户输入区 [光标]                                │
│                                                     │
│ 状态提示: 按Enter发送, Esc退出, ↑↓滚动            │
└─────────────────────────────────────────────────────┘
```

**4.3 对话块渲染**
每个对话块包含：
- **用户输入**：清晰标识用户消息
- **思考过程**：折叠显示，可展开查看
- **AI响应**：Markdown格式渲染
- **工具调用**：工具名称和参数摘要
- **状态指示器**：保存/存储/错误状态图标

**4.4 滚动机制**
- **自动滚动**：新消息到达时自动滚动到底部
- **手动滚动**：用户滚动时暂停自动滚动
- **滚动边界**：基于总行数计算的智能边界

#### 总结/建议
- 布局应适应不同终端尺寸
- 对话块渲染应考虑性能，支持虚拟滚动
- 状态指示器应直观易懂

### 5. 主题系统

#### 概述
TUI支持可配置的主题系统，提供不同的视觉风格。

#### 引用文件
- `/home/mypenfly/projects/synapcore/tui/src/app/theme/` - 主题模块
- `/home/mypenfly/projects/synapcore/tui/src/app/mod.rs` - 主题定义

#### 正文

**5.1 内置主题**

```rust
pub struct Theme {
    pub name: String,
    // 颜色定义
    pub background: Color,
    pub foreground: Color,
    pub primary: Color,
    pub secondary: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    // 样式定义
    pub border_style: BorderStyle,
    pub text_style: TextStyle,
    // Markdown渲染样式
    pub markdown_styles: MarkdownStyles,
}
```

**当前实现的主题**：
- **everyforest**：暗色森林风格（默认）
- **one_dark**：One Dark主题风格

**5.2 主题配置**
主题通过`Rc<RefCell<Theme>>`共享，支持运行时切换：
```rust
let theme = Rc::new(RefCell::new(Theme::everyforest()));
```

**5.3 Markdown渲染**
使用`tui-markdown` crate支持Markdown内容渲染：
- 标题、列表、代码块等格式
- 内联代码和链接
- 表格支持（未来扩展）

**5.4 颜色系统**
主题定义完整的颜色调色板：
- 基础色：背景、前景、边框
- 语义色：成功、错误、警告、信息
- 强调色：主要、次要、高亮

#### 总结/建议
- 主题系统应支持从配置文件加载
- 颜色应确保在不同终端上的可读性
- Markdown渲染需要性能优化

### 6. 集成说明

#### 概述
TUI如何与SynapCore系统的其他模块集成。

#### 引用文件
- `/home/mypenfly/projects/synapcore/tui/Cargo.toml` - 依赖关系
- `/home/mypenfly/projects/synapcore/tui/src/app/provider_client.rs` - Provider集成

#### 正文

**6.1 依赖关系**
```toml
[dependencies]
synapcore_provider = { path = "../provider" }  # Provider扩展层
synapcore_core = { path = "../core" }          # 核心库（间接依赖）
```

**6.2 与Provider的集成**

**连接流程**：
```rust
// 1. 创建Provider实例
let provider = Provider::new()?;

// 2. 创建通信通道
let (cmd_tx, cmd_rx) = mpsc::channel(1024);
let (resp_tx, resp_rx) = mpsc::channel(1024);

// 3. 启动Provider主循环
tokio::spawn(async move {
    provider.run(cmd_rx, resp_tx).await;
});

// 4. TUI使用通道与Provider通信
let client = ProviderClient { cmd_tx, resp_rx };
```

**消息流**：
```
TUI输入 → UserMessage::task() → ProviderCommand::Send
    ↓
Provider处理 → ProviderResponse → TUI显示
```

**6.3 错误处理集成**
- **连接错误**：显示连接失败提示
- **发送错误**：重试或显示错误信息
- **接收错误**：重新建立连接

**6.4 状态同步**
TUI实时显示Provider状态：
- 生成中状态
- 工具调用状态
- 保存/存储状态
- 错误状态

#### 总结/建议
- 集成应保持松耦合，便于测试
- 错误处理需要完善的恢复机制
- 状态同步应实时准确

### 7. 开发指南

#### 概述
如何扩展和定制TUI功能。

#### 引用文件
- `/home/mypenfly/projects/synapcore/tui/src/app/README.md` - 内部开发文档

#### 正文

**7.1 添加新页面**

1. 在`AppPage`枚举中添加新变体
2. 在`App::draw()`中添加对应的绘制逻辑
3. 在`App::handle_event()`中添加页面特定事件处理
4. 创建对应的UI组件模块

**7.2 扩展主题系统**

1. 在`Theme`结构体中添加新样式字段
2. 创建新的主题构造函数（如`Theme::dracula()`）
3. 在UI组件中使用新样式
4. 添加主题切换命令

**7.3 添加新交互功能**

**示例：添加搜索功能**
```rust
// 1. 在TaskPageStore中添加搜索状态
pub struct TaskPageStore {
    // ... 现有字段
    pub search_query: String,
    pub search_results: Vec<usize>,
    pub current_search_idx: usize,
}

// 2. 添加搜索快捷键（如Ctrl+F）
KeyCode::Char('f') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
    self.task_store.enter_search_mode();
}

// 3. 实现搜索逻辑
impl TaskPageStore {
    pub fn search(&mut self, query: &str) {
        self.search_results = self.chunks
            .iter()
            .enumerate()
            .filter(|(_, chunk)| chunk.input.contains(query) || chunk.content.contains(query))
            .map(|(idx, _)| idx)
            .collect();
    }
}
```

**7.4 性能优化建议**

1. **虚拟滚动**：只渲染可见区域的对话块
2. **缓存渲染结果**：对话块内容变化时重用渲染
3. **批量更新**：多个状态变化合并为一次重绘
4. **懒加载**：历史对话延迟加载

**7.5 测试策略**

```rust
// 单元测试：组件逻辑
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_task_store_add_chunk() {
        let theme = Rc::new(RefCell::new(Theme::everyforest()));
        let mut store = TaskPageStore::new(theme);
        let idx = store.add_new_chunk("test input");
        assert_eq!(idx, 0);
        assert_eq!(store.chunks.len(), 1);
    }
}

// 集成测试：与Provider交互
#[tokio::test]
async fn test_provider_integration() {
    // 模拟Provider响应
    // 验证TUI正确显示
}
```

**7.6 调试技巧**

1. **日志输出**：关键操作添加debug日志
2. **状态检查**：添加状态验证断言
3. **性能分析**：使用tracing进行性能监控
4. **内存检查**：定期检查内存使用情况

#### 总结/建议
- 新功能应遵循现有架构模式
- 性能优化应在必要时进行，避免过早优化
- 测试应覆盖主要交互路径
- 调试工具应便于问题排查

---

**文档版本**: 1.0  
**最后更新**: 2026-04-29  
**TUI版本**: synapcore_tui 0.1.0  
**维护者**: SynapCore开发团队  

*TUI模块是SynapCore系统的用户界面层，致力于提供最佳的终端AI交互体验。*