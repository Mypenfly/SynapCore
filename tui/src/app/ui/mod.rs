use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Padding, Paragraph},
};

pub mod start_page;
pub mod task_page;
// use crate::state::AppPage;

pub fn render_input(frame: &mut Frame, area: Rect, text: String) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .vertical_margin(1)
        .horizontal_margin(2)
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .padding(Padding::horizontal(1))
        .title("ask for agents");

    frame.render_widget(block.clone(), chunks[1]);

    let inner = block.inner(chunks[1]);

    let paragraph = Paragraph::new(text.clone())
        .style(Style::default())
        .scroll((text.len().saturating_sub(inner.width as usize) as u16, 0));

    frame.render_widget(paragraph, inner);
}
