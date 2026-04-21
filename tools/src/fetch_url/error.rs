use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum FetchErr {
    #[error("网络异常:{0}")]
    NetWork(reqwest::Error),
    #[error("Url error:{message}({code})")]
    Url { code: usize, message: String },
}
