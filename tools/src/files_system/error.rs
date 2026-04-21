use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum FileSystemErr {
    #[error("dir walk failed:{0}")]
    Walk(walkdir::Error),
    #[error("file io failed:{0}")]
    Fs(std::io::Error),
}
