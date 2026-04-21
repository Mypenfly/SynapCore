// use crate::request_body::tool_call::ToolCall;
use tools::define_call::tool_call::ToolCall;

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
    Tool {
        tools: Vec<ToolCall>,
    },
    Error {
        err: String,
    },
}
