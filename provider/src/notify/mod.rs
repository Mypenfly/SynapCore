use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotifyErr {
    #[error("系统通知发送失败: {0}")]
    Send(String),
}

type NotifyResult<T> = Result<T, NotifyErr>;

pub struct SystemNotify;

impl SystemNotify {
    pub fn send(title: &str, body: &str) -> NotifyResult<()> {
        notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .show()
            .map_err(|e| NotifyErr::Send(e.to_string()))?;
        Ok(())
    }
}
