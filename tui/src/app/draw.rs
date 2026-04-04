use crate::app::ui::task_page::render_task;

use super::state::AppPage;
use super::ui::render_input;
use super::ui::start_page::render_start;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
};

///输出内容的管理
#[derive(Debug, Default, Clone)]
pub struct DrawWorker {
    pub task: String,
    pub messenge: String,
}

impl DrawWorker {
    pub fn new() -> Self {
        Self {
            task: String::new(),
            messenge: String::new(),
        }
    }

    pub fn draw_ui(&mut self, frame: &mut Frame, text: String, page: &AppPage) {
        let area = frame.area();

        match page {
            AppPage::StartPage => {
                let chunks = Layout::vertical([
                    Constraint::Percentage(50),
                    Constraint::Percentage(20),
                    Constraint::Percentage(30),
                ])
                .split(area);
                render_start(frame, chunks[0]);
                render_input(frame, chunks[1], text);
            }
            AppPage::TaskPage => {
                let chunks =
                    Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)])
                        .split(area);
                render_task(frame, chunks[0], &self.messenge, &self.task);
                render_input(frame, chunks[1], text);
            }
            _ => (),
        }
    }
}
