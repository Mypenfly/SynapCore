use chrono::{DateTime, Utc};
use std::{cell::RefCell, rc::Rc};
use synapcore_core::BotResponse;

use crate::app::Theme;

/// 任务存储错误类型
#[derive(Debug)]
pub enum TaskPageStoreError {
    InvalidChunkIndex,
    EmptyInput,
    ThemeError(String),
}

impl std::fmt::Display for TaskPageStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidChunkIndex => write!(f, "无效的chunk索引"),
            Self::EmptyInput => write!(f, "输入为空"),
            Self::ThemeError(msg) => write!(f, "主题错误: {}", msg),
        }
    }
}

impl std::error::Error for TaskPageStoreError {}

/// 任务页面数据管理
#[derive(Debug)]
pub struct TaskPageStore {
    /// 对话块列表
    pub chunks: Vec<TaskPageChunk>,

    /// 当前活动chunk索引
    pub current_chunk_idx: usize,

    /// 滚动偏移量（行数）
    pub scroll_offset: usize,

    /// 是否自动滚动到底部
    pub auto_scrolling: bool,

    /// 是否正在生成响应
    pub generating: bool,

    /// 当前输入缓冲区
    pub input_buffer: String,

    /// 光标位置（字符索引）
    pub cursor_position: usize,

    /// 共享主题引用
    pub theme: Rc<RefCell<Theme>>,
}

/// 单个对话块
#[derive(Debug)]
pub struct TaskPageChunk {
    /// 用户输入
    pub input: String,

    /// 思考过程
    pub reasoning: String,

    /// 正文内容
    pub content: String,

    /// 工具准备信息
    pub tool_preparing: Option<ToolPreparing>,

    /// 工具调用列表
    pub tool_calls: Vec<ToolCallInfo>,

    /// 是否已保存对话
    pub saved: bool,

    /// 是否已存储到记忆
    pub stored: bool,

    /// Token使用量（存储在BotResponse::Usage中，这里只标记有）
    pub has_usage: bool,

    /// 错误信息
    pub error: Option<String>,

    /// 创建时间
    pub created_at: DateTime<Utc>,

    /// 详情是否展开
    pub details_expanded: bool,
}

/// 工具准备信息
#[derive(Debug)]
pub struct ToolPreparing {
    pub character: String,
    pub name: String,
    pub prepared_at: DateTime<Utc>,
}

/// 工具调用信息
#[derive(Debug)]
pub struct ToolCallInfo {
    pub character: String,
    pub name: String,
    pub arguments: String,
    pub full_arguments: String,
    pub called_at: DateTime<Utc>,
}

impl TaskPageStore {
    /// 创建新的TaskPageStore
    pub fn new(theme: Rc<RefCell<Theme>>) -> Self {
        TaskPageStore {
            chunks: Vec::new(),
            current_chunk_idx: 0,
            scroll_offset: 0,
            auto_scrolling: true,
            generating: false,
            input_buffer: String::new(),
            cursor_position: 0,
            theme,
        }
    }

    /// 添加新的对话块
    pub fn add_new_chunk(&mut self, input: &str) -> usize {
        let chunk = TaskPageChunk {
            input: input.to_string(),
            reasoning: String::new(),
            content: String::new(),
            tool_preparing: None,
            tool_calls: Vec::new(),
            saved: false,
            stored: false,
            has_usage: false,
            error: None,
            created_at: Utc::now(),
            details_expanded: false,
        };

        self.chunks.push(chunk);
        self.current_chunk_idx = self.chunks.len() - 1;
        self.auto_scrolling = true;
        self.generating = true;
        self.scroll_offset = 0;
        self.current_chunk_idx
    }

    /// 获取当前活动的chunk（可变引用）
    pub fn current_chunk_mut(&mut self) -> Option<&mut TaskPageChunk> {
        self.chunks.get_mut(self.current_chunk_idx)
    }

    /// 获取当前活动的chunk（不可变引用）
    pub fn current_chunk(&self) -> Option<&TaskPageChunk> {
        self.chunks.get(self.current_chunk_idx)
    }

    /// 处理BotResponse
    pub fn handle_bot_response(&mut self, response: BotResponse) {
        if let Some(chunk) = self.current_chunk_mut() {
            match response {
                BotResponse::Reasoning {
                    chunk: reasoning_chunk,
                } => {
                    chunk.reasoning.push_str(&reasoning_chunk);
                }
                BotResponse::Content {
                    chunk: content_chunk,
                } => {
                    chunk.content.push_str(&content_chunk);
                }
                BotResponse::ToolPreparing { charater, name } => {
                    chunk.tool_preparing = Some(ToolPreparing {
                        character: charater,
                        name,
                        prepared_at: Utc::now(),
                    });
                }
                BotResponse::ToolCall {
                    character,
                    name,
                    arguments,
                } => {
                    let truncated_args = Self::truncate_text(&arguments, 120);
                    let tool_call = ToolCallInfo {
                        character,
                        name,
                        arguments: truncated_args,
                        full_arguments: arguments,
                        called_at: Utc::now(),
                    };
                    chunk.tool_calls.push(tool_call);
                }
                BotResponse::Save { character: _ } => {
                    chunk.saved = true;
                    self.generating = false;
                }
                BotResponse::Store { character: _ } => {
                    chunk.stored = true;
                    self.generating = false;
                }
                BotResponse::Usage { usage: _ } => {
                    chunk.has_usage = true;
                    self.generating = false;
                }
                BotResponse::Error { character, error } => {
                    chunk.error = Some(format!("({}): {}", character, error));
                    self.generating = false;
                }
            }
        }
    }

    /// 设置错误信息
    pub fn set_error(&mut self, character: &str, error: &str) {
        if let Some(chunk) = self.current_chunk_mut() {
            chunk.error = Some(format!("({}): {}", character, error));
        }
        self.generating = false;
    }

    /// 滚动向上
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
        self.auto_scrolling = false;
    }

    /// 滚动向下
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        // 如果滚动到底部且正在生成，重新启用自动滚动
        if self.scroll_offset == 0 && self.generating {
            self.auto_scrolling = true;
        }
    }

    /// 滚动到顶部
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
        self.auto_scrolling = false;
    }

    /// 滚动到底部
    pub fn scroll_to_bottom(&mut self) {
        // TODO: 实现基于总行数计算到底部的offset
        self.scroll_offset = 0;
        self.auto_scrolling = self.generating;
    }

    /// 获取当前输入（截断后用于顶栏显示）
    pub fn current_input(&self) -> String {
        if let Some(chunk) = self.current_chunk() {
            Self::truncate_text(&chunk.input, 20)
        } else {
            String::new()
        }
    }

    /// 检查是否有详情可显示
    pub fn has_details(&self) -> bool {
        self.chunks.iter().any(|chunk| chunk.has_details())
    }

    /// 切换详情显示
    pub fn toggle_details(&mut self, chunk_index: usize) {
        if let Some(chunk) = self.chunks.get_mut(chunk_index) {
            chunk.details_expanded = !chunk.details_expanded;
        }
    }

    /// 截断文本（中英文混合处理）
    fn truncate_text(text: &str, max_chars: usize) -> String {
        let char_count = text.chars().count();
        if char_count > max_chars {
            let truncated: String = text.chars().take(max_chars - 3).collect();
            format!("{}...", truncated)
        } else {
            text.to_string()
        }
    }

    /// 移动光标到左边
    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    /// 移动光标到右边
    pub fn move_cursor_right(&mut self) {
        let len = self.input_buffer.chars().count();
        if self.cursor_position < len {
            self.cursor_position += 1;
        }
    }

    /// 移动光标到行首
    pub fn move_cursor_home(&mut self) {
        self.cursor_position = 0;
    }

    /// 移动光标到行尾
    pub fn move_cursor_end(&mut self) {
        self.cursor_position = self.input_buffer.chars().count();
    }

    /// 在光标位置插入字符
    pub fn insert_char(&mut self, c: char) {
        let pos = self
            .input_buffer
            .char_indices()
            .nth(self.cursor_position)
            .map(|(i, _)| i)
            .unwrap_or(self.input_buffer.len());
        self.input_buffer.insert(pos, c);
        self.cursor_position += 1;
    }

    /// 在光标位置删除字符（Backspace）
    pub fn delete_char_backward(&mut self) {
        if self.cursor_position > 0 {
            let pos = self
                .input_buffer
                .char_indices()
                .nth(self.cursor_position - 1)
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input_buffer.remove(pos);
            self.cursor_position -= 1;
        }
    }

    /// 在光标位置删除字符（Delete）
    pub fn delete_char_forward(&mut self) {
        let len = self.input_buffer.chars().count();
        if self.cursor_position < len {
            let pos = self
                .input_buffer
                .char_indices()
                .nth(self.cursor_position)
                .map(|(i, _)| i)
                .unwrap_or(self.input_buffer.len());
            self.input_buffer.remove(pos);
        }
    }

    /// 计算所有chunks的总行数（用于滚动边界计算）
    pub fn total_chunk_lines(&self) -> usize {
        self.chunks
            .iter()
            .map(|chunk| {
                let mut lines = 0;
                if !chunk.input.is_empty() {
                    lines += 1;
                }
                if !chunk.reasoning.is_empty() {
                    lines += chunk.reasoning.lines().count();
                }
                if !chunk.content.is_empty() {
                    lines += chunk.content.lines().count();
                }
                if chunk.tool_preparing.is_some() {
                    lines += 1;
                }
                lines += chunk.tool_calls.len();
                if chunk.saved {
                    lines += 1;
                }
                if chunk.stored {
                    lines += 1;
                }
                if chunk.error.is_some() {
                    lines += 1;
                }
                if chunk.has_details() {
                    lines += 1;
                }
                if chunk.details_expanded {
                    lines += chunk
                        .tool_calls
                        .iter()
                        .filter(|tc| tc.arguments != tc.full_arguments)
                        .count();
                    if chunk.has_usage {
                        lines += 1;
                    }
                }
                lines
            })
            .sum()
    }
}

impl TaskPageChunk {
    /// 检查是否有详情可显示
    pub fn has_details(&self) -> bool {
        self.saved || self.stored || self.has_usage || !self.tool_calls.is_empty()
    }

    /// 获取错误字符（用于错误显示）
    pub fn error_character(&self) -> Option<&str> {
        self.error.as_ref().and_then(|err| {
            if let Some(start) = err.find('(') {
                let result = err[start..].find(')');
                if let Some(end) = result {
                    return Some(&err[start + 1..start + end]);
                }
            }
            None
        })
    }
}
