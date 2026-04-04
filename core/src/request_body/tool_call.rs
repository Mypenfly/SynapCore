use serde::{Deserialize, Serialize};

#[derive(Debug,Default,PartialEq,Serialize,Deserialize,Clone)]
pub struct ToolCall{
    pub id:String,
    #[serde(rename="type")]
    pub tool_type:String,
    pub function: Function,
}

#[derive(Debug,Default,PartialEq, Serialize,Deserialize,Clone)]
pub struct Function{
    pub name:String,
    pub arguments: String,
}

