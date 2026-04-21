use thiserror::Error;

///工具调用的错误定义
#[derive(Debug, Error)]
pub enum ToolErr {
    #[error("工具配置读取错误:{0}")]
    ReadConfigError(std::io::Error),
    #[error("序列化错误:{0}")]
    SerdeError(toml::de::Error),
    #[error("TOML failde:{0}")]
    TomlError(toml::ser::Error),
    #[error("unkown tool")]
    Unkown,
}
