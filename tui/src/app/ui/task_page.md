# Task_Page 技术规格文档

> **状态**: 待实现
> **位置**: `src/app/ui/task_page.rs`
> **依赖**: `tui-markdown`, `synapcore_core`, `synapcore_provider`
> **关联**: `TaskPageStore`数据结构, `ProviderClient`通信

## 一、页面启动流程

### 进入时机
1. 用户在`StartPage`输入内容后按`Enter`
2. 清空输入框，跳转到`TaskPage`
3. 立即创建新的`TaskPageStore::chunk`并设置`generating = true`
4. 通过`ProviderClient::send_message()`发送消息到Provider

### 页面布局 (垂直布局)
```
┌─────────────────────────────────────┐
│ 顶栏 (吸顶)                         │ 约束: Length(3)
│ - 当前任务: [截断的用户输入...]      │
├─────────────────────────────────────┤
│ 消息区域 (可滚动)                    │ 约束: Min(10)
│ - 按时间顺序显示所有chunks           │
│ - 支持向上/向下滚动                 │
│ - 生成时自动滚动到底部               │
├─────────────────────────────────────┤
│ 输入栏 (底部)                       │ 约束: Length(6)
│ - 多行输入框                        │
│ - 生成状态动画                      │
│ - 页面状态提示                      │
└─────────────────────────────────────┘
```

## 二、数据结构定义

### TaskPageStore (`src/app/task_store.rs`)

```rust
/// 任务页面数据管理
pub struct TaskPageStore {
    /// 对话块列表，每个chunk对应一次用户输入和完整的Assistant响应
    pub chunks: Vec<TaskPageChunk>,
    
    /// 当前活动chunk索引（最新生成的chunk）
    pub current_chunk_idx: usize,
    
    /// 滚动偏移量（行数）
    pub scroll_offset: usize,
    
    /// 是否自动滚动到底部
    pub auto_scrolling: bool,
    
    /// 是否正在生成响应
    pub generating: bool,
    
    /// 当前输入缓冲区
    pub input_buffer: String,
    
    /// 共享主题引用（避免clone）
    pub theme: Rc<RefCell<Theme>>,
}

/// 单个任务对话块
pub struct TaskPageChunk {
    /// 用户输入（原始文本）
    pub input: String,
    
    /// 思考过程（Reasoning chunks）
    pub reasoning: String,
    
    /// 正文内容（Content chunks）
    pub content: String,
    
    /// 工具准备信息（可选）
    pub tool_preparing: Option<ToolPreparing>,
    
    /// 工具调用列表（可能多次调用）
    pub tool_calls: Vec<ToolCallInfo>,
    
    /// 是否已保存对话
    pub saved: bool,
    
    /// 是否已存储到记忆
    pub stored: bool,
    
    /// Token使用量（可选）
    pub usage: Option<Usage>,
    
    /// 错误信息（可选）
    pub error: Option<String>,
    
    /// 时间戳（用于排序和显示）
    pub created_at: DateTime<Utc>,
}

/// 工具准备信息
pub struct ToolPreparing {
    pub character: String,
    pub name: String,
    pub prepared_at: DateTime<Utc>,
}

/// 工具调用信息
pub struct ToolCallInfo {
    pub character: String,
    pub name: String,
    pub arguments: String, // 截断后的参数显示
    pub full_arguments: String, // 完整参数（供详情显示）
    pub called_at: DateTime<Utc>,
}
```

## 三、渲染逻辑实现

### 3.1 总体渲染流程 (`task_page.rs::render_task()`)
```rust
pub fn render_task(
    frame: &mut Frame,
    area: Rect,
    store: &TaskPageStore,
    theme: &Theme,
    generating: bool,
) -> Result<()> {
    // 1. 垂直布局分割
    let chunks = Layout::vertical([
        Constraint::Length(3),  // 顶栏
        Constraint::Min(10),    // 消息区域
        Constraint::Length(6),  // 输入栏
    ]).split(area);
    
    // 2. 渲染各区域
    render_top_bar(frame, chunks[0], store.current_input(), theme)?;
    render_message_area(frame, chunks[1], store, theme)?;
    render_input_bar(frame, chunks[2], &store.input_buffer, generating, theme)?;
    
    Ok(())
}
```

### 3.2 顶栏渲染 (`render_top_bar()`)
**功能**: 
- 显示当前任务（截断的用户输入）
- 固定位置，不随消息滚动
- 根据theme显示样式

**截断逻辑**:
```rust
fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() > max_chars {
        let truncated: String = text.chars().take(max_chars - 3).collect();
        format!("{}...", truncated)
    } else {
        text.to_string()
    }
}
```

### 3.3 消息区域渲染 (`render_message_area()`)
**功能**:
- 渲染所有可见的chunks
- 处理滚动逻辑
- 计算可见区域内的chunks

**滚动逻辑**:
```rust
impl TaskPageStore {
    pub fn scroll_up(&mut self, lines: usize) {
        if self.scroll_offset + lines <= self.total_lines() {
            self.scroll_offset += lines;
        }
        self.auto_scrolling = false;
    }
    
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        // 如果滚动到底部，重新启用自动滚动
        if self.scroll_offset == 0 && self.generating {
            self.auto_scrolling = true;
        }
    }
    
    pub fn get_visible_chunks(&self, area_height: usize) -> Vec<&TaskPageChunk> {
        // 根据scroll_offset和area_height计算哪些chunks可见
        // 实现虚拟滚动，避免渲染所有chunks
    }
}
```

### 3.4 Chunk项目渲染 (`render_chunk_items()`)
```rust
fn render_chunk_items(chunk: &TaskPageChunk, theme: &Theme) -> Vec<ListItem> {
    let mut items = Vec::new();
    
    // 1. 用户输入 (深灰色block，白色加粗字体)
    items.push(ListItem::new(Text::styled(
        format!("> {}", chunk.input),
        theme.input_style(),
    )));
    
    // 2. Reasoning部分 (灰色字体)
    if !chunk.reasoning.is_empty() {
        items.push(ListItem::new(Text::styled(
            chunk.reasoning.clone(),
            theme.reasoning_style(),
        )));
    }
    
    // 3. Content部分 (Markdown渲染)
    if !chunk.content.is_empty() {
        use tui_markdown::Markdown;
        let md = Markdown::new(&chunk.content)
            .with_theme(theme.markdown_theme());
        items.push(ListItem::new(md));
    }
    
    // 4. Tool准备 (主题色字体)
    if let Some(tp) = &chunk.tool_preparing {
        items.push(ListItem::new(Text::styled(
            format!("{} preparing tool: {}", tp.character, tp.name),
            theme.tool_prepare_style(),
        )));
    }
    
    // 5. Tool调用 (主题色字体)
    for tool in &chunk.tool_calls {
        items.push(ListItem::new(Text::styled(
            format!("{} using tool: {} - {}", 
                tool.character, tool.name, tool.arguments),
            theme.tool_call_style(),
        )));
    }
    
    // 6. 错误信息 (红色加粗)
    if let Some(err) = &chunk.error {
        items.push(ListItem::new(Text::styled(
            format!("({}): {}", chunk.error_character().unwrap_or("System"), err),
            theme.error_style(),
        )));
    }
    
    // 7. 详情按钮 (如果有save/store/usage)
    if chunk.has_details() {
        items.push(ListItem::new(Text::styled(
            "[Details]",
            theme.detail_style(),
        )));
    }
    
    items
}
```

### 3.5 输入栏渲染 (`render_input_bar()`)
**实现要求**:
1. **圆角边框**: 使用`Block::bordered().border_type(BorderType::Rounded)`
2. **自动换行**: 使用`Paragraph`的`wrap`功能
3. **生成动画**: 在右下角显示旋转动画`:`, `:.`, `::`, `:.`循环
4. **提示文本**: 当输入为空时显示`"输入以发起对话/任务"`
5. **页面状态**: 在左上角显示`Task`标签

```rust
fn render_input_bar(
    frame: &mut Frame,
    area: Rect,
    input: &str,
    generating: bool,
    theme: &Theme,
) -> Result<()> {
    let block = Block::default()
        .title(" Task ")
        .title_style(theme.input_title_style())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.input_border_style());
    
    let mut text = if input.is_empty() && !generating {
        Text::styled("输入以发起对话/任务", theme.placeholder_style())
    } else {
        Text::styled(input, theme.input_text_style())
    };
    
    // 添加生成动画
    if generating {
        let animation = get_loading_animation();
        text = text.append(Text::styled(animation, theme.animation_style()));
    }
    
    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true });
    
    frame.render_widget(paragraph, area);
    Ok(())
}
```

## 四、BotResponse处理

### 4.1 处理函数 (`TaskPageStore::handle_bot_response()`)
```rust
impl TaskPageStore {
    pub fn handle_bot_response(&mut self, response: BotResponse) {
        let current_chunk = self.current_chunk_mut();
        
        match response {
            BotResponse::Reasoning { chunk } => {
                current_chunk.reasoning.push_str(&chunk);
            }
            BotResponse::Content { chunk } => {
                current_chunk.content.push_str(&chunk);
                // 触发重绘
            }
            BotResponse::ToolPreparing { charater, name } => {
                current_chunk.tool_preparing = Some(ToolPreparing {
                    character: charater,
                    name,
                    prepared_at: Utc::now(),
                });
            }
            BotResponse::ToolCall { character, name, arguments } => {
                let tool_call = ToolCallInfo {
                    character,
                    name,
                    arguments: truncate_arguments(&arguments, 120),
                    full_arguments: arguments,
                    called_at: Utc::now(),
                };
                current_chunk.tool_calls.push(tool_call);
            }
            BotResponse::Save { character } => {
                current_chunk.saved = true;
                // 可以在这里触发保存动画或通知
            }
            BotResponse::Store { character } => {
                current_chunk.stored = true;
                // 可以在这里触发存储动画或通知
            }
            BotResponse::Usage { usage } => {
                current_chunk.usage = Some(usage);
            }
            BotResponse::Error { character, error } => {
                current_chunk.error = Some(error);
                // 生成停止
                self.generating = false;
            }
        }
    }
}
```

### 4.2 详情显示
当用户点击`[Details]`按钮时，显示弹窗包含：
- 完整的工具调用参数
- 详细的token使用统计
- 保存/存储的时间戳
- 错误堆栈信息（如果有）

## 五、主题集成

### 5.1 主题样式定义 (`theme/mod.rs`)
```rust
pub struct Theme {
    // 基础颜色
    pub bg_color: Color,
    pub fg_color: Color,
    
    // 特定样式
    pub input_style: Style,
    pub reasoning_style: Style,
    pub tool_call_style: Style,
    pub error_style: Style,
    pub detail_style: Style,
    
    // Markdown主题
    pub markdown_theme: tui_markdown::Theme,
}
```

### 5.2 everyforest主题 (`theme/everyforest.rs`)
```rust
impl Theme {
    pub fn everyforest() -> Self {
        Self {
            bg_color: Color::Rgb(16, 24, 31),  // 深绿色背景
            fg_color: Color::Rgb(168, 192, 148), // 浅绿色文字
            input_style: Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .bg(Color::Rgb(40, 54, 58))
                .add_modifier(Modifier::BOLD),
            reasoning_style: Style::default()
                .fg(Color::Rgb(150, 150, 150)), // 灰色
            tool_call_style: Style::default()
                .fg(Color::Rgb(86, 156, 214)), // 蓝色
            error_style: Style::default()
                .fg(Color::Rgb(255, 85, 85))   // 红色
                .add_modifier(Modifier::BOLD),
            // ... 其他样式
        }
    }
}
```

## 六、键盘交互

### 6.1 快捷键定义
- `Esc`: 返回上一页或退出
- `Enter`: 提交输入（如果输入不为空）
- `Ctrl+Enter`: 输入换行
- `Up/Down`: 滚动消息区域
- `PageUp/PageDown`: 快速滚动
- `Ctrl+S`: 显示/隐藏详情
- `Ctrl+R`: 重新发送最后一条消息

### 6.2 滚动控制
```rust
fn handle_key_event(&mut self, key: KeyCode) {
    match key {
        KeyCode::Up => self.scroll_up(1),
        KeyCode::Down => self.scroll_down(1),
        KeyCode::PageUp => self.scroll_up(10),
        KeyCode::PageDown => self.scroll_down(10),
        KeyCode::Enter => {
            if !self.input_buffer.trim().is_empty() {
                self.submit_input();
            }
        }
        // ... 其他按键处理
    }
}
```

## 七、实现步骤清单

### 第一阶段：数据结构与基础渲染
1. [ ] 创建`task_store.rs`模块
2. [ ] 实现`TaskPageStore`和`TaskPageChunk`结构
3. [ ] 创建基本的渲染函数框架
4. [ ] 实现顶栏和输入栏渲染

### 第二阶段：消息区域与滚动
1. [ ] 实现消息区域虚拟滚动
2. [ ] 添加基本的chunk项目渲染
3. [ ] 实现自动滚动逻辑
4. [ ] 集成Markdown渲染

### 第三阶段：BotResponse处理
1. [ ] 实现`handle_bot_response`方法
2. [ ] 添加各种BotResponse类型的渲染
3. [ ] 实现工具调用显示
4. [ ] 添加错误处理显示

### 第四阶段：主题与优化
1. [ ] 创建主题模块
2. [ ] 实现everyforest和one_dark主题
3. [ ] 优化渲染性能
4. [ ] 添加动画效果

### 第五阶段：交互与测试
1. [ ] 实现完整的键盘交互
2. [ ] 添加鼠标支持（如果计划支持）
3. [ ] 编写单元测试
4. [ ] 进行集成测试

## 八、实现状态总结

### ✅ 已完成的核心功能
1. **TaskPageStore数据结构**: 已完整实现所有结构体和核心方法
2. **任务页面渲染**: 实现三栏布局、消息列表、输入栏动画
3. **滚动系统**: 支持向上/向下滚动、自动滚动控制
4. **BotResponse处理**: 完整处理所有BotResponse变体
5. **用户交互**: 键盘输入处理、事件分发机制
6. **Provider集成**: ProviderClient模块提供异步通信

### 🔄 部分完成的功能
1. **主题系统**: 基础Theme结构已定义，具体样式待完善
2. **Markdown渲染**: 框架已准备，但需要更好的集成
3. **详情显示**: 基础功能已实现，需要完善样式和交互

### 📋 下一步优化建议
1. **性能优化**: 实现消息区域虚拟滚动以支持大量消息
2. **主题完善**: 实现everyforest和one_dark完整主题配置
3. **错误处理**: 添加更多错误恢复和用户反馈机制
4. **测试覆盖**: 添加单元测试和集成测试

---
**版本**: 3.0 (实现版本)
**状态**: 核心功能已实现
**完成时间**: 2026-04-28
**代码位置**: `src/app/ui/task_page.rs` (已重构完成)
