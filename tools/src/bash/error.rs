use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum BashErr {
    #[error("Init bash failed: {0}")]
    Init(std::io::Error),
    #[error("Other failed: {0}")]
    Other(String),
}
