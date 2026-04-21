use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum TimerToolErr {
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("序列化错误: {0}")]
    Serde(#[from] serde_json::Error),
}
