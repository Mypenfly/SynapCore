// use crate::request_body::tool_call::ToolCall;
use tools::define_call::tool_call::ToolCall;

use crate::request_body::Usage;

#[derive(Default, Debug, Clone)]
pub enum LLMResponse {
    #[default]
    Nothing,
    Reasoning {
        chunk: String,
    },
    Content {
        chunk: String,
    },
    ToolPreparing{
        name:String,
    },
    ToolCall {
        tools: Vec<ToolCall>,
    },
    TokensUsage{
        usage:Usage,
    },
    Error {
        err: String,
    },
}
