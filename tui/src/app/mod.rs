use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, KeyCode},
};
mod draw;
pub mod state;
pub mod ui;
use draw::DrawWorker;
use state::{AppPage, AppState};

///app 的错误类型
#[derive(Debug)]
pub enum AppErr {
    DrawError(std::io::Error),
}

pub type AppResult<T> = Result<T, AppErr>;

///app 定义
#[derive(Debug, Default, Clone)]
pub struct App {
    ///状态
    state: AppState,

    ///页面
    page: AppPage,

    ///输入
    input: String,

    ///存放信息
    draw_worker:DrawWorker
}

impl App {
    pub fn new() -> Self {
        let state = AppState::default();
        let page = AppPage::default();
        let input = String::new();
        let draw_worker = DrawWorker::new();
        Self { state, page, input, draw_worker }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> AppResult<()> {
        let (key_tx, mut key_rx) = tokio::sync::mpsc::channel::<KeyCode>(2);
        tokio::spawn(async move {
            loop {
                let event = tokio::task::spawn_blocking(event::read).await;
                match event {
                    Ok(Ok(event::Event::Key(key))) => {
                        if key_tx.send(key.code).await.is_err() {
                            break;
                        }
                    }
                    _ => break,
                }
            }
        });

        while self.state != AppState::Stopped {
            tokio::select! {
                Some(key_code) = key_rx.recv() => {
                    self.handle_key(key_code).await;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(16)) => {
                    terminal.draw(|frame| self.draw_worker.draw_ui(frame, self.input.clone(), &self.page))
                        .map_err(AppErr::DrawError)?;
                }
            }
            terminal
                .draw(|frame| self.draw_worker.draw_ui(frame, self.input.clone(), &self.page))
                .map_err(AppErr::DrawError)?;
        }
        Ok(())
    }

    pub async fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => self.state = AppState::Stopped,
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Enter => {
                self.input = String::new();
                self.page = AppPage::TaskPage;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            _ => (),
        }
    }
}
