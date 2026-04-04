use thiserror::Error;

///提取错误定义
#[derive(Error, Debug)]
pub enum ExtractErr {
    #[error("args json serialize failed:{0}")]
    JsonError(serde_json::Error),
    #[error("checked failed:{0}")]
    Check(String),
    #[error("pdf extract failed:{0}")]
    Pdf(pdf_extract::OutputError),
    #[error("File operation failed:{0}")]
    File(std::io::Error),
    #[error("docx extract failed:{0}")]
    Docx(docx_rs::ReaderError),
    #[error("xml extract failed:{0}")]
    Xml(String),
}

pub type ExtractResult<T> = Result<T, ExtractErr>;
