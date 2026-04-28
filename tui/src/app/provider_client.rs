use synapcore_core::UserMessage;
use synapcore_provider::{Provider, ProviderCommand, ProviderResponse};
use tokio::{sync::mpsc, task::JoinHandle};

/// Provider客户端错误类型
#[derive(Debug)]
pub enum ProviderClientError {
    Connection(String),
    Send(String),
    Receive(String),
    Io(std::io::Error),
}

impl std::fmt::Display for ProviderClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connection(msg) => write!(f, "连接错误: {}", msg),
            Self::Send(msg) => write!(f, "发送错误: {}", msg),
            Self::Receive(msg) => write!(f, "接收错误: {}", msg),
            Self::Io(err) => write!(f, "IO错误: {}", err),
        }
    }
}

impl std::error::Error for ProviderClientError {}

impl From<std::io::Error> for ProviderClientError {
    fn from(err: std::io::Error) -> Self {
        ProviderClientError::Io(err)
    }
}

/// Provider客户端
#[derive(Debug)]
pub struct ProviderClient {
    cmd_tx: mpsc::Sender<ProviderCommand>,
    resp_rx: mpsc::Receiver<ProviderResponse>,
}

impl ProviderClient {
    /// 连接到Provider
    pub async fn connect() -> Result<(Self, JoinHandle<()>), ProviderClientError> {
        // 创建通信通道
        let (cmd_tx, cmd_rx) = mpsc::channel::<ProviderCommand>(1024);
        let (resp_tx, resp_rx) = mpsc::channel::<ProviderResponse>(1024);

        // 创建Provider实例
        let provider = Provider::new()
            .map_err(|e| ProviderClientError::Connection(format!("创建Provider失败: {}", e)))?;

        // 启动Provider主循环任务
        let provider_handle = tokio::spawn(async move {
            if let Err(e) = provider.run(cmd_rx, resp_tx).await {
                eprintln!("Provider运行失败: {}", e);
            }
        });

        let client = Self { cmd_tx, resp_rx };
        Ok((client, provider_handle))
    }

    /// 发送消息给Provider,task
    pub async fn send_message(&mut self, text: &str) -> Result<(), ProviderClientError> {
        let message = UserMessage::task(text);
        let cmd = ProviderCommand::Send { message };

        self.cmd_tx
            .send(cmd)
            .await
            .map_err(|e| ProviderClientError::Send(format!("发送消息失败: {}", e)))?;

        Ok(())
    }

    /// 接收Provider响应
    pub async fn receive_response(&mut self) -> Option<ProviderResponse> {
        self.resp_rx.recv().await
    }

    /// 退出Provider
    pub async fn exit(&mut self) -> Result<(), ProviderClientError> {
        let cmd = ProviderCommand::Exit;

        self.cmd_tx
            .send(cmd)
            .await
            .map_err(|e| ProviderClientError::Send(format!("发送退出命令失败: {}", e)))?;

        Ok(())
    }
}
