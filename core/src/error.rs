use thiserror::Error;
use tools::error::ToolErr;

#[derive(Error,Debug)]
pub enum CoreErr {
    #[error("初始化失败：{0}")]
    InitError(String),
    // InitTomlError(Box<dyn std::error::Error>),
    #[error("助手失败: ({model}) {error}")]
    AssistantError{
        model:String,
        error:String
    },
    #[error("工具调用失败:{0}")]
    ToolError(ToolErr),
    
}

pub type CoreResult<T> = Result<T,CoreErr>;


