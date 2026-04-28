use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Text,
    widgets::Paragraph,
};

use crate::app::{
    TaskPageStore,
    ui::{start_page::render_start, task_page::render_task},
};

/// 绘制工作者
#[derive(Debug, Default, Clone)]
pub struct DrawWorker;

impl DrawWorker {
    pub fn new() -> Self {
        Self
    }

    /// 绘制启动页面
    pub fn draw_start_page(&self, frame: &mut Frame, input_buffer: &str) {
        let area = frame.area();
        let chunks = Layout::vertical([
            Constraint::Percentage(50),
            Constraint::Percentage(20),
            Constraint::Percentage(30),
        ])
        .split(area);

        render_start(frame, chunks[0]);
        self.draw_input_bar(frame, chunks[1], input_buffer, false, "Start");
    }

    /// 绘制任务页面
    pub fn draw_task_page(&self, frame: &mut Frame, task_store: &TaskPageStore, generating: bool) {
        let area = frame.area();
        render_task(frame, area, task_store, generating);
    }

    /// 绘制占位符页面
    pub fn draw_placeholder(&self, frame: &mut Frame, message: &str) {
        let area = frame.area();
        let text = Text::styled(
            format!("{} (功能待开发)", message),
            Style::default().fg(Color::Yellow),
        );
        let paragraph = Paragraph::new(text).alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// 绘制输入栏（简单版本）
    fn draw_input_bar(
        &self,
        frame: &mut Frame,
        area: Rect,
        text: &str,
        generating: bool,
        page_label: &str,
    ) {
        let mut display_text = if text.is_empty() && !generating {
            format!("{} > 输入以发起对话/任务", page_label)
        } else {
            format!("{} > {}", page_label, text)
        };

        // 添加生成动画
        if generating {
            display_text.push_str(" ...");
        }

        let paragraph = Paragraph::new(display_text).style(Style::default().fg(Color::White));

        frame.render_widget(paragraph, area);
    }
}
