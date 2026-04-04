use serde::{Deserialize, Serialize};

#[derive(Deserialize,Debug,Default,Serialize,Clone)]
pub struct ToolDefinition{
    #[serde(rename="type")]
    pub tool_type:String, //"function"
    pub function:FunctionDefinition,
}

#[derive(Debug,Clone,Default,Serialize,Deserialize)]
pub struct FunctionDefinition {
    pub name:String,
    pub description: String,
    pub parameters:serde_json::Value
}
