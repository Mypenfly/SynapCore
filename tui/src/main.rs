mod app;
use app::{App, AppErr};
use std::io;

use ratatui::crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};

#[derive(Debug)]
#[allow(dead_code)]
enum MainErr {
    TermianlError(std::io::Error),
    AppError(AppErr),
}

type MainResult<T> = Result<T, MainErr>;

#[tokio::main]
async fn main() -> MainResult<()> {
    let mut terminal = ratatui::init();

    terminal.clear().map_err(MainErr::TermianlError)?;
    execute!(io::stdout(), EnableMouseCapture).map_err(MainErr::TermianlError)?;

    let mut app = App::new().await.map_err(MainErr::AppError)?;
    let result = app.run(&mut terminal).await.map_err(MainErr::AppError);
    execute!(io::stdout(), DisableMouseCapture).map_err(MainErr::TermianlError)?;

    ratatui::restore();
    result
}
