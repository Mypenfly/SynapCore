use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::Text,
    widgets::Paragraph,
};

use crate::app::{
    TaskPageStore,
    ui::{start_page::render_start, task_page},
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
            Constraint::Percentage(45),
            Constraint::Percentage(10),
            Constraint::Length(9),
        ])
        .split(area);

        render_start(frame, chunks[0]);
        task_page::render_input_bar(
            frame,
            chunks[2],
            input_buffer,
            false,
            input_buffer.chars().count(),
        );
    }

    /// 绘制任务页面
    pub fn draw_task_page(&self, frame: &mut Frame, task_store: &TaskPageStore, generating: bool) {
        let area = frame.area();
        task_page::render_task(frame, area, task_store, generating);
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
}
