use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::TaskPageStore;

/// 渲染任务页面
pub fn render_task(frame: &mut Frame, area: Rect, store: &TaskPageStore, generating: bool) {
    // 垂直布局：顶栏、消息区域、输入栏
    let chunks = Layout::vertical([
        Constraint::Length(2), // 顶栏
        Constraint::Min(10),   // 消息区域（可滚动）
        Constraint::Length(4), // 输入栏（简化版）
    ])
    .split(area);

    render_top_bar(frame, chunks[0], store);
    render_message_area(frame, chunks[1], store);
    render_input_bar(
        frame,
        chunks[2],
        &store.input_buffer,
        generating,
        store.cursor_position,
    );
}

/// 渲染顶栏（吸顶）
fn render_top_bar(frame: &mut Frame, area: Rect, store: &TaskPageStore) {
    let current_task = store.current_input();
    let text = if current_task.is_empty() {
        "等待任务输入..."
    } else {
        &current_task
    };

    let block = Block::default()
        .title("当前任务")
        .borders(Borders::BOTTOM)
        .style(Style::default().fg(Color::LightCyan));

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

/// 渲染消息区域
fn render_message_area(frame: &mut Frame, area: Rect, store: &TaskPageStore) {
    if store.chunks.is_empty() {
        render_empty_message_area(frame, area);
        return;
    }

    // 创建消息列表
    let items: Vec<ListItem> = store
        .chunks
        .iter()
        .enumerate()
        .flat_map(|(idx, chunk)| render_chunk_items(idx, chunk))
        .collect();

    let scroll_offset = if store.auto_scrolling && store.generating {
        // 自动滚动时使用一个大值，ratatui 会自动 clamp 到底部
        let total = store.total_chunk_lines();
        let visible = area.height as usize;
        total.saturating_sub(visible)
    } else {
        store.scroll_offset
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    let mut list_state = ListState::default().with_offset(scroll_offset);
    frame.render_stateful_widget(list, area, &mut list_state);
}

/// 渲染空消息区域
fn render_empty_message_area(frame: &mut Frame, area: Rect) {
    let text = Text::styled("输入问题开始对话", Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(text).alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(paragraph, area);
}

/// 渲染单个chunk的项目
fn render_chunk_items(_chunk_idx: usize, chunk: &crate::app::TaskPageChunk) -> Vec<ListItem> {
    let mut items = Vec::new();

    // 1. 用户输入（深灰色背景，白色字体）
    if !chunk.input.is_empty() {
        let input_text = format!("> {}", chunk.input);
        items.push(ListItem::new(Text::styled(
            input_text,
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
    }

    // 2. Reasoning部分（灰色字体）
    if !chunk.reasoning.is_empty() {
        items.push(ListItem::new(Text::styled(
            chunk.reasoning.clone(),
            Style::default().fg(Color::DarkGray),
        )));
    }

    // 3. Content部分（Markdown渲染）
    if !chunk.content.is_empty() {
        // 简化版：先使用普通文本，后续集成tui-markdown
        items.push(ListItem::new(Text::raw(chunk.content.clone())));
    }

    // 4. Tool准备信息
    if let Some(tool_prep) = &chunk.tool_preparing {
        let text = format!("{} preparing tool: {}", tool_prep.character, tool_prep.name);
        items.push(ListItem::new(Text::styled(
            text,
            Style::default().fg(Color::Cyan),
        )));
    }

    // 5. Tool调用信息
    for tool_call in &chunk.tool_calls {
        let text = format!(
            "{} using tool: {} - {}",
            tool_call.character, tool_call.name, tool_call.arguments
        );
        items.push(ListItem::new(Text::styled(
            text,
            Style::default().fg(Color::Blue),
        )));
    }

    // 6. 保存/存储状态
    if chunk.saved {
        items.push(ListItem::new(Text::styled(
            "✓ 对话已保存",
            Style::default().fg(Color::Green),
        )));
    }

    if chunk.stored {
        items.push(ListItem::new(Text::styled(
            "✓ 记忆已存储",
            Style::default().fg(Color::Green),
        )));
    }

    // 7. 错误信息
    if let Some(err) = &chunk.error {
        items.push(ListItem::new(Text::styled(
            format!("❌ {}", err),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
    }

    // 8. 详情按钮
    if chunk.has_details() {
        let detail_text = if chunk.details_expanded {
            "[收起详情]"
        } else {
            "[显示详情]"
        };

        items.push(ListItem::new(Text::styled(
            detail_text,
            Style::default().fg(Color::DarkGray),
        )));

        // 如果展开，显示详细信息
        if chunk.details_expanded {
            // 显示工具调用的完整参数
            for tool_call in &chunk.tool_calls {
                if tool_call.arguments != tool_call.full_arguments {
                    items.push(ListItem::new(Text::styled(
                        format!("完整参数: {}", tool_call.full_arguments),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }

            // 显示token用量信息
            if chunk.has_usage {
                items.push(ListItem::new(Text::styled(
                    "token用量: 已记录",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
    }

    // 添加分隔线（除了最后一个chunk）
    items
}

/// 渲染输入栏
fn render_input_bar(
    frame: &mut Frame,
    area: Rect,
    input: &str,
    generating: bool,
    cursor_pos: usize,
) {
    let border_style = if generating {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .title("输入")
        .borders(Borders::ALL)
        .border_style(border_style);

    let text = if input.is_empty() && !generating {
        let mut spans = vec![Span::styled(
            "输入以发起对话/任务...",
            Style::default().fg(Color::DarkGray),
        )];
        spans.push(Span::styled("█", Style::default().fg(Color::White)));
        Text::from(Line::from(spans))
    } else {
        let char_count = input.chars().count();
        let pos = cursor_pos.min(char_count);
        let mut spans = Vec::new();

        // 光标位置之前的文本
        let before: String = input.chars().take(pos).collect();
        if !before.is_empty() {
            spans.push(Span::raw(before));
        }

        // 光标字符
        let cursor_char = input.chars().nth(pos).unwrap_or(' ').to_string();
        spans.push(Span::styled(
            cursor_char,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));

        // 光标位置之后的文本
        let after: String = input.chars().skip(pos + 1).collect();
        if !after.is_empty() {
            spans.push(Span::raw(after));
        }

        // 添加生成动画
        if generating {
            let animation = get_loading_animation();
            spans.push(Span::styled(
                format!(" {}", animation),
                Style::default().fg(Color::Cyan),
            ));
        }

        Text::from(Line::from(spans))
    };

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

/// 获取加载动画字符
fn get_loading_animation() -> &'static str {
    use std::time::SystemTime;

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    match now % 4 {
        0 => "⠋",
        1 => "⠙",
        2 => "⠹",
        3 => "⠸",
        _ => "⠴",
    }
}

/// 工具条渲染（旧版本兼容）
#[allow(dead_code)]
fn render_bar(frame: &mut Frame, area: Rect, task: &str) {
    let txt = format!("当前任务: {}", task);

    let para = Paragraph::new(txt)
        .alignment(ratatui::layout::Alignment::Left)
        .style(Style::default().fg(Color::LightCyan));

    frame.render_widget(para, area);
}
