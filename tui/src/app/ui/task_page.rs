use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::TaskPageStore;

const MARGIN_H: u16 = 2;
const INPUT_VISIBLE_LINES: usize = 5;
const INPUT_PADDING_BOTTOM: u16 = 2;

pub fn render_task(frame: &mut Frame, area: Rect, store: &TaskPageStore, generating: bool) {
    let inner_area = Rect {
        x: area.x + MARGIN_H,
        width: area.width.saturating_sub(MARGIN_H * 2),
        ..area
    };

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(10),
        Constraint::Length(1),
        Constraint::Length(INPUT_VISIBLE_LINES as u16 + INPUT_PADDING_BOTTOM + 2),
    ])
    .split(inner_area);

    render_top_bar(frame, chunks[0], store);
    render_message_area(frame, chunks[1], store);
    render_input_bar(
        frame,
        chunks[3],
        &store.input_buffer,
        generating,
        store.cursor_position,
    );
}

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

fn render_message_area(frame: &mut Frame, area: Rect, store: &TaskPageStore) {
    if store.chunks.is_empty() {
        let text = Text::styled("输入问题开始对话", Style::default().fg(Color::DarkGray));
        let paragraph = Paragraph::new(text).alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let text = build_message_text(store);

    let scroll = if store.auto_scrolling && store.generating {
        u16::MAX
    } else {
        store.scroll_offset as u16
    };

    let paragraph = Paragraph::new(text)
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn build_message_text(store: &TaskPageStore) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for (i, chunk) in store.chunks.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }

        if !chunk.input.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("> {}", chunk.input),
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )));
        }

        if !chunk.reasoning.is_empty() {
            for rl in chunk.reasoning.lines() {
                lines.push(Line::from(Span::styled(
                    rl.to_string(),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        if !chunk.content.is_empty() {
            for cl in chunk.content.lines() {
                lines.push(Line::from(Span::raw(cl.to_string())));
            }
        }

        if let Some(tp) = &chunk.tool_preparing {
            lines.push(Line::from(Span::styled(
                format!("{} preparing tool: {}", tp.character, tp.name),
                Style::default().fg(Color::Cyan),
            )));
        }

        for tc in &chunk.tool_calls {
            lines.push(Line::from(Span::styled(
                format!(
                    "{} using tool: {} - {}",
                    tc.character, tc.name, tc.arguments
                ),
                Style::default().fg(Color::Blue),
            )));
        }

        if chunk.saved {
            lines.push(Line::from(Span::styled(
                "✓ 对话已保存",
                Style::default().fg(Color::Green),
            )));
        }

        if chunk.stored {
            lines.push(Line::from(Span::styled(
                "✓ 记忆已存储",
                Style::default().fg(Color::Green),
            )));
        }

        if let Some(err) = &chunk.error {
            lines.push(Line::from(Span::styled(
                format!("❌ {}", err),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
        }

        if chunk.has_details() {
            let detail_text = if chunk.details_expanded {
                "[收起详情]"
            } else {
                "[显示详情]"
            };
            lines.push(Line::from(Span::styled(
                detail_text,
                Style::default().fg(Color::DarkGray),
            )));

            if chunk.details_expanded {
                for tc in &chunk.tool_calls {
                    if tc.arguments != tc.full_arguments {
                        lines.push(Line::from(Span::styled(
                            format!("完整参数: {}", tc.full_arguments),
                            Style::default().fg(Color::DarkGray),
                        )));
                    }
                }
                if chunk.has_usage {
                    lines.push(Line::from(Span::styled(
                        "token用量: 已记录",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        }
    }

    Text::from(lines)
}

pub fn render_input_bar(
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

    let inner = block.inner(area);

    let input_chunks = Layout::vertical([
        Constraint::Length(INPUT_VISIBLE_LINES as u16),
        Constraint::Length(INPUT_PADDING_BOTTOM),
    ])
    .split(inner);

    render_input_text(frame, input_chunks[0], input, generating, cursor_pos);
    frame.render_widget(block, area);
}

fn render_input_text(
    frame: &mut Frame,
    area: Rect,
    input: &str,
    generating: bool,
    cursor_pos: usize,
) {
    let max_width = area.width as usize;
    if max_width == 0 {
        return;
    }

    if input.is_empty() && !generating {
        let text = Text::from(Line::from(Span::styled(
            "输入以发起对话/任务...",
            Style::default().fg(Color::DarkGray),
        )));
        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph, area);
        return;
    }

    let wrapped = wrap_text(input, max_width);
    let (cursor_line, cursor_col) = find_cursor_in_wrapped(input, cursor_pos, max_width);
    let total_lines = wrapped.len();
    let visible = INPUT_VISIBLE_LINES;

    let scroll = if total_lines <= visible || cursor_line < visible {
        0
    } else {
        (cursor_line - visible + 1).min(total_lines - visible)
    };

    let start = scroll;
    let end = (start + visible).min(total_lines);

    let mut lines: Vec<Line<'static>> = Vec::new();

    for (li, line_text) in wrapped[start..end].iter().cloned().enumerate() {
        if li == cursor_line {
            let char_count = line_text.chars().count();
            let col = cursor_col.min(char_count);
            let before: String = line_text.chars().take(col).collect();
            let cursor_char = line_text.chars().nth(col).unwrap_or(' ').to_string();
            let after: String = line_text.chars().skip(col + 1).collect();

            let mut spans = Vec::new();
            if !before.is_empty() {
                spans.push(Span::raw(before));
            }
            spans.push(Span::styled(
                cursor_char,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ));
            if !after.is_empty() {
                spans.push(Span::raw(after));
            }
            lines.push(Line::from(spans));
        } else {
            lines.push(Line::from(Span::raw(line_text)));
        }
    }

    if generating {
        let animation = get_loading_animation();
        if lines.len() < visible {
            lines.push(Line::from(Span::styled(
                animation,
                Style::default().fg(Color::Cyan),
            )));
        } else if let Some(last) = lines.last_mut() {
            last.push_span(Span::styled(
                format!(" {}", animation),
                Style::default().fg(Color::Cyan),
            ));
        }
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw_line in text.split('\n') {
        let chars: Vec<char> = raw_line.chars().collect();
        if chars.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut start = 0;
        while start < chars.len() {
            let end = (start + max_width).min(chars.len());
            lines.push(chars[start..end].iter().collect());
            start = end;
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn find_cursor_in_wrapped(text: &str, cursor_pos: usize, max_width: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let pos = cursor_pos.min(chars.len());
    let mut line = 0usize;
    let mut col = 0usize;
    for char in chars[0..pos].iter() {
        if char == &'\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
            if col >= max_width && max_width > 0 {
                line += 1;
                col = 0;
            }
        }
    }
    (line, col)
}

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
