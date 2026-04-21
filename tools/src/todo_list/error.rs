use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum TodoListErr {
    #[error("Io failed : {0}")]
    Io(std::io::Error),
    #[error("Serde failed : {0}")]
    Serde(serde_json::Error),
}
