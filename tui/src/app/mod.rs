use crossterm::event::{Event, KeyCode, KeyEvent, MouseEventKind};
use ratatui::DefaultTerminal;
use std::{cell::RefCell, rc::Rc};
use tokio::sync::mpsc;

// 模块声明
mod app_event;
mod draw;
mod provider_client;
pub mod state;
mod task_store;
pub mod ui;

// 重新导出
pub use draw::DrawWorker;
pub use provider_client::{ProviderClient, ProviderClientError};
pub use state::{AppPage, AppState};
pub use task_store::{TaskPageChunk, TaskPageStore, TaskPageStoreError};

use crate::app::app_event::AppEvent;

/// 应用错误类型
#[derive(Debug)]
pub enum AppErr {
    Draw(std::io::Error),
    Provider(ProviderClientError),
    TaskStore(TaskPageStoreError),
    Io(std::io::Error),
    Channel(String),
}

impl std::fmt::Display for AppErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draw(err) => write!(f, "绘制错误: {}", err),
            Self::Provider(err) => write!(f, "Provider错误: {}", err),
            Self::TaskStore(err) => write!(f, "任务存储错误: {}", err),
            Self::Io(err) => write!(f, "IO错误: {}", err),
            Self::Channel(msg) => write!(f, "通道错误: {}", msg),
        }
    }
}

impl std::error::Error for AppErr {}

impl From<std::io::Error> for AppErr {
    fn from(err: std::io::Error) -> Self {
        AppErr::Io(err)
    }
}

impl From<ProviderClientError> for AppErr {
    fn from(err: ProviderClientError) -> Self {
        AppErr::Provider(err)
    }
}

impl From<TaskPageStoreError> for AppErr {
    fn from(err: TaskPageStoreError) -> Self {
        AppErr::TaskStore(err)
    }
}

/// 应用主结构体
#[derive(Debug)]
pub struct App {
    /// 应用状态
    pub state: AppState,

    /// 当前页面
    pub page: AppPage,

    /// 任务页面数据存储
    pub task_store: TaskPageStore,

    /// Provider客户端
    pub provider_client: ProviderClient,

    /// 事件发送通道（用于内部事件通信）
    event_tx: Option<mpsc::Sender<AppEvent>>,

    /// 绘制工作者
    draw_worker: DrawWorker,
}

impl App {
    /// 创建新的应用实例
    pub async fn new() -> Result<Self, AppErr> {
        // TODO: 从配置文件加载主题
        let theme = Rc::new(RefCell::new(Theme::everyforest())); // 暂时使用硬编码everyforest
        //启动provider
        let (client, _joinhandle) = ProviderClient::connect().await.map_err(AppErr::Provider)?;

        let app = App {
            state: AppState::Running,
            page: AppPage::StartPage,
            task_store: TaskPageStore::new(theme),
            provider_client: client,
            event_tx: None,
            draw_worker: DrawWorker::new(),
        };
        Ok(app)
    }

    /// 运行应用主循环
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), AppErr> {
        // 创建事件通道
        let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(1024);
        self.event_tx = Some(event_tx.clone());

        // 启动键盘监听器
        let keyboard_tx = event_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::keyboard_listener(keyboard_tx).await {
                eprintln!("键盘监听器错误: {}", e);
            }
        });

        // 主事件循环
        while self.state != AppState::Stopped {
            tokio::select! {
                Some(app_event) = event_rx.recv() => {
                    self.handle_event(app_event).await?;
                }
                Some(response) = self.provider_client.receive_response() => {
                    self.handle_provider_response(response).await?;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(16)) => {
                    // 重绘界面
                    self.draw(terminal)?;
                }
            }
        }
        self.provider_client
            .exit()
            .await
            .map_err(AppErr::Provider)?;

        Ok(())
    }

    /// 处理应用事件
    async fn handle_event(&mut self, event: AppEvent) -> Result<(), AppErr> {
        match event {
            AppEvent::Key(key_event) => {
                self.handle_key(key_event).await?;
            }
            AppEvent::ProviderResponse(response) => {
                self.handle_provider_response(response).await?;
            }
            AppEvent::InputSubmitted(text) => {
                self.on_input_submitted(&text).await?;
            }
            AppEvent::Scroll(delta) => {
                if delta > 0 {
                    self.task_store.scroll_up(delta as usize);
                } else {
                    self.task_store.scroll_down((-delta) as usize);
                }
            }
            AppEvent::Generating(_generating) => {
                // 生成状态已由handle_provider_response处理
            }
            AppEvent::Exit => {
                self.state = AppState::Stopped;
            }
            _ => {} // 处理其他事件类型
        }
        Ok(())
    }

    /// 处理键盘事件
    async fn handle_key(&mut self, key_event: KeyEvent) -> Result<(), AppErr> {
        match key_event.code {
            KeyCode::Esc => {
                self.state = AppState::Stopped;
            }
            KeyCode::Char(c) => {
                self.task_store.insert_char(c);
            }
            KeyCode::Enter => {
                if !self.task_store.input_buffer.trim().is_empty() {
                    let text = self.task_store.input_buffer.clone();
                    self.task_store.input_buffer.clear();
                    self.task_store.cursor_position = 0;

                    if let Some(tx) = &self.event_tx {
                        tx.send(AppEvent::InputSubmitted(text)).await.map_err(|e| {
                            AppErr::Channel(format!("发送InputSubmitted事件失败: {}", e))
                        })?;
                    }
                }
            }
            KeyCode::Backspace => {
                self.task_store.delete_char_backward();
            }
            KeyCode::Left => {
                self.task_store.move_cursor_left();
            }
            KeyCode::Right => {
                self.task_store.move_cursor_right();
            }
            KeyCode::Home => {
                self.task_store.move_cursor_home();
            }
            KeyCode::End => {
                self.task_store.move_cursor_end();
            }
            KeyCode::Up => {
                if let Some(tx) = &self.event_tx {
                    tx.send(AppEvent::Scroll(1))
                        .await
                        .map_err(|e| AppErr::Channel(format!("发送Scroll事件失败: {}", e)))?;
                }
            }
            KeyCode::Down => {
                if let Some(tx) = &self.event_tx {
                    tx.send(AppEvent::Scroll(-1))
                        .await
                        .map_err(|e| AppErr::Channel(format!("发送Scroll事件失败: {}", e)))?;
                }
            }
            KeyCode::PageUp => {
                if let Some(tx) = &self.event_tx {
                    tx.send(AppEvent::Scroll(10))
                        .await
                        .map_err(|e| AppErr::Channel(format!("发送Scroll事件失败: {}", e)))?;
                }
            }
            KeyCode::PageDown => {
                if let Some(tx) = &self.event_tx {
                    tx.send(AppEvent::Scroll(-10))
                        .await
                        .map_err(|e| AppErr::Channel(format!("发送Scroll事件失败: {}", e)))?;
                }
            }
            KeyCode::Delete => {
                self.task_store.delete_char_forward();
            }
            _ => {}
        }
        Ok(())
    }

    /// 处理输入提交
    async fn on_input_submitted(&mut self, text: &str) -> Result<(), AppErr> {
        // 如果是StartPage，跳转到TaskPage
        if matches!(self.page, AppPage::StartPage) {
            self.page = AppPage::TaskPage;
        }

        // 在TaskPage中创建新的chunk
        self.task_store.add_new_chunk(text);

        // 发送消息到Provider
        self.provider_client.send_message(text).await?;

        Ok(())
    }

    /// 处理Provider响应
    async fn handle_provider_response(
        &mut self,
        response: synapcore_provider::ProviderResponse,
    ) -> Result<(), AppErr> {
        match response {
            synapcore_provider::ProviderResponse::Response(bot_resp) => {
                self.task_store.handle_bot_response(bot_resp);
            }
            synapcore_provider::ProviderResponse::Error(err) => {
                self.task_store.set_error("Provider", &err);
            }
        }
        Ok(())
    }

    /// 绘制界面
    fn draw(&mut self, terminal: &mut DefaultTerminal) -> Result<(), AppErr> {
        terminal
            .draw(|frame| {
                match self.page {
                    AppPage::StartPage => {
                        self.draw_worker
                            .draw_start_page(frame, &self.task_store.input_buffer);
                    }
                    AppPage::TaskPage => {
                        // 简单检查是否有生成中的chunk
                        let generating = self.task_store.generating;
                        self.draw_worker
                            .draw_task_page(frame, &self.task_store, generating);
                    }
                    AppPage::ChatPage => {
                        // TODO: 实现ChatPage
                        self.draw_worker
                            .draw_placeholder(frame, "Chat Page (未实现)");
                    }
                }
            })
            .map_err(AppErr::Draw)?;

        Ok(())
    }

    /// 键盘/鼠标监听器任务
    async fn keyboard_listener(event_tx: mpsc::Sender<AppEvent>) -> Result<(), AppErr> {
        loop {
            let event = tokio::task::spawn_blocking(crossterm::event::read)
                .await
                .map_err(|e| AppErr::Io(std::io::Error::other(e.to_string())))?
                .map_err(|e| AppErr::Io(std::io::Error::other(e.to_string())))?;

            match event {
                Event::Key(key_event) => {
                    event_tx
                        .send(AppEvent::Key(key_event))
                        .await
                        .map_err(|e| AppErr::Channel(format!("发送键盘事件失败: {}", e)))?;
                }
                Event::Mouse(mouse_event) => match mouse_event.kind {
                    MouseEventKind::ScrollUp => {
                        let _ = event_tx.send(AppEvent::Scroll(1)).await;
                    }
                    MouseEventKind::ScrollDown => {
                        let _ = event_tx.send(AppEvent::Scroll(-1)).await;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

pub type AppResult<T> = Result<T, AppErr>;

// 临时主题定义（待移动到theme模块）
#[derive(Debug, Clone, Default)]
pub struct Theme {
    pub name: String,
}

impl Theme {
    pub fn everyforest() -> Self {
        Theme {
            name: "everyforest".to_string(),
        }
    }

    pub fn one_dark() -> Self {
        Theme {
            name: "one_dark".to_string(),
        }
    }
}
