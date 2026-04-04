use ratatui::{Frame, layout::{Constraint, Layout, Rect}, style::Style, widgets::Paragraph};

pub fn render_task(frame:&mut Frame,area:Rect,messenge:&str,task:&str) {

    let chunks = Layout::default().direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage(95),
            Constraint::Percentage(5),
        ]).split(area);

    let  para= Paragraph::new(messenge)
        .alignment(ratatui::layout::HorizontalAlignment::Center)
        .style(Style::default());

    frame.render_widget(para, chunks[0]);
    render_bar(frame, chunks[1], task);
}

fn render_bar(frame:&mut Frame,area:Rect,task:&str){

    let txt = format!("Now task: {}",task);

    let para = Paragraph::new(txt)
        .alignment(ratatui::layout::HorizontalAlignment::Left)
        .style(Style::default().fg(ratatui::style::Color::LightCyan));

    frame.render_widget(para, area);
}
