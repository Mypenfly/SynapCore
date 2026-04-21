use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::Paragraph,
};

const LOGO: &str = include_str!("./logo.txt");
///启动页
pub fn render_start(frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(10),
            Constraint::Percentage(10),
        ])
        .split(area);

    let logo = Paragraph::new(LOGO).alignment(Alignment::Center).style(
        Style::default()
            .fg(ratatui::style::Color::Blue)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_widget(logo, chunks[1]);
}
