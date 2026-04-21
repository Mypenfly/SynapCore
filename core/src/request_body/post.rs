use serde::{Deserialize, Serialize};

use crate::{
    read_config::{Params, Provider},
    request_body::session::Session,
};

use tools::define_call::tool_define::ToolDefinition;

///请求体定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostBody {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub session: Session,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    pub streaming: bool,
    pub params: Params,
}

impl PostBody {
    pub fn build(
        model: String,
        provider: &Provider,
        session: &Session,
        tools: Option<Vec<ToolDefinition>>,
        params: Params,
    ) -> Self {
        Self {
            base_url: provider.base_url.clone(),
            api_key: provider.api_key.clone(),
            model,
            session: session.clone(),
            tools,
            streaming: true,
            params,
        }
    }
}
